#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "pillow>=11.3.0",
# ]
# ///
"""Render the ER all-glyphs example with every imported implementation."""

import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
INPUT_PATH = REPOSITORY_ROOT / "render_examples" / "er_all_glyphs.sbgn"
OUTPUT_ROOT = REPOSITORY_ROOT / "output" / "languages"
COMPARISON_PATH = (
    REPOSITORY_ROOT / "output" / "comparisons" / "er_all_glyphs_languages.png"
)
ORIGINAL_PATH = (
    REPOSITORY_ROOT / "output" / "all_figures" / "png" / "appendix_b_reference_card.png"
)
FONT_PATH = REPOSITORY_ROOT / "assets" / "LiberationSans-Regular.ttf"
OUTPUT_FORMATS = ("png", "svg")
PANEL_WIDTH = 1000
PANEL_HEIGHT = 700
PANEL_GAP = 32
LABEL_HEIGHT = 52


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


def compose_comparison() -> None:
    """Combine the original reference card and four renderer outputs.

    Returns:
        None.
    """

    panels = [
        ("Original (Appendix B)", ORIGINAL_PATH),
        ("Python", OUTPUT_ROOT / "python" / "er_all_glyphs.png"),
        ("Rust", OUTPUT_ROOT / "rust" / "er_all_glyphs.png"),
        ("Go", OUTPUT_ROOT / "go" / "er_all_glyphs.png"),
        ("R", OUTPUT_ROOT / "r" / "er_all_glyphs.png"),
    ]
    missing_paths = [str(path) for _, path in panels if not path.is_file()]
    if missing_paths:
        raise FileNotFoundError(
            f"Comparison inputs are missing: {', '.join(missing_paths)}"
        )

    canvas_width = PANEL_GAP * 4 + PANEL_WIDTH * 3
    canvas_height = PANEL_GAP * 3 + PANEL_HEIGHT * 2
    canvas = Image.new("RGB", (canvas_width, canvas_height), "white")
    draw = ImageDraw.Draw(canvas)
    font = ImageFont.truetype(str(FONT_PATH), 32)
    top_x_positions = [
        PANEL_GAP,
        PANEL_GAP * 2 + PANEL_WIDTH,
        PANEL_GAP * 3 + PANEL_WIDTH * 2,
    ]
    bottom_start = (canvas_width - (PANEL_WIDTH * 2 + PANEL_GAP)) // 2
    bottom_x_positions = [bottom_start, bottom_start + PANEL_WIDTH + PANEL_GAP]
    positions = [
        *((x, PANEL_GAP) for x in top_x_positions),
        *((x, PANEL_GAP * 2 + PANEL_HEIGHT) for x in bottom_x_positions),
    ]

    for (label, image_path), (panel_x, panel_y) in zip(panels, positions):
        draw.rounded_rectangle(
            (
                panel_x,
                panel_y,
                panel_x + PANEL_WIDTH,
                panel_y + PANEL_HEIGHT,
            ),
            radius=12,
            fill="white",
            outline="#d0d7de",
            width=2,
        )
        label_box = draw.textbbox((0, 0), label, font=font)
        label_width = label_box[2] - label_box[0]
        draw.text(
            (panel_x + (PANEL_WIDTH - label_width) / 2, panel_y + 10),
            label,
            fill="#24292f",
            font=font,
        )

        with Image.open(image_path) as source_image:
            rendered = source_image.convert("RGBA")
            rendered.thumbnail(
                (PANEL_WIDTH - 32, PANEL_HEIGHT - LABEL_HEIGHT - 24),
                Image.Resampling.LANCZOS,
            )
            flattened = Image.new("RGBA", rendered.size, "white")
            flattened.alpha_composite(rendered)
            image_x = panel_x + (PANEL_WIDTH - rendered.width) // 2
            image_y = (
                panel_y
                + LABEL_HEIGHT
                + (PANEL_HEIGHT - LABEL_HEIGHT - rendered.height) // 2
            )
            canvas.paste(flattened.convert("RGB"), (image_x, image_y))

    COMPARISON_PATH.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(COMPARISON_PATH, optimize=True)


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
        REPOSITORY_ROOT / "rust",
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
    compose_comparison()


if __name__ == "__main__":
    main()
