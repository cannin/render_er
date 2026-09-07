# Development guide

## Commands

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run --release -- draw_sbgnml -i examples/reference_card.sbgn -o output/reference_card.svg
cargo run --release -- draw_sbgnml -i render_examples/er_all_glyphs.sbgn -o output/er_all_glyphs.svg
uv run python scripts/render_all_figures.py
```

## Architecture and constraints

- `src/main.rs` contains the CLI, SBGN-ML parser, geometry, and PNG/SVG backends.
- Keep the renderer deterministic and independent of browser or system graphics libraries.
- Preserve SBGN-ML coordinates, arc bend points, arc groups, and nested glyph parentage.
- Use the embedded Liberation Sans asset; generated SVG declares Arial first and Liberation Sans as fallback.
- Keep fixtures in `examples/` valid against the LibSBGN milestone 3 schema.
- Keep `figures/catalog.json` in specification order and at 69 entries: 68
  numbered figures plus the Appendix B reference card.
- Do not replace the exact upstream SVG/PDF figure sources with screenshots.

## Testing

- Add parser coverage for every newly accepted SBGN-ML construct.
- Add marker mapping tests for every new arc class.
- Render both PNG and SVG for visual changes and inspect representative outputs.
