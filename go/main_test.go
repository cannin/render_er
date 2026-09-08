package main

import (
	"math"
	"strings"
	"testing"
)

// TestPortedGlyphCoreRectMatchesAFPDLogicalSize verifies all logical operators
// use the same 21-unit core size established by the AF and PD renderings.
func TestPortedGlyphCoreRectMatchesAFPDLogicalSize(t *testing.T) {
	transform := &Transform{ScaleX: 2.0, ScaleY: 3.0}
	outer := PixelRect{
		X0: 10, Y0: 20, Width: 168, Height: 168,
		Center: Point{X: 94, Y: 104},
	}

	for _, className := range []string{"and", "or", "not", "delay"} {
		t.Run(className, func(t *testing.T) {
			core := portedGlyphCoreRect(
				outer,
				&Glyph{ClassName: className},
				transform,
			)
			if math.Abs(core.Width-42.0) > 1e-9 || math.Abs(core.Height-42.0) > 1e-9 {
				t.Fatalf("logical core size = %gx%g, want 42x42 rendered pixels", core.Width, core.Height)
			}
			if core.Center != outer.Center {
				t.Fatalf("logical core center = %#v, want %#v", core.Center, outer.Center)
			}
			if !isPortedGlyphClass(className) || !isCytoscapePortedClass(className) {
				t.Fatalf("logical class %q is not port-aware", className)
			}
		})
	}
}

// TestPortedLogicalGlyphWithoutPortsRetainsStubs verifies an explicit arc that
// ends at the source bbox remains joined to the smaller logical core.
func TestPortedLogicalGlyphWithoutPortsRetainsStubs(t *testing.T) {
	transform := &Transform{ScaleX: 1.0, ScaleY: 1.0}
	outer := PixelRect{
		X0: 10, Y0: 20, Width: 60, Height: 60,
		Center: Point{X: 40, Y: 50},
	}

	bounds := portedGlyphPath(
		outer,
		&Glyph{ClassName: "and"},
		transform,
	).Bounds()
	if math.Abs(bounds.X0-outer.X0) > 1e-9 || math.Abs(bounds.X1-(outer.X0+outer.Width)) > 1e-9 {
		t.Fatalf("logical path horizontal bounds = [%g, %g], want [%g, %g]", bounds.X0, bounds.X1, outer.X0, outer.X0+outer.Width)
	}
}

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

// TestJSArcRenderPointsClipsInteriorEndpoints verifies ER relationships meet
// their connecting symbols at the border rather than the center.
func TestJSArcRenderPointsClipsInteriorEndpoints(t *testing.T) {
	source := &Glyph{ID: "source", ClassName: "entity", BBox: &BBox{X: 0, Y: 0, W: 20, H: 20}}
	target := &Glyph{ID: "target", ClassName: "entity", BBox: &BBox{X: 80, Y: 0, W: 20, H: 20}}
	arc := Arc{
		ID: "arc", ClassName: "interaction", Source: "source", Target: "target",
		Points: []Point{{X: 10, Y: 10}, {X: 90, Y: 10}},
	}

	points, _, _, ok := jsArcRenderPoints(
		arc,
		map[string]*Glyph{"source": source, "target": target},
		map[string]string{},
	)
	if !ok {
		t.Fatal("jsArcRenderPoints() rejected an explicit ER path")
	}
	if points[0] != (Point{X: 20, Y: 10}) || points[1] != (Point{X: 80, Y: 10}) {
		t.Fatalf("clipped points = %#v", points)
	}
}

// TestJSArcRenderPointsClipsNestedAuxiliaryEndpoint verifies connections stop
// at an ER auxiliary symbol that overlaps its referenced parent entity.
func TestJSArcRenderPointsClipsNestedAuxiliaryEndpoint(t *testing.T) {
	value := &Glyph{ID: "value", ClassName: "variable value", BBox: &BBox{X: 0, Y: 30, W: 20, H: 20}}
	entity := &Glyph{ID: "entity", ClassName: "entity", BBox: &BBox{X: 0, Y: 0, W: 20, H: 20}}
	existence := &Glyph{ID: "existence", ParentID: "entity", ClassName: "existence", BBox: &BBox{X: 7, Y: 15, W: 6, H: 10}}
	arc := Arc{
		ID: "arc", ClassName: "assignment", Source: "value", Target: "entity",
		Points: []Point{{X: 10, Y: 30}, {X: 10, Y: 18}},
	}

	points, _, _, ok := jsArcRenderPoints(
		arc,
		map[string]*Glyph{"value": value, "entity": entity, "existence": existence},
		map[string]string{},
	)
	if !ok {
		t.Fatal("jsArcRenderPoints() rejected a nested ER endpoint")
	}
	if points[1] != (Point{X: 10, Y: 25}) {
		t.Fatalf("nested auxiliary endpoint = %#v", points[1])
	}
}

// TestAssignmentUsesBarbedArrow verifies ER assignments use their distinctive
// filled arrowhead with a recessed tail rather than a stimulation triangle.
func TestAssignmentUsesBarbedArrow(t *testing.T) {
	if marker := jsArcMarker("assignment"); marker != "barbed-arrow" {
		t.Fatalf("assignment marker = %q, want barbed-arrow", marker)
	}
}

// TestERInteractionHasNoTriangleMarker verifies pure ER interaction arcs do
// not render the black triangle marker used by other SBGN languages.
func TestERInteractionHasNoTriangleMarker(t *testing.T) {
	arc := Arc{ID: "interaction", ClassName: "interaction", Source: "a", Target: "b"}
	glyphs := map[string]*Glyph{
		"a": {ID: "a", ClassName: "entity"},
		"b": {ID: "b", ClassName: "entity"},
	}
	if marker := jsArcMarkerForEndpoints(arc, glyphs, map[string]string{}); marker != "none" {
		t.Fatalf("ER interaction marker = %q, want none", marker)
	}
}

// TestERInteractionHasNoLegacyTriangleMarkers verifies pure ER interactions
// never receive the filled triangle markers used by other SBGN languages.
func TestERInteractionHasNoLegacyTriangleMarkers(t *testing.T) {
	arc := Arc{ID: "interaction", ClassName: "interaction", Source: "a", Target: "b"}
	glyphs := map[string]*Glyph{
		"a": {ID: "a", ClassName: "entity"},
		"b": {ID: "b", ClassName: "entity"},
	}
	sourceMarker, targetMarker := jsArcEndpointMarkers(arc, glyphs, map[string]string{})
	if sourceMarker != "none" || targetMarker != "none" {
		t.Fatalf("interaction endpoint markers = %q, %q; want none, none", sourceMarker, targetMarker)
	}
}

// TestAbsoluteInhibitionUsesDoubleTee verifies absolute inhibition is not
// rendered with the default hollow stimulation triangle.
func TestAbsoluteInhibitionUsesDoubleTee(t *testing.T) {
	if marker := jsArcMarker("absolute inhibition"); marker != "double-tee" {
		t.Fatalf("absolute inhibition marker = %q, want double-tee", marker)
	}
}

// TestAbsoluteInhibitionOffsetsLeaveEqualGaps verifies the target-to-front-bar
// gap equals the spacing between the two bars.
func TestAbsoluteInhibitionOffsetsLeaveEqualGaps(t *testing.T) {
	front, rear := absoluteInhibitionBarOffsets(10)
	if math.Abs(front-1.2) > 1e-9 || math.Abs((rear-front)-front) > 1e-9 {
		t.Fatalf("absolute inhibition offsets = (%g, %g), want equal 1.2 gaps", front, rear)
	}
}

// TestAbsoluteInhibitionConnectorJoinsBarCenters verifies the short connector
// is orthogonal to both tee bars and spans exactly between their centers.
func TestAbsoluteInhibitionConnectorJoinsBarCenters(t *testing.T) {
	from, to, ok := absoluteInhibitionConnector(Point{X: 30, Y: 40}, Point{X: 30, Y: 10}, 10)
	if !ok {
		t.Fatal("absoluteInhibitionConnector() rejected a nonzero direction")
	}
	if from != (Point{X: 30, Y: 38.8}) || to != (Point{X: 30, Y: 37.6}) {
		t.Fatalf("connector = %#v -> %#v, want (30,38.8) -> (30,37.6)", from, to)
	}
}

// TestComputeBoundsIncludesNestedStateVariables keeps detached-looking state
// variables inside the fitted canvas when their SBGN parent is an entity.
func TestComputeBoundsIncludesNestedStateVariables(t *testing.T) {
	glyphs := []Glyph{
		{ID: "entity", ClassName: "entity", BBox: &BBox{X: 0, Y: 100, W: 100, H: 50}},
		{ID: "value", ParentID: "entity", ClassName: "state variable", BBox: &BBox{X: 20, Y: 0, W: 60, H: 20}},
	}
	bounds, err := computeBounds(glyphs, nil)
	if err != nil {
		t.Fatalf("computeBounds() error = %v", err)
	}
	if bounds.MinY != 0 {
		t.Fatalf("bounds.MinY = %g, want 0", bounds.MinY)
	}
}

// TestInfluenceMarkerTipStopsAtArcTarget verifies a marker targeting an arc
// connection ends at the declared point instead of extending across that arc.
func TestInfluenceMarkerTipStopsAtArcTarget(t *testing.T) {
	arc := Arc{
		ID: "stimulation", ClassName: "stimulation", Source: "outcome", Target: "arc_port",
		Points: []Point{{X: 10, Y: 10}, {X: 30, Y: 10}, {X: 30, Y: 40}},
	}
	point, ok := jsArcMarkerPoint(arc, map[string]*Glyph{}, map[string]string{}, map[string][]PixelRect{})
	if !ok {
		t.Fatal("jsArcMarkerPoint() rejected an explicit arc target")
	}
	if point != (Point{X: 30, Y: 40}) {
		t.Fatalf("marker point = %#v, want the declared arc contact point", point)
	}
}

// TestSquareStandaloneStateVariableUsesCircle verifies compact state values
// remain circular.
func TestSquareStandaloneStateVariableUsesCircle(t *testing.T) {
	glyph := &Glyph{ID: "value", ClassName: "state variable", StateValue: "T", BBox: &BBox{W: 20, H: 20}}
	if shape := jsAuxiliaryShapeType(glyph); shape != "ellipse" {
		t.Fatalf("square standalone state-variable shape = %q, want ellipse", shape)
	}
}

// TestWideStandaloneStateVariableUsesStadium verifies non-circular state
// variables such as PSD use a pill rather than an ellipse.
func TestWideStandaloneStateVariableUsesStadium(t *testing.T) {
	glyph := &Glyph{ID: "value", ClassName: "state variable", StateValue: "PSD", BBox: &BBox{W: 36, H: 20}}
	if shape := jsAuxiliaryShapeType(glyph); shape != "stadium_round_rectangle" {
		t.Fatalf("wide standalone state-variable shape = %q, want stadium_round_rectangle", shape)
	}
}

// TestExistenceIsAuxiliaryEllipse verifies nested existence state variables use
// their circular auxiliary-glyph rendering path.
func TestExistenceIsAuxiliaryEllipse(t *testing.T) {
	glyph := &Glyph{ID: "exists", ParentID: "entity", ClassName: "existence"}
	if !isAuxiliaryGlyphClass(glyph.ClassName) {
		t.Fatal("existence glyph is not classified as auxiliary")
	}
	if shape := jsAuxiliaryShapeType(glyph); shape != "ellipse" {
		t.Fatalf("existence shape = %q, want ellipse", shape)
	}
}

// TestLocationUsesDiameterAndOffsetChord verifies the restored location glyph
// has a centered diagonal plus an opposite-slope chord offset by R/3.
func TestLocationUsesDiameterAndOffsetChord(t *testing.T) {
	rect := PixelRect{X0: 10, Y0: 20, Width: 20, Height: 20, Center: Point{X: 20, Y: 30}}
	lines := locationCrossLines(rect)
	if len(lines) != 2 {
		t.Fatalf("location line count = %d, want 2", len(lines))
	}
	chordMidpoint := Point{X: (lines[1][0].X + lines[1][1].X) / 2, Y: (lines[1][0].Y + lines[1][1].Y) / 2}
	if math.Abs(math.Hypot(chordMidpoint.X-rect.Center.X, chordMidpoint.Y-rect.Center.Y)-rect.Width/6) > 1e-9 {
		t.Fatal("location chord is not offset by radius/3")
	}
	intersection := Point{X: chordMidpoint.X, Y: chordMidpoint.Y}
	if math.Abs(lines[0][0].X-intersection.X) > 1e-9 || math.Abs(lines[0][0].Y-intersection.Y) > 1e-9 {
		t.Fatalf("centered diagonal starts at %#v, want chord intersection %#v", lines[0][0], intersection)
	}
	dx := lines[0][1].X - lines[0][0].X
	dy := lines[0][1].Y - lines[0][0].Y
	cross := (rect.Center.X-lines[0][0].X)*dy - (rect.Center.Y-lines[0][0].Y)*dx
	if math.Abs(cross) > 1e-9 {
		t.Fatalf("truncated diagonal does not pass through center: cross product %g", cross)
	}
}

// TestAssignmentToImplicitXORSuppressesMarker verifies converging assignment
// branches do not place barbs at the invisible merge point.
func TestAssignmentToImplicitXORSuppressesMarker(t *testing.T) {
	arc := Arc{ID: "branch", ClassName: "assignment", Source: "value", Target: "merge"}
	glyphs := map[string]*Glyph{
		"value": {ID: "value", ClassName: "state variable"},
		"merge": {ID: "merge", ClassName: "implicit xor"},
	}
	if marker := jsArcMarkerForEndpoints(arc, glyphs, map[string]string{}); marker != "none" {
		t.Fatalf("assignment-to-implicit-xor marker = %q, want none", marker)
	}
}
