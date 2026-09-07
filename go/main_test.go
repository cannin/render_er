package main

import (
	"strings"
	"testing"
)

// TestParseSBGNRetainsERArcGroupNodes verifies grouped interactions and arc
// outcomes are available to the renderer as normal glyph records.
func TestParseSBGNRetainsERArcGroupNodes(t *testing.T) {
	root, err := parseXML(strings.NewReader(`
<sbgn xmlns="http://sbgn.org/libsbgn/0.3">
  <map id="map" language="entity relationship">
    <glyph id="a" class="entity"><bbox x="0" y="0" w="20" h="20"/></glyph>
    <glyph id="b" class="entity"><bbox x="80" y="0" w="20" h="20"/></glyph>
    <arcgroup class="interaction">
      <glyph id="interaction" class="interaction"><bbox x="40" y="0" w="20" h="20"/></glyph>
      <arc id="arc" class="interaction" source="a" target="interaction">
        <glyph id="outcome" class="outcome"><bbox x="25" y="7" w="6" h="6"/></glyph>
        <start x="20" y="10"/><end x="40" y="10"/>
      </arc>
    </arcgroup>
  </map>
</sbgn>`))
	if err != nil {
		t.Fatalf("parseXML() error = %v", err)
	}

	glyphs, arcs, _, err := parseSBGN(root)
	if err != nil {
		t.Fatalf("parseSBGN() error = %v", err)
	}
	if len(glyphs) != 4 {
		t.Fatalf("parseSBGN() glyph count = %d, want 4", len(glyphs))
	}
	if len(arcs) != 1 {
		t.Fatalf("parseSBGN() arc count = %d, want 1", len(arcs))
	}
}

// TestJSArcRenderPointsRetainsAuxiliaryEndpoints verifies explicit ER paths do
// not depend on both endpoint identifiers resolving to standalone glyphs.
func TestJSArcRenderPointsRetainsAuxiliaryEndpoints(t *testing.T) {
	arc := Arc{
		ID:        "arc",
		ClassName: "stimulation",
		Source:    "outcome",
		Target:    "target",
		Points: []Point{
			{X: 10, Y: 10},
			{X: 20, Y: 30},
			{X: 40, Y: 30},
		},
	}

	points, sourceID, targetID, ok := jsArcRenderPoints(
		arc,
		map[string]*Glyph{},
		map[string]string{},
	)
	if !ok {
		t.Fatal("jsArcRenderPoints() rejected an explicit ER path")
	}
	if len(points) != 3 {
		t.Fatalf("jsArcRenderPoints() point count = %d, want 3", len(points))
	}
	if sourceID != "outcome" || targetID != "target" {
		t.Fatalf("resolved endpoints = %q -> %q", sourceID, targetID)
	}
}
