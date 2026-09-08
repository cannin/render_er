#!/usr/bin/env python3
"""Exercise every renderer against the canonical ER SBGN-ML fixture."""

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
from typing import Any, Sequence

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INPUT_PATH = REPOSITORY_ROOT / "render_examples" / ("er_all_glyphs.sbgn")
DEFAULT_WIDTH = 1200
DEFAULT_HEIGHT = 800
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
CONFORMANCE_FIELDS = (
    "id",
    "kind",
    "class",
    "text",
    "source",
    "target",
    "marker",
)


@dataclass(frozen=True)
class Renderer:
    """Describe one renderer command.

    Args:
        name: Stable name used for output directories.
        command: Executable and fixed arguments before renderer options.
        working_directory: Directory in which the command runs.
    """

    name: str
    command: tuple[str, ...]
    working_directory: Path


def run_command(command: Sequence[str], working_directory: Path) -> None:
    """Run a command and fail with its native diagnostic.

    Args:
        command: Command and arguments to execute.
        working_directory: Directory in which to run the command.

    Returns:
        None.
    """
    subprocess.run(command, cwd=working_directory, check=True)


def require_tools() -> None:
    """Verify that all native toolchains are available.

    Returns:
        None.
    """
    missing = [
        name for name in ("cargo", "go", "Rscript") if shutil.which(name) is None
    ]
    if missing:
        raise RuntimeError(f"Missing required tools: {', '.join(missing)}")


def build_renderers(output_directory: Path) -> list[Renderer]:
    """Build native renderers and describe all four invocations.

    Args:
        output_directory: Root for generated test artifacts.

    Returns:
        Renderer command descriptions for all implementations.
    """
    rust_directory = REPOSITORY_ROOT / "rust"
    go_directory = REPOSITORY_ROOT / "go"
    go_binary = output_directory / "bin" / "render_sbgn_go"
    go_binary.parent.mkdir(parents=True, exist_ok=True)

    run_command(
        [
            "cargo",
            "build",
            "--manifest-path",
            str(rust_directory / "Cargo.toml"),
            "--bin",
            "render_er",
        ],
        REPOSITORY_ROOT,
    )
    run_command(["go", "build", "-o", str(go_binary), "."], go_directory)

    return [
        Renderer(
            "python",
            (sys.executable, "-m", "render_sbgn_py.cli"),
            REPOSITORY_ROOT / "python",
        ),
        Renderer(
            "rust",
            (str(rust_directory / "target" / "debug" / "render_er"),),
            rust_directory,
        ),
        Renderer("go", (str(go_binary),), go_directory),
        Renderer(
            "r",
            ("Rscript", "draw_sbgnml.R"),
            REPOSITORY_ROOT / "r",
        ),
    ]


def verify_png(path: Path, width: int, height: int) -> None:
    """Verify a generated PNG signature and dimensions.

    Args:
        path: PNG file to inspect.
        width: Expected pixel width.
        height: Expected pixel height.

    Returns:
        None.
    """
    header = path.read_bytes()[:24]
    if len(header) != 24 or header[:8] != PNG_SIGNATURE or header[12:16] != b"IHDR":
        raise AssertionError(f"{path} is not a valid PNG")
    actual_width, actual_height = struct.unpack(">II", header[16:24])
    if (actual_width, actual_height) != (width, height):
        raise AssertionError(
            f"{path} is {actual_width}x{actual_height}, expected {width}x{height}"
        )


def load_manifest(
    path: Path, input_path: Path, width: int, height: int
) -> dict[str, Any]:
    """Load and validate a renderer manifest.

    Args:
        path: JSON manifest path.
        input_path: Source diagram path.
        width: Expected canvas width.
        height: Expected canvas height.

    Returns:
        Parsed manifest record.
    """
    manifest = json.loads(path.read_text(encoding="utf-8"))
    if manifest.get("diagram_id") != input_path.name:
        raise AssertionError(f"Unexpected diagram_id in {path}")
    if manifest.get("coordinate_space") != "rendered_pixel":
        raise AssertionError(f"Unexpected coordinate_space in {path}")
    canvas = manifest.get("canvas", {})
    if (canvas.get("width"), canvas.get("height")) != (width, height):
        raise AssertionError(f"Unexpected canvas dimensions in {path}")
    if not manifest.get("elements"):
        raise AssertionError(f"No rendered elements in {path}")
    return manifest


def conformance_signature(manifest: dict[str, Any]) -> list[tuple[Any, ...]]:
    """Return semantic and marker fields shared by all backends.

    Args:
        manifest: Parsed renderer manifest.
    Returns:
        Stable tuples describing shared element identity and topology.
    """
    return sorted(
        tuple(element.get(field) for field in CONFORMANCE_FIELDS)
        for element in manifest["elements"]
    )


def render_fixture(
    renderer: Renderer,
    input_path: Path,
    output_directory: Path,
    width: int,
    height: int,
) -> dict[str, Any]:
    """Render one image and manifest, then validate both.

    Args:
        renderer: Renderer command description.
        input_path: Canonical SBGN-ML file.
        output_directory: Root for generated output.
        width: Requested image width.
        height: Requested image height.

    Returns:
        Parsed manifest record.
    """
    renderer_directory = output_directory / renderer.name
    renderer_directory.mkdir(parents=True, exist_ok=True)
    image_path = renderer_directory / f"{input_path.stem}.png"
    manifest_path = renderer_directory / f"{input_path.stem}.json"
    shared_args = (
        "draw_sbgnml",
        "--input-path",
        str(input_path),
        "--width",
        str(width),
        "--height",
        str(height),
    )

    run_command(
        (*renderer.command, *shared_args, "--output-path", str(image_path)),
        renderer.working_directory,
    )
    verify_png(image_path, width, height)
    run_command(
        (
            *renderer.command,
            *shared_args,
            "--output-path",
            str(manifest_path),
            "--generate-render-test-manifest",
        ),
        renderer.working_directory,
    )
    return load_manifest(manifest_path, input_path, width, height)


def run_conformance(
    input_path: Path, output_directory: Path, width: int, height: int
) -> None:
    """Render the fixture and compare structural manifests.

    Args:
        input_path: Canonical ER fixture.
        output_directory: Root for temporary or requested artifacts.
        width: Requested image width.
        height: Requested image height.

    Returns:
        None.
    """
    if not input_path.is_file():
        raise FileNotFoundError(f"Missing conformance fixture: {input_path}")

    renderers = build_renderers(output_directory)
    manifests = {
        renderer.name: render_fixture(
            renderer, input_path, output_directory, width, height
        )
        for renderer in renderers
    }
    baseline = conformance_signature(manifests["python"])
    for name, manifest in manifests.items():
        if conformance_signature(manifest) != baseline:
            raise AssertionError(
                f"{input_path.name}: {name} manifest semantics or markers "
                "differs from Python"
            )

    print(
        f"Conformance passed for {input_path.name} and {len(renderers)} renderers "
        "(shared element semantics and markers)."
    )


def main() -> None:
    """Parse arguments and run cross-language conformance checks.

    Returns:
        None.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT_PATH)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--width", type=int, default=DEFAULT_WIDTH)
    parser.add_argument("--height", type=int, default=DEFAULT_HEIGHT)
    args = parser.parse_args()

    require_tools()
    if args.output_dir is not None:
        run_conformance(args.input, args.output_dir, args.width, args.height)
        return

    with tempfile.TemporaryDirectory(prefix="render-er-conformance-") as temporary:
        run_conformance(args.input, Path(temporary), args.width, args.height)


if __name__ == "__main__":
    main()
