# Development guide

## Commands

```bash
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo --manifest-path rust/Cargo.toml run --release -- draw_sbgnml -i examples/reference_card.sbgn -o output/reference_card.svg
cargo --manifest-path rust/Cargo.toml run --release -- draw_sbgnml -i render_examples/er_all_glyphs.sbgn -o output/er_all_glyphs.svg
uv run python scripts/render_all_figures.py
```

For changes to the Go renderer, run from `go/`:

```bash
gofmt -w main.go main_test.go
go test ./...
go run . draw_sbgnml --input-path ../examples/figure_1_2.sbgn --output-path ../output/figure_1_2.png
go run . draw_sbgnml --input-path ../examples/figure_1_2.sbgn --output-path ../output/figure_1_2.svg
```

## Architecture and constraints

- `rust/src/main.rs` contains the CLI, SBGN-ML parser, geometry, and PNG/SVG backends.
- Keep the renderer deterministic and independent of browser or system graphics libraries.
- Preserve SBGN-ML coordinates, arc bend points, arc groups, and nested glyph parentage.
- Use the embedded Liberation Sans asset; generated SVG declares Arial first and Liberation Sans as fallback.
- Keep fixtures in `examples/` valid against the LibSBGN milestone 3 schema.
- Keep `figures/catalog.json` in specification order and at 69 entries: 68
  numbered figures plus the Appendix B reference card.
- Do not replace the exact upstream SVG/PDF figure sources with screenshots.
- Keep figure-review status in the requested CSV tracker. Do not mark SBGN-ML
  or rendered figures complete until the reviewer explicitly confirms them.

## ER rendering conventions

- Pure ER rendering must never synthesize solid triangular arrowheads. A plain
  `interaction` arc has no terminal marker; use explicit `assignment` arcs
  wherever filled recessed-tail barbs are intended. Do not infer PD-style
  production triangles from entity endpoints.
- Assignment arcs use filled barbed arrowheads with a recessed tail. When both
  ends require assignment barbs, encode both directed arcs unless the format
  provides an explicit bidirectional construct.
- An influence targeting another arc must terminate with its tip touching the
  target stroke while its marker body remains on the approach side.
- Absolute inhibition uses two parallel tee bars joined by a centered
  orthogonal segment. Leave one inter-bar spacing between the target stroke and
  the front bar, use the same spacing between bars, and stop the incoming stem
  at the rear bar.
- Suppress assignment markers at invisible merge glyphs such as `implicit xor`;
  converging assignment branches must not create a barb cluster at the merge.
- Render square standalone state variables as circles. Render non-square state
  variables as stadium/pill shapes, not ellipses. Nested ordinary state
  variables also use stadium shapes.
- Render existence state variables as half-filled circles.
- Render location state variables as circles with a centered 45-degree
  diagonal and an opposite-slope chord offset by one-third of the radius. The
  centered diagonal stops where it meets the offset chord rather than crossing
  through it.
- When truncating a centered location-glyph diagonal at its chord, test that the
  shortened segment remains collinear with the circle center; its midpoint is
  no longer the circle center and must not be used as the assertion.
- Include detached-looking nested auxiliary glyphs in fitted canvas bounds so
  state variables are not clipped.

## Testing

- Add parser coverage for every newly accepted SBGN-ML construct.
- Add marker mapping tests for every new arc class.
- Render both PNG and SVG for visual changes and inspect representative outputs.
- For visual marker fixes, test mapping and geometry separately, then inspect
  every requested endpoint in the rendered PNG. A valid XML class or passing
  unit test does not prove the marker is correctly placed.
- For annotated review images, map each red mark to a specific glyph or arc
  endpoint before editing. Do not infer that nearby unmarked markers require
  the same change.
