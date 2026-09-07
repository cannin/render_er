# Complete ER Level 1 figure catalog

`output/all_figures/` contains a source-faithful rendering of every figure in
`sbgn_ER-level1.pdf`: Figures 1.1-1.2, 2.1-2.47, 3.1-3.2, 4.1-4.7, A.1-A.10,
and the unnumbered Appendix B reference card. This is 69 artifacts in total.

## Sources and rendering path

`figures/catalog.json` maps each specification figure to the exact asset used by
the specification's TeX source in
[`sbgn/entity-relationships`](https://github.com/sbgn/entity-relationships/tree/master/examples).
The catalog includes assets from both the upstream `examples/` and `images/`
directories.

Most inputs are SVG. The three PDF-only inputs (`HelloWorld.pdf`,
`layout-edge-edge.pdf`, and `rtk-complex.pdf`) are first converted to SVG with
Poppler. The Rust `draw_svg` command then parses, crops, scales, and rasterizes
each SVG with `resvg`. It uses a white background, adds 12 pixels of padding,
and preserves source fills and strokes, including the red example annotations,
the blue receptor-kinase series, and amber backgrounds.

This exact-source path complements the structured SBGN-ML renderer. The
upstream figure repository provides only one of these examples as SBGN-ML
(`examples/rtk.sbgn`), so the SVG/PDF sources are retained instead of claiming
lossy reverse-engineered SBGN-ML for the remaining figures.

## Rebuild

From the repository root:

```bash
uv run python scripts/render_all_figures.py
```

Use an existing upstream checkout if desired:

```bash
uv run python scripts/render_all_figures.py \
  --source-dir /path/to/entity-relationships \
  --scale 2
```

The output includes a browsable `index.html`, a CSV manifest containing each
source URL and non-neutral hexadecimal colors, and six contact sheets for rapid
visual review.

## Validation

All 69 PNGs were checked for successful decoding, nonzero dimensions, and
reasonable canvas sizes. The six contact sheets were visually inspected.
Representative comparisons against the supplied PDF included the red Figure
2.12 perturbing-agent example, Figure 4.1 layout guidance, blue Figure A.7, and
the Appendix B reference card. Their structure, text, line work, and colors
match the source figures embedded in the PDF.
