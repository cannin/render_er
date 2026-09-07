#!/usr/bin/env python3
"""Render every figure from the SBGN ER Level 1 specification source."""

import argparse
import csv
import html
import json
import re
import shutil
import subprocess
from pathlib import Path

REPOSITORY_URL = "https://github.com/sbgn/entity-relationships.git"
SOURCE_WEB_ROOT = "https://github.com/sbgn/entity-relationships/blob/master"
COLOR_PATTERN = re.compile(r"#[0-9a-fA-F]{6}")
NEUTRAL_COLORS = {
    "#000000",
    "#ffffff",
    "#fafafa",
    "#969696",
    "#646464",
    "#666666",
    "#000003",
}


def run(command: list[str], cwd: Path) -> None:
    """Run one required subprocess.

    Args:
        command: Command and arguments to execute.
        cwd: Working directory for the command.
    """
    subprocess.run(command, cwd=cwd, check=True)


def prepare_source(source_path: Path, destination: Path) -> None:
    """Copy an SVG source or convert a PDF source to SVG.

    Args:
        source_path: Upstream SVG or PDF figure.
        destination: Stable SVG destination used by the renderer.
    """
    destination.parent.mkdir(parents=True, exist_ok=True)
    if source_path.suffix.lower() == ".svg":
        shutil.copy2(source_path, destination)
        return
    if source_path.suffix.lower() == ".pdf":
        run(["pdftocairo", "-svg", str(source_path), str(destination)], destination.parent)
        return
    raise ValueError(f"Unsupported source format: {source_path}")


def source_colors(svg_path: Path) -> list[str]:
    """Return non-neutral hexadecimal colors used by an SVG.

    Args:
        svg_path: SVG file to inspect.

    Returns:
        Sorted lowercase colors other than black, white, and neutral grays.
    """
    source = svg_path.read_text(encoding="utf-8", errors="replace")
    colors = {color.lower() for color in COLOR_PATTERN.findall(source)}
    return sorted(colors - NEUTRAL_COLORS)


def write_index(rows: list[dict[str, str]], output_dir: Path) -> None:
    """Write a browsable HTML index for all rendered figures.

    Args:
        rows: Rendered figure metadata.
        output_dir: Catalog output directory.
    """
    cards = []
    for row in rows:
        cards.append(
            f"""
<article>
  <h2>Figure {html.escape(row['figure'])}</h2>
  <p>{html.escape(row['title'])}</p>
  <a href="png/{row['slug']}.png"><img src="png/{row['slug']}.png" alt="Figure {html.escape(row['figure'])}"></a>
  <p><a href="svg/{row['slug']}.svg">SVG</a> | <a href="{html.escape(row['source_url'])}">upstream source</a></p>
</article>"""
        )
    document = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>SBGN ER Level 1 figure renderings</title>
<style>
body {{ font-family: Arial, 'Liberation Sans', sans-serif; margin: 2rem; color: #111; }}
main {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 1rem; }}
article {{ border: 1px solid #bbb; border-radius: 8px; padding: 1rem; }}
h1 {{ margin-bottom: 0.25rem; }} h2 {{ margin: 0; font-size: 1.05rem; }}
img {{ width: 100%; height: 260px; object-fit: contain; background: white; }}
p {{ margin: 0.5rem 0; }}
</style>
</head>
<body>
<h1>SBGN Entity Relationship Level 1 figures</h1>
<p>All 68 numbered figures plus the Appendix B reference card. Original colors are preserved.</p>
<main>{''.join(cards)}</main>
</body>
</html>
"""
    (output_dir / "index.html").write_text(document, encoding="utf-8")


def write_contact_sheets(png_paths: list[Path], output_dir: Path, cwd: Path) -> None:
    """Create compact visual QA sheets with ImageMagick when available.

    Args:
        png_paths: Figure PNG paths in specification order.
        output_dir: Catalog output directory.
        cwd: Project working directory.
    """
    magick = shutil.which("magick")
    if magick is None:
        return
    contact_dir = output_dir / "contact_sheets"
    contact_dir.mkdir(parents=True, exist_ok=True)
    for page_index, start in enumerate(range(0, len(png_paths), 12), start=1):
        page_paths = png_paths[start : start + 12]
        command = [
            magick,
            "montage",
            "-label",
            "%t",
            *[str(path) for path in page_paths],
            "-thumbnail",
            "360x250",
            "-tile",
            "4x3",
            "-geometry",
            "+16+32",
            str(contact_dir / f"overview_{page_index:02d}.png"),
        ]
        run(command, cwd)


def parse_args() -> argparse.Namespace:
    """Parse command-line arguments.

    Returns:
        Parsed arguments.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source-dir",
        type=Path,
        default=Path("tmp/entity-relationships"),
        help="Checkout of sbgn/entity-relationships",
    )
    parser.add_argument(
        "--scale",
        type=float,
        default=2.0,
        help="Raster scale used by the Rust SVG renderer",
    )
    return parser.parse_args()


def main() -> None:
    """Build the complete source-faithful figure catalog."""
    args = parse_args()
    project_dir = Path(__file__).resolve().parents[1]
    source_dir = args.source_dir
    if not source_dir.is_absolute():
        source_dir = project_dir / source_dir
    if not source_dir.exists():
        source_dir.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "clone", "--depth", "1", REPOSITORY_URL, str(source_dir)], project_dir)

    catalog = json.loads((project_dir / "figures/catalog.json").read_text(encoding="utf-8"))
    output_dir = project_dir / "output/all_figures"
    svg_dir = output_dir / "svg"
    png_dir = output_dir / "png"
    svg_dir.mkdir(parents=True, exist_ok=True)
    png_dir.mkdir(parents=True, exist_ok=True)

    run(["cargo", "build", "--release"], project_dir)
    renderer = project_dir / "target/release/render_er"
    rows: list[dict[str, str]] = []
    png_paths: list[Path] = []

    for entry in catalog:
        source_path = source_dir / entry["source"]
        if not source_path.exists():
            raise FileNotFoundError(source_path)
        svg_path = svg_dir / f"{entry['slug']}.svg"
        png_path = png_dir / f"{entry['slug']}.png"
        prepare_source(source_path, svg_path)
        run(
            [
                str(renderer),
                "draw_svg",
                "--input-path",
                str(svg_path),
                "--output-path",
                str(png_path),
                "--padding",
                "12",
                "--scale",
                str(args.scale),
            ],
            project_dir,
        )
        colors = source_colors(svg_path)
        rows.append(
            {
                "figure": entry["figure"],
                "slug": entry["slug"],
                "title": entry["title"],
                "source": entry["source"],
                "source_url": f"{SOURCE_WEB_ROOT}/{entry['source']}",
                "colors": ";".join(colors),
            }
        )
        png_paths.append(png_path)

    with (output_dir / "manifest.csv").open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    write_index(rows, output_dir)
    write_contact_sheets(png_paths, output_dir, project_dir)
    print(f"Rendered {len(rows)} figures to {output_dir}")


if __name__ == "__main__":
    main()
