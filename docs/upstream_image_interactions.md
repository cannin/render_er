# Connected upstream image reconstructions

The source set is the `images/` directory at
[`sbgn/entity-relationships`](https://github.com/sbgn/entity-relationships/tree/master/images),
snapshot commit `1251c4a24caf7dccf57fa0a90d0e9b40a5a6a14b`.

An image qualifies when it depicts at least two semantic SBGN nodes joined by
at least one complete relationship. The selected figures are assignment,
asynchronous, entity granularity, entity identity, interaction, logic arc,
nesting existence, nesting location, the Appendix B reference card,
simultaneous, and synchronous.

Single-glyph definitions, dangling marker examples, and layout-rule fragments
do not qualify. `self` has only one semantic node. `non-interaction` explicitly
depicts the absence of an interaction and is obsolete in the current ER
specification. `synchronous-PD` is a Process Description diagram and cannot be
represented as schema-valid Entity Relationship SBGN-ML.

Schema-valid reconstructions are stored in `examples/upstream_images/`.
Each result directory beneath `output/upstream_image_interactions/` contains
the source-faithful original, PNG and SVG output from Python, Rust, Go, and R,
and a five-panel comparison. Open
[`output/upstream_image_interactions/index.html`](../output/upstream_image_interactions/index.html)
to browse the complete set.

Generation also requests a structural manifest from every renderer and stops
if any SBGN glyph or arc is absent. This guarantees that all nodes and
relationships appear in every language output even where styling and layout
still differ from the upstream artwork.

Regenerate everything with:

```bash
./scripts/render_upstream_image_interactions.py
```
