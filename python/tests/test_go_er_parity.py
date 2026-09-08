"""Parity tests derived from the current Go ER renderer behavior."""

from pathlib import Path
import io
import tempfile
import unittest
from unittest.mock import patch

import cairo

from render_sbgn_py.cli import parse_args
from render_sbgn_py.renderer import (
    Arc,
    BBox,
    Bounds,
    Glyph,
    PixelRect,
    Point,
    Port,
    Transform,
    absolute_inhibition_bar_offsets,
    absolute_inhibition_connector,
    auxiliary_glyph_shape,
    compute_bounds,
    draw_js_arc_marker,
    draw_sbgnml,
    is_auxiliary_glyph_class,
    is_ported_glyph_class,
    js_arc_endpoint_markers,
    js_arc_marker,
    js_arc_marker_point,
    js_arc_path,
    location_cross_lines,
    parse_render_information,
    parse_sbgnml,
    path_ported_glyph,
    ported_glyph_core_rect,
    render_output_paths,
    sbgnml_basic_render_manifest,
)


def glyph(glyph_id, class_name, bbox=None, parent_id=None, ports=None):
    return Glyph(
        id=glyph_id,
        parent_id=parent_id,
        class_name=class_name,
        bbox=bbox,
        extra_width=None,
        extra_height=None,
        label="",
        ports=ports or [],
        has_clone=False,
        state_value=None,
        state_variable=None,
        orientation=None,
    )


class GoERParityTests(unittest.TestCase):
    def test_cli_requires_input_and_accepts_go_aliases(self):
        with (
            patch("sys.argv", ["render_sbgn_py", "draw_sbgnml"]),
            patch("sys.stderr", new=io.StringIO()),
        ):
            with self.assertRaises(SystemExit):
                parse_args()
        with patch(
            "sys.argv",
            [
                "render_sbgn_py",
                "draw_sbgnml",
                "--input",
                "diagram.sbgn",
                "--output",
                "diagram.svg",
            ],
        ):
            args = parse_args()
        self.assertEqual(args.input_path, "diagram.sbgn")
        self.assertEqual(args.output_path, "diagram.svg")

    def test_cli_output_formats_are_deduplicated(self):
        paths = render_output_paths(Path("diagram.sbgn"), None, "png,svg,png")
        self.assertEqual(
            paths,
            [(Path("diagram.png"), "png"), (Path("diagram.svg"), "svg")],
        )

    def test_logical_operators_use_fixed_core_and_port_stubs(self):
        outer = PixelRect(10, 20, 168, 168, Point(94, 104))
        transform = Transform(0, 0, 2, 3)
        for class_name in ("and", "or", "not", "delay"):
            with self.subTest(class_name=class_name):
                logical = glyph(class_name, class_name)
                core = ported_glyph_core_rect(outer, logical, transform)
                self.assertEqual((core.width, core.height), (42, 42))
                self.assertEqual(core.center, outer.center)
                self.assertTrue(is_ported_glyph_class(class_name))

        surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, 100, 100)
        context = cairo.Context(surface)
        outer = PixelRect(10, 20, 60, 60, Point(40, 50))
        path_ported_glyph(context, outer, glyph("and", "and"), Transform(0, 0, 1, 1))
        x0, _, x1, _ = context.path_extents()
        self.assertAlmostEqual(x0, outer.x0)
        self.assertAlmostEqual(x1, outer.x0 + outer.width)

    def test_arc_group_glyphs_and_outcomes_are_parsed(self):
        xml = """<sbgn xmlns='http://sbgn.org/libsbgn/0.3'>
        <map language='entity relationship'>
          <glyph id='a' class='entity'><bbox x='0' y='0' w='20' h='20'/></glyph>
          <glyph id='b' class='entity'><bbox x='80' y='0' w='20' h='20'/></glyph>
          <arcgroup class='interaction'>
            <glyph id='interaction' class='interaction'>
              <bbox x='40' y='0' w='20' h='20'/>
            </glyph>
            <arc id='arc' class='interaction' source='a' target='interaction'>
              <glyph id='outcome' class='outcome'>
                <bbox x='25' y='7' w='6' h='6'/>
              </glyph>
              <start x='20' y='10'/><end x='40' y='10'/>
            </arc>
          </arcgroup>
        </map></sbgn>"""
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "groups.sbgn"
            path.write_text(xml, encoding="utf-8")
            glyphs, arcs, _ = parse_sbgnml(path)
        self.assertEqual(len(glyphs), 4)
        self.assertEqual(len(arcs), 1)

    def test_standalone_state_variable_circle_or_stadium(self):
        square = glyph("square", "state variable", BBox(0, 0, 20, 20))
        wide = glyph("wide", "state variable", BBox(0, 0, 36, 20))
        self.assertEqual(auxiliary_glyph_shape(square), "ellipse")
        self.assertEqual(auxiliary_glyph_shape(wide), "stadium_round_rectangle")

    def test_nested_auxiliary_is_in_fitted_bounds(self):
        entity = glyph("entity", "entity", BBox(0, 100, 100, 50))
        state = glyph("state", "state variable", BBox(20, 0, 60, 20), "entity")
        self.assertEqual(compute_bounds([entity, state], []).min_y, 0)

    def test_location_uses_truncated_diameter_and_offset_chord(self):
        rect = PixelRect(10, 20, 20, 20, Point(20, 30))
        diameter, chord = location_cross_lines(rect)
        chord_midpoint = Point(
            (chord[0].x + chord[1].x) / 2,
            (chord[0].y + chord[1].y) / 2,
        )
        self.assertAlmostEqual(
            (
                (chord_midpoint.x - rect.center.x) ** 2
                + (chord_midpoint.y - rect.center.y) ** 2
            )
            ** 0.5,
            rect.width / 6,
        )
        self.assertEqual(diameter[0], chord_midpoint)
        dx = diameter[1].x - diameter[0].x
        dy = diameter[1].y - diameter[0].y
        self.assertAlmostEqual(
            (rect.center.x - diameter[0].x) * dy - (rect.center.y - diameter[0].y) * dx,
            0,
        )

    def test_interaction_has_no_endpoint_triangles(self):
        arc = Arc("a", "interaction", "source", "target", [Point(0, 0), Point(10, 0)])
        lookup = {
            "source": glyph("source", "entity"),
            "target": glyph("target", "entity"),
        }
        self.assertEqual(js_arc_endpoint_markers(arc, lookup, {}), ("none", "none"))
        surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, 20, 20)
        context = cairo.Context(surface)
        with patch("render_sbgn_py.renderer.draw_js_marker") as draw_marker:
            draw_js_arc_marker(context, Transform(0, 0, 1, 1), arc, lookup, {})
        draw_marker.assert_not_called()

    def test_er_marker_mapping_matches_go(self):
        expected = {
            "assignment": "barbed-arrow",
            "interaction": "none",
            "absolute inhibition": "double-tee",
            "inhibition": "tee",
            "negative influence": "tee",
            "logic arc": "none",
        }
        for class_name, marker in expected.items():
            with self.subTest(class_name=class_name):
                self.assertEqual(js_arc_marker(class_name), marker)

    def test_absolute_inhibition_geometry(self):
        front, rear = absolute_inhibition_bar_offsets(10)
        self.assertAlmostEqual(front, 1.2)
        self.assertAlmostEqual(rear - front, front)
        connector = absolute_inhibition_connector(Point(30, 40), Point(30, 10), 10)
        self.assertEqual(connector, (Point(30, 38.8), Point(30, 37.6)))

    def test_assignment_to_implicit_xor_suppresses_barb(self):
        arc = Arc("a", "assignment", "value", "merge", [Point(0, 0), Point(10, 0)])
        lookup = {
            "value": glyph("value", "state variable"),
            "merge": glyph("merge", "implicit xor"),
        }
        self.assertEqual(js_arc_endpoint_markers(arc, lookup, {}), ("none", "none"))

    def test_delay_endpoints_snap_to_declared_ports(self):
        delay = glyph(
            "delay",
            "delay",
            BBox(20, 20, 42, 42),
            ports=[Port(62, 41, "delay.in"), Port(20, 41, "delay.out")],
        )
        target = glyph("target", "process", BBox(0, 80, 20, 20))
        lookup = {"delay": delay, "target": target}
        ports = {"delay.in": "delay", "delay.out": "delay"}
        incoming = Arc(
            "in", "logic arc", "target", "delay.in", [Point(10, 90), Point(41, 41)]
        )
        outgoing = Arc(
            "out",
            "necessary stimulation",
            "delay.out",
            "target",
            [Point(41, 41), Point(10, 90)],
        )
        self.assertEqual(js_arc_path(incoming, lookup, ports)[0][-1], Point(62, 41))
        self.assertEqual(js_arc_path(outgoing, lookup, ports)[0][0], Point(20, 41))

    def test_logic_arc_reaches_outcome_center(self):
        source = glyph("source", "state variable", BBox(0, 0, 20, 20))
        outcome = glyph("outcome", "outcome", BBox(40, 30, 20, 20))
        arc = Arc(
            "logic", "logic arc", "source", "outcome", [Point(10, 20), Point(50, 40)]
        )
        self.assertEqual(
            js_arc_path(arc, {"source": source, "outcome": outcome}, {})[0][-1],
            Point(50, 40),
        )

    def test_arc_target_contact_does_not_overshoot(self):
        arc = Arc(
            "stim",
            "stimulation",
            "outcome",
            "arc_port",
            [Point(10, 10), Point(30, 10), Point(30, 40)],
        )
        self.assertEqual(js_arc_marker_point(arc, arc.points, {}, {}), Point(30, 40))

    def test_existence_is_an_auxiliary_ellipse(self):
        existence = glyph("exists", "existence", parent_id="entity")
        self.assertTrue(is_auxiliary_glyph_class(existence.class_name))
        self.assertEqual(auxiliary_glyph_shape(existence), "ellipse")

    def test_render_information_background_is_parsed(self):
        xml = """<sbgn xmlns='http://sbgn.org/libsbgn/0.3'>
        <map language='entity relationship'>
        <extension><renderInformation background-color='#123456'>
        <listOfColorDefinitions>
        <colorDefinition id='accent' value='#abcdef'/></listOfColorDefinitions>
        <listOfStyles><style><g fill='accent' font-size='13'/></style>
        <style idList='e'><g stroke='#010203' stroke-width='2'/></style></listOfStyles>
        </renderInformation></extension>
        <glyph id='e' class='entity'>
          <bbox x='0' y='0' w='20' h='20'/>
        </glyph></map></sbgn>"""
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "render.sbgn"
            path.write_text(xml, encoding="utf-8")
            info = parse_render_information(path)
        self.assertEqual(
            info["background_color"], (0x12 / 255, 0x34 / 255, 0x56 / 255, 1.0)
        )
        self.assertEqual(
            info["colors"]["accent"], (0xAB / 255, 0xCD / 255, 0xEF / 255, 1.0)
        )
        self.assertEqual(info["default_style"]["fill_color"], info["colors"]["accent"])
        self.assertEqual(info["default_style"]["font_size"], 13)
        self.assertEqual(info["styles"]["e"]["stroke_width"], 2)

    def test_render_information_background_is_painted(self):
        xml = """<sbgn xmlns='http://sbgn.org/libsbgn/0.3'>
        <map language='entity relationship'>
        <extension><renderInformation background-color='#123456'/></extension>
        <glyph id='e' class='entity'><bbox x='20' y='20' w='20' h='20'/></glyph>
        </map></sbgn>"""
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / "render.sbgn"
            output = Path(tmp) / "render.png"
            source.write_text(xml, encoding="utf-8")
            draw_sbgnml(source, output, padding=10)
            surface = cairo.ImageSurface.create_from_png(str(output))
            pixel = bytes(surface.get_data()[:4])
        self.assertEqual(pixel, bytes((0x56, 0x34, 0x12, 0xFF)))

    def test_manifest_reports_suppressed_implicit_xor_marker(self):
        xml = """<sbgn xmlns='http://sbgn.org/libsbgn/0.3'>
        <map language='entity relationship'>
        <glyph id='value' class='state variable'>
          <bbox x='0' y='0' w='20' h='20'/>
        </glyph>
        <glyph id='merge' class='implicit xor'>
          <bbox x='40' y='0' w='10' h='10'/>
        </glyph>
        <arc id='branch' class='assignment' source='value' target='merge'>
          <start x='20' y='10'/><end x='45' y='5'/>
        </arc>
        </map></sbgn>"""
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "implicit.sbgn"
            path.write_text(xml)
            glyphs, arcs, bounds = parse_sbgnml(path)
        manifest = sbgnml_basic_render_manifest(glyphs, arcs, bounds, "implicit.sbgn")
        line = next(e for e in manifest["elements"] if e["id"] == "branch::line")
        self.assertEqual(line["marker"], "none")
        self.assertFalse(any(e["id"] == "branch::marker" for e in manifest["elements"]))

    def test_manifest_contains_go_compatible_marker_details(self):
        source = glyph("source", "state variable", BBox(0, 0, 20, 20))
        target = glyph("target", "entity", BBox(40, 0, 20, 20))
        arc = Arc(
            "assign",
            "assignment",
            "source",
            "target",
            [Point(20, 10), Point(40, 10)],
        )
        manifest = sbgnml_basic_render_manifest(
            [source, target],
            [arc],
            Bounds(0, 60, 0, 20),
            "details.sbgn",
            output_width=160,
            output_height=120,
        )
        marker = next(e for e in manifest["elements"] if e["id"] == "assign::marker")
        detail = marker["rendered_detail"]
        self.assertEqual(detail["coordinate_space"], "rendered_pixel")
        self.assertEqual(detail["style"]["fill_mode"], "filled")
        self.assertEqual(detail["drawn_primitives"][0]["shape"], "barbed-arrow")

    def test_manifest_contains_go_compatible_node_details(self):
        complex_glyph = glyph("complex", "complex", BBox(0, 0, 40, 30))
        manifest = sbgnml_basic_render_manifest(
            [complex_glyph],
            [],
            Bounds(0, 40, 0, 30),
            "details.sbgn",
            output_width=140,
            output_height=130,
        )
        shape = next(
            element
            for element in manifest["elements"]
            if element["id"] == "complex::shape"
        )
        detail = shape["rendered_detail"]
        self.assertEqual(detail["coordinate_space"], "rendered_pixel")
        self.assertEqual(detail["drawn_primitives"][0]["shape"], "complex")
        self.assertEqual(len(detail["drawn_primitives"][0]["rendered_points"]), 8)


if __name__ == "__main__":
    unittest.main()
