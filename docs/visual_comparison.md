# Visual comparison with SBGN ER Level 1 Version 2

The supplied `sbgn_ER-level1.pdf` was rendered with Poppler and inspected at
120 DPI. The renderer outputs were compared against the following figures:

Side-by-side crops are saved in `output/comparisons/figure_1_1.png`,
`output/comparisons/figure_a9.png`, and `output/comparisons/figure_a10.png`.
The complete Appendix B comparison is saved as
`output/comparisons/er_all_glyphs.png`.

| Renderer example | Specification figure | Comparison result |
| --- | --- | --- |
| `figure_1_1.sbgn` | Figure 1.1 | Entity A state assignment, assignment outcome, stimulation, interaction outcome, and the bidirectional interaction are all present. |
| `figure_a9_pcr.sbgn` | Figure A.9 | The four DNA entities, perturbing agent, existence variables, six outcomes, OR operator, routed arcs, and all terminal marker types agree structurally. |
| `figure_a10_camkii.sbgn` | Figure A.10 | Nested routing, assignment values, state and information units, phenotype, interaction outcomes, and cis/trans cardinalities agree structurally. |
| `reference_card.sbgn` | Appendix B reference card and Figure 2 glyph definitions | Every ER Level 1 glyph and arc class is rendered with its specified shape. |
| `render_examples/er_all_glyphs.sbgn` | Appendix B reference card | All 23 listed notation items are present, plus entity nesting and interaction cardinality. |

The comparison is structural rather than pixel-identical. The PDF uses TeX
fonts and figure-specific scaling, while this renderer embeds Liberation Sans
and preserves the coordinates stored in SBGN-ML. The generated figures use
black one-color notation, white fills, rounded entity corners, and orthogonal
polyline bends in line with the specification.

The comparison found and drove fixes for differences inherited from the
upstream renderer: barbed assignment arrowheads, interaction arrowheads at
entity endpoints, arc-group
interaction nodes, circular existence variables with the right semicircle
filled, and multi-segment arc routing. It also added the circular crossed
location symbol and the two absolute influence markers. Both circular auxiliary
glyphs are inscribed in their SBGN-ML bounding boxes so non-square boxes cannot
distort them into ellipses.
