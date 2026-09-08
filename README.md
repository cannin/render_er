# render_er

Render SBGN Entity Relationship (ER) Level 1 maps to PNG or SVG with four
independent native implementations: [Python](python/), [Rust](rust/),
[Go](go/), and [R](r/). The implementations share fixtures and parity tests so
their observable rendering behavior stays aligned.

The Rust implementation also provides a source-faithful SVG-to-PNG path used to
reproduce every figure in the ER Level 1 specification while retaining source
colors. The rendering engines are adapted from the MIT-licensed implementations
in [`cannin/render_sbgn`](https://github.com/cannin/render_sbgn). ER parsing and
rendering support includes arc groups, nested entities, outcomes attached to
statement arcs, arc ports, bend points, and every ER Level 1 influence marker.

## Quick start

### Go

Render PNG and SVG directly from the repository root:

```bash
(cd go && go run . draw_sbgnml \
  --input-path ../examples/figure_1_1.sbgn \
  --output-path ../output/figure_1_1-go.png)

(cd go && go run . draw_sbgnml \
  --input-path ../examples/figure_1_1.sbgn \
  --output-path ../output/figure_1_1-go.svg)
```

### Rust

```bash
cargo run --manifest-path rust/Cargo.toml --release -- draw_sbgnml \
  --input-path examples/figure_1_1.sbgn \
  --output-path output/figure_1_1-rust.svg
```

### Python

```bash
uv run --project python render_sbgn_py draw_sbgnml \
  --input-path examples/figure_1_1.sbgn \
  --output-path output/figure_1_1-python.png
```

### R

```bash
Rscript r/draw_sbgnml.R \
  --input-path examples/figure_1_1.sbgn \
  --output-path output/figure_1_1-r.png
```

An explicit `.png` or `.svg` output path selects that format. Without an output
path, the renderers write both formats beside the input. Each implementation
also accepts `--help` for all sizing, styling, and manifest options.

## Complete specification figure set

Render all 68 numbered figures and the Appendix B reference card from the
official `sbgn/entity-relationships` sources:

```bash
uv run python scripts/render_all_figures.py
```

The script clones the source repository into `tmp/` when needed, converts its
three PDF-only assets to SVG, builds the Rust renderer, and sends all 69 figures
through `render_er draw_svg`. Original red, blue, amber, and other source colors
are preserved. Results are written to:

- `output/all_figures/png/` — tightly cropped PNG renderings
- `output/all_figures/svg/` — original or PDF-converted SVG sources
- `output/all_figures/index.html` — browsable figure gallery
- `output/all_figures/manifest.csv` — figure-to-source mapping and color report
- `output/all_figures/contact_sheets/` — six visual QA overviews

The complete figure mapping and validation notes are in
[`docs/all_figures.md`](docs/all_figures.md).

Render one source SVG directly with Rust:

```bash
cargo run --manifest-path rust/Cargo.toml --release -- draw_svg \
  --input-path figure.svg \
  --output-path figure.png \
  --scale 2
```

## Supported ER notation

- Interactors: entity, nested entity, outcome, and perturbing agent
- Logical operators: and, or, not, and delay
- Statements: assignment, binary or n-ary interaction, and phenotype
- Auxiliary units: unit of information, state variable, variable value,
  existence, location, and cardinality
- Influences: modulation, stimulation, necessary stimulation, absolute
  stimulation, inhibition, absolute inhibition, and logic arc
- Annotation callouts

## SBGN-ML compatibility

No private XML vocabulary is needed. LibSBGN milestone 3 (`0.3`) already defines
all ER Level 1 Version 2 glyph and arc classes, including `variable value`,
`delay`, `existence`, `location`, and the absolute influence classes. Both of
these map declarations are accepted:

```xml
<map language="entity relationship" id="map1">
```

```xml
<map version="http://identifiers.org/combine.specifications/sbgn.er.level-1.version-2"
     id="map1">
```

An explicit non-ER `language` is rejected. Namespace-qualified SBGN-ML 0.2 and
0.3 files are parsed by local element name.

## Examples and visual comparison

- `render_examples/er_all_glyphs.sbgn` exercises every ER Level 1 reference-card
  glyph and arc class in one schema-valid diagram. Its 23-of-23 coverage table
  is in [`docs/er_all_glyphs_coverage.md`](docs/er_all_glyphs_coverage.md).
- `figure_1_1.sbgn` reconstructs the introductory assignment-stimulates-
  interaction diagram.
- `figure_a9_pcr.sbgn` corresponds to PDF Figure A.9.
- `figure_a10_camkii.sbgn` corresponds to PDF Figure A.10.
- `reference_card.sbgn` exercises the full ER symbol vocabulary.
- `nested_entity.sbgn` demonstrates recursive entity containment.

Rendered PNG and SVG files are in `output/`. The comparison method and results
for hand-authored SBGN-ML examples are documented in
[`docs/visual_comparison.md`](docs/visual_comparison.md). The separate complete
catalog uses the exact figure assets referenced by the specification's TeX
source; only `examples/rtk.sbgn` is supplied upstream as structured SBGN-ML.

## Cross-language renders

The Go, R, and Python renderer baselines imported from `render_sbgn` remain
independent implementations alongside the native Rust renderer. Generate PNG
and SVG versions of the ER all-glyphs fixture with all four languages:

```bash
./scripts/render_language_examples.py
```

The results are written below `output/languages/{rust,go,r,python}/`. Python
uses the same `pycairo` dependency as `render_sbgn`; R uses the upstream base-R
graphics implementation and therefore does not require the R Cairo package.

![Original ER reference card and Rust, Go, R, and Python renders](output/comparisons/er_all_glyphs_languages.png)

Connected diagrams reconstructed from the specification's upstream `images/`
directory are available in the
[four-language comparison gallery](output/upstream_image_interactions/index.html).
The selection criteria and exclusions are documented in
[`docs/upstream_image_interactions.md`](docs/upstream_image_interactions.md).

## Verification

Run the coordinated version, native test, CLI-parity, and cross-language
conformance checks:

```bash
./scripts/test-all.sh
```

Individual Rust checks remain available:

```bash
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path rust/Cargo.toml
```

## Releases

Every implementation uses the same semantic version. Verify the version and
complete test suite before tagging:

```bash
./scripts/check-versions.sh X.Y.Z
./scripts/test-all.sh
```

A release requires annotated `vX.Y.Z` and `go/vX.Y.Z` tags on the same commit.
Push both tags together; the `vX.Y.Z` tag starts the GitHub Actions release:

```bash
git tag -a vX.Y.Z -m "render_er X.Y.Z"
git tag -a go/vX.Y.Z -m "render_er Go X.Y.Z"
git push origin vX.Y.Z go/vX.Y.Z
```

The workflow publishes source archives, a Python wheel, a checked R source
package, Go binaries for Linux, macOS, and Windows on amd64 and arm64, and Rust
binaries for Linux amd64, macOS arm64, and Windows amd64. It also publishes a
SHA-256 checksum manifest for every asset.

To validate an example against the official LibSBGN schema:

```bash
xmllint --noout --schema /path/to/libsbgn/resources/SBGN.xsd \
  examples/reference_card.sbgn
```
