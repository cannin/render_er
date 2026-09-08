# ER all-glyph coverage

`render_examples/er_all_glyphs.sbgn` is a schema-valid SBGN-ML 0.3 Entity
Relationship map that exercises every visible item listed on the Appendix B
reference card in `sbgn_ER-level1.pdf`.

## Reference-card comparison

| PDF category | PDF listing | SBGN-ML representation | Example ID | Included |
| --- | --- | --- | --- | --- |
| Interactors | entity | `glyph class="entity"` | `entity_basic` | Yes |
| Interactors | outcome | `glyph class="outcome"` on an arc | `interaction_outcome` | Yes |
| Interactors | perturbing agent | `glyph class="perturbing agent"` | `perturbing_agent` | Yes |
| Logical operators | and operator | `glyph class="and"` | `logic_and` | Yes |
| Logical operators | or operator | `glyph class="or"` | `logic_or` | Yes |
| Logical operators | not operator | `glyph class="not"` | `logic_not` | Yes |
| Logical operators | delay operator | `glyph class="delay"` | `logic_delay` | Yes |
| Auxiliary units | unit of information | `glyph class="unit of information"` | `information_unit` | Yes |
| Auxiliary units | state variable | `glyph class="state variable"` | `state_variable` | Yes |
| Auxiliary units | existence | `glyph class="existence"` | `existence` | Yes |
| Auxiliary units | location | `glyph class="location"` | `location` | Yes |
| Auxiliary units | variable value | `glyph class="variable value"` | `variable_value` | Yes |
| Statements | assignment | `arc class="assignment"` | `assignment` | Yes |
| Statements | interaction | interaction arc group, glyph, and arcs | `interaction_node` | Yes |
| Statements | phenotype | `glyph class="phenotype"` | `phenotype` | Yes |
| Influence | modulation | `arc class="modulation"` | `modulation` | Yes |
| Influence | stimulation | `arc class="stimulation"` | `stimulation` | Yes |
| Influence | necessary stimulation | `arc class="necessary stimulation"` | `necessary_stimulation` | Yes |
| Influence | absolute stimulation | `arc class="absolute stimulation"` | `absolute_stimulation` | Yes |
| Influence | inhibition | `arc class="inhibition"` | `inhibition` | Yes |
| Influence | absolute inhibition | `arc class="absolute inhibition"` | `absolute_inhibition` | Yes |
| Influence | logic arc | `arc class="logic arc"` | `logic_arc` | Yes |
| Reference nodes | annotation | `glyph class="annotation"` | `annotation` | Yes |

Coverage is 23 of 23 Appendix B entries. The map also includes the interaction
cardinality decoration (`interaction_cardinality`) and an entity nested inside
another entity (`entity_nested_child`). These exercise notation shown elsewhere
in the Level 1 specification but not listed as independent Appendix B rows.

## Validation and rendering

Validate against the official LibSBGN milestone-3 schema:

```bash
xmllint --noout --schema /path/to/libsbgn/resources/SBGN.xsd \
  render_examples/er_all_glyphs.sbgn
```

Render both output formats:

```bash
cargo run --manifest-path rust/Cargo.toml --release -- draw_sbgnml \
  --input-path render_examples/er_all_glyphs.sbgn \
  --output-path output/er_all_glyphs.png

cargo run --manifest-path rust/Cargo.toml --release -- draw_sbgnml \
  --input-path render_examples/er_all_glyphs.sbgn \
  --output-path output/er_all_glyphs.svg
```
