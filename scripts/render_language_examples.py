#!/usr/bin/env python3
"""Render the ER all-glyphs example with every imported implementation."""

import shutil
import subprocess
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
INPUT_PATH = REPOSITORY_ROOT / "render_examples" / "er_all_glyphs.sbgn"
OUTPUT_ROOT = REPOSITORY_ROOT / "output" / "languages"
OUTPUT_FORMATS = ("png", "svg")


def run_renderer(language: str, command: list[str], working_directory: Path) -> None:
    """Run one renderer for both supported output formats.

    Args:
        language: Output subdirectory and filename label.
        command: Renderer command before input and output arguments.
        working_directory: Directory from which to invoke the renderer.

    Returns:
        None.
    """

    output_directory = OUTPUT_ROOT / language
    output_directory.mkdir(parents=True, exist_ok=True)
    for output_format in OUTPUT_FORMATS:
        output_path = output_directory / f"er_all_glyphs.{output_format}"
        subprocess.run(
            [
                *command,
                "-i",
                str(INPUT_PATH),
                "-o",
                str(output_path),
            ],
            cwd=working_directory,
            check=True,
        )


def require_command(command: str) -> None:
    """Raise a helpful error when a required language tool is unavailable.

    Args:
        command: Executable name to locate on PATH.

    Returns:
        None.
    """

    if shutil.which(command) is None:
        raise RuntimeError(f"Required command is not installed: {command}")


def main() -> None:
    """Generate PNG and SVG ER renders in Rust, Go, R, and Python.

    Returns:
        None.
    """

    for command in ("cargo", "go", "Rscript", "uv"):
        require_command(command)

    run_renderer(
        "rust",
        ["cargo", "run", "--release", "--", "draw_sbgnml"],
        REPOSITORY_ROOT,
    )
    run_renderer(
        "go",
        ["go", "run", ".", "draw_sbgnml"],
        REPOSITORY_ROOT / "go",
    )
    run_renderer(
        "r",
        ["Rscript", "r/draw_sbgnml.R"],
        REPOSITORY_ROOT,
    )
    run_renderer(
        "python",
        ["uv", "run", "--frozen", "render_sbgn_py", "draw_sbgnml"],
        REPOSITORY_ROOT / "python",
    )


if __name__ == "__main__":
    main()
