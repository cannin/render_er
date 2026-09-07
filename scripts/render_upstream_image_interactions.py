#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "pillow>=11.3.0",
# ]
# ///
"""Reconstruct connected upstream ER images and render them in four languages."""

import csv
import html
import json
import os
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Callable

from PIL import Image, ImageDraw, ImageFont

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
UPSTREAM_ROOT = REPOSITORY_ROOT / "tmp" / "entity-relationships"
UPSTREAM_IMAGES = UPSTREAM_ROOT / "images"
EXAMPLE_ROOT = REPOSITORY_ROOT / "examples" / "upstream_images"
OUTPUT_ROOT = REPOSITORY_ROOT / "output" / "upstream_image_interactions"
FONT_PATH = REPOSITORY_ROOT / "assets" / "LiberationSans-Regular.ttf"
SBGN_NAMESPACE = "http://sbgn.org/libsbgn/0.3"
UPSTREAM_URL = "https://github.com/sbgn/entity-relationships"
UPSTREAM_IMAGES_URL = f"{UPSTREAM_URL}/tree/master/images"
UPSTREAM_IMAGE_FILE_URL = f"{UPSTREAM_URL}/blob/master/images"
OUTPUT_FORMATS = ("png", "svg")
PANEL_WIDTH = 1000
PANEL_HEIGHT = 700
PANEL_GAP = 32
LABEL_HEIGHT = 52

ET.register_namespace("", SBGN_NAMESPACE)


def qualified_name(name: str) -> str:
    """Return an SBGN-namespace-qualified XML name.

    Args:
        name: Local XML element name.

    Returns:
        Qualified XML element name.
    """

    return f"{{{SBGN_NAMESPACE}}}{name}"


class DiagramBuilder:
    """Build a small, schema-shaped SBGN Entity Relationship document."""

    def __init__(self, map_id: str) -> None:
        """Initialize one SBGN document.

        Args:
            map_id: XML identifier for the map.
        """

        self.root = ET.Element(qualified_name("sbgn"))
        self.map = ET.SubElement(
            self.root,
            qualified_name("map"),
            {"id": map_id, "language": "entity relationship"},
        )

    def glyph(
        self,
        glyph_id: str,
        class_name: str,
        bbox: tuple[float, float, float, float],
        label: str | None = None,
        parent: ET.Element | None = None,
        state: tuple[str | None, str | None] | None = None,
    ) -> ET.Element:
        """Add a glyph and return it for optional nesting.

        Args:
            glyph_id: Unique glyph identifier.
            class_name: SBGN glyph class.
            bbox: Absolute x, y, width, and height.
            label: Optional glyph label.
            parent: Map, glyph, arc, or arc-group parent.
            state: Optional state-variable value and variable name.

        Returns:
            Newly created glyph element.
        """

        container = parent if parent is not None else self.map
        glyph = ET.SubElement(
            container,
            qualified_name("glyph"),
            {"id": glyph_id, "class": class_name},
        )
        if label is not None:
            ET.SubElement(glyph, qualified_name("label"), {"text": label})
        if state is not None:
            value, variable = state
            attributes = {}
            if value is not None:
                attributes["value"] = value
            if variable is not None:
                attributes["variable"] = variable
            ET.SubElement(glyph, qualified_name("state"), attributes)
        x, y, width, height = bbox
        ET.SubElement(
            glyph,
            qualified_name("bbox"),
            {
                "x": str(x),
                "y": str(y),
                "w": str(width),
                "h": str(height),
            },
        )
        return glyph

    def arc(
        self,
        arc_id: str,
        class_name: str,
        source: str,
        target: str,
        points: list[tuple[float, float]],
        parent: ET.Element | None = None,
        outcome: tuple[str, tuple[float, float, float, float]] | None = None,
        port: tuple[str, float, float] | None = None,
        cardinality: tuple[str, str, tuple[float, float, float, float]] | None = None,
    ) -> ET.Element:
        """Add an arc with optional ER decorations.

        Args:
            arc_id: Unique arc identifier.
            class_name: SBGN arc class.
            source: Source glyph, outcome, or port identifier.
            target: Target glyph, state variable, or port identifier.
            points: Start, optional bend points, and end coordinates.
            parent: Map or arc-group parent.
            outcome: Optional outcome identifier and bounding box.
            port: Optional arc-port identifier and coordinates.
            cardinality: Optional cardinality identifier, text, and bounding box.

        Returns:
            Newly created arc element.
        """

        if len(points) < 2:
            raise ValueError("An arc requires at least a start and end point")
        container = parent if parent is not None else self.map
        arc = ET.SubElement(
            container,
            qualified_name("arc"),
            {
                "id": arc_id,
                "class": class_name,
                "source": source,
                "target": target,
            },
        )
        if outcome is not None:
            outcome_id, bbox = outcome
            self.glyph(outcome_id, "outcome", bbox, parent=arc)
        if cardinality is not None:
            cardinality_id, text, bbox = cardinality
            self.glyph(cardinality_id, "cardinality", bbox, label=text, parent=arc)
        if port is not None:
            port_id, x, y = port
            ET.SubElement(
                arc,
                qualified_name("port"),
                {"id": port_id, "x": str(x), "y": str(y)},
            )
        point_names = ["start", *(["next"] * (len(points) - 2)), "end"]
        for point_name, (x, y) in zip(point_names, points):
            ET.SubElement(
                arc,
                qualified_name(point_name),
                {"x": str(x), "y": str(y)},
            )
        return arc

    def arc_group(self, class_name: str = "interaction") -> ET.Element:
        """Add and return an arc group.

        Args:
            class_name: SBGN arc-group class.

        Returns:
            Newly created arc-group element.
        """

        return ET.SubElement(
            self.map,
            qualified_name("arcgroup"),
            {"class": class_name},
        )

    def write(self, path: Path) -> None:
        """Serialize the diagram as formatted UTF-8 XML.

        Args:
            path: Destination SBGN-ML path.

        Returns:
            None.
        """

        children = list(self.map)
        glyphs = [child for child in children if child.tag == qualified_name("glyph")]
        relationships = [
            child for child in children if child.tag != qualified_name("glyph")
        ]
        self.map[:] = [*glyphs, *relationships]
        path.parent.mkdir(parents=True, exist_ok=True)
        ET.indent(self.root, space="  ")
        tree = ET.ElementTree(self.root)
        tree.write(path, encoding="utf-8", xml_declaration=True)


def add_state_entity(
    diagram: DiagramBuilder,
    entity_id: str,
    label: str,
    bbox: tuple[float, float, float, float],
    state_id: str,
    state_class: str,
    state_bbox: tuple[float, float, float, float],
    state: tuple[str | None, str | None] | None = None,
) -> None:
    """Add an entity carrying a state, existence, or location variable.

    Args:
        diagram: Diagram receiving the glyphs.
        entity_id: Entity identifier.
        label: Entity label.
        bbox: Entity bounding box.
        state_id: Nested state identifier.
        state_class: Nested glyph class.
        state_bbox: Nested glyph bounding box.
        state: Optional value and variable pair.

    Returns:
        None.
    """

    entity = diagram.glyph(entity_id, "entity", bbox, label=label)
    diagram.glyph(
        state_id,
        state_class,
        state_bbox,
        parent=entity,
        state=state,
    )


def build_assignment() -> DiagramBuilder:
    """Reconstruct the single- and multi-value assignment examples."""

    diagram = DiagramBuilder("upstream_assignment")
    diagram.glyph("value", "variable value", (45, 25, 90, 30), label="value")
    add_state_entity(
        diagram,
        "entity_a",
        "Entity A",
        (20, 145, 170, 70),
        "variable_a",
        "state variable",
        (70, 135, 70, 20),
        (None, "variable"),
    )
    diagram.arc(
        "assignment_a",
        "assignment",
        "value",
        "variable_a",
        [(90, 55), (90, 135)],
        outcome=("outcome_a", (84, 91, 12, 12)),
    )
    diagram.glyph("value_1", "variable value", (285, 25, 90, 30), label="value 1")
    diagram.glyph("value_2", "variable value", (415, 25, 90, 30), label="value 2")
    add_state_entity(
        diagram,
        "entity_b",
        "Entity B",
        (300, 145, 190, 70),
        "variable_b",
        "state variable",
        (360, 135, 70, 20),
        (None, "variable"),
    )
    diagram.arc(
        "assignment_b1",
        "assignment",
        "value_1",
        "variable_b",
        [(330, 55), (395, 100), (395, 135)],
        outcome=("outcome_b1", (354, 71, 12, 12)),
    )
    diagram.arc(
        "assignment_b2",
        "assignment",
        "value_2",
        "variable_b",
        [(460, 55), (395, 100), (395, 135)],
        outcome=("outcome_b2", (430, 71, 12, 12)),
    )
    return diagram


def add_consequence_entities(diagram: DiagramBuilder) -> None:
    """Add the three entities and assignments used by synchrony examples."""

    add_state_entity(
        diagram,
        "entity_c",
        "C",
        (230, 20, 180, 80),
        "existence_c",
        "existence",
        (308, 88, 24, 24),
    )
    diagram.glyph("true_c", "variable value", (285, 150, 70, 34), label="T")
    diagram.arc(
        "assignment_c",
        "assignment",
        "true_c",
        "existence_c",
        [(320, 150), (320, 112)],
        outcome=("outcome_c", (312, 124, 16, 16)),
    )
    add_state_entity(
        diagram,
        "entity_a",
        "A",
        (25, 360, 180, 80),
        "existence_a",
        "existence",
        (103, 348, 24, 24),
    )
    diagram.glyph("false_a", "variable value", (80, 260, 70, 34), label="F")
    diagram.arc(
        "assignment_a",
        "assignment",
        "false_a",
        "existence_a",
        [(115, 294), (115, 348)],
        outcome=("outcome_a", (107, 318, 16, 16)),
        port=("assignment_a_target", 115, 318),
    )
    add_state_entity(
        diagram,
        "entity_b",
        "B",
        (435, 360, 180, 80),
        "existence_b",
        "existence",
        (513, 348, 24, 24),
    )
    diagram.glyph("true_b", "variable value", (490, 260, 70, 34), label="T")
    diagram.arc(
        "assignment_b",
        "assignment",
        "true_b",
        "existence_b",
        [(525, 294), (525, 348)],
        outcome=("outcome_b", (517, 318, 16, 16)),
        port=("assignment_b_target", 525, 318),
    )


def build_asynchronous() -> DiagramBuilder:
    """Reconstruct the independent consequence example."""

    diagram = DiagramBuilder("upstream_asynchronous")
    add_consequence_entities(diagram)
    diagram.arc(
        "stimulation_a",
        "stimulation",
        "outcome_c",
        "assignment_a_target",
        [(312, 132), (115, 132), (115, 318)],
    )
    diagram.arc(
        "stimulation_b",
        "stimulation",
        "outcome_c",
        "assignment_b_target",
        [(328, 132), (525, 132), (525, 318)],
    )
    return diagram


def build_synchronous() -> DiagramBuilder:
    """Reconstruct the shared, visually branched consequence example."""

    diagram = DiagramBuilder("upstream_synchronous")
    add_consequence_entities(diagram)
    shared_points = [(320, 140), (320, 220)]
    diagram.arc(
        "stimulation_a",
        "stimulation",
        "outcome_c",
        "assignment_a_target",
        [*shared_points, (115, 220), (115, 318)],
    )
    diagram.arc(
        "stimulation_b",
        "stimulation",
        "outcome_c",
        "assignment_b_target",
        [*shared_points, (525, 220), (525, 318)],
    )
    return diagram


def build_simultaneous() -> DiagramBuilder:
    """Reconstruct one event with multiple influence consequences."""

    diagram = DiagramBuilder("upstream_simultaneous")
    add_state_entity(
        diagram,
        "entity",
        "Entity",
        (190, 20, 180, 80),
        "state",
        "state variable",
        (268, 88, 24, 24),
        (None, "state"),
    )
    diagram.glyph("value", "variable value", (245, 155, 70, 34), label="value")
    diagram.arc(
        "assignment",
        "assignment",
        "value",
        "state",
        [(280, 155), (280, 112)],
        outcome=("event", (272, 126, 16, 16)),
    )
    targets = [
        ("target_a", "Outcome A", (25, 365, 150, 60), "necessary stimulation"),
        ("target_b", "Outcome B", (205, 405, 150, 60), "stimulation"),
        ("target_c", "Outcome C", (385, 365, 150, 60), "inhibition"),
    ]
    for index, (target_id, label, bbox, arc_class) in enumerate(targets):
        diagram.glyph(target_id, "phenotype", bbox, label=label)
        target_x = bbox[0] + bbox[2] / 2
        diagram.arc(
            f"influence_{index}",
            arc_class,
            "event",
            target_id,
            [(280, 142), (280, 260), (target_x, 310), (target_x, bbox[1])],
        )
    return diagram


def build_entity_granularity() -> DiagramBuilder:
    """Reconstruct the two entity-granularity alternatives."""

    diagram = DiagramBuilder("upstream_entity_granularity")
    diagram.glyph("phosphorylated", "variable value", (65, 25, 50, 34), label="P")
    add_state_entity(
        diagram,
        "camkii_state",
        "CaMKII",
        (20, 145, 180, 75),
        "t306",
        "state variable",
        (75, 133, 70, 24),
        (None, "T306"),
    )
    diagram.arc(
        "phosphorylation",
        "assignment",
        "phosphorylated",
        "t306",
        [(90, 59), (90, 133)],
        outcome=("phosphorylation_outcome", (82, 92, 16, 16)),
    )
    diagram.glyph("true", "variable value", (340, 25, 50, 34), label="T")
    add_state_entity(
        diagram,
        "camkii_explicit",
        "CaMKII P@T306",
        (270, 145, 220, 75),
        "exists",
        "existence",
        (368, 133, 24, 24),
    )
    diagram.arc(
        "existence_assignment",
        "assignment",
        "true",
        "exists",
        [(365, 59), (380, 133)],
        outcome=("existence_outcome", (372, 92, 16, 16)),
    )
    return diagram


def build_entity_identity() -> DiagramBuilder:
    """Reconstruct alternate explicit identities for assigned entity states."""

    diagram = DiagramBuilder("upstream_entity_identity")
    diagram.glyph("p", "variable value", (30, 25, 50, 34), label="P")
    add_state_entity(
        diagram,
        "camkii",
        "CaMKII",
        (15, 145, 170, 75),
        "t306",
        "state variable",
        (65, 133, 70, 24),
        (None, "T306"),
    )
    diagram.arc(
        "assignment",
        "assignment",
        "p",
        "t306",
        [(55, 59), (100, 133)],
        outcome=("assigned", (78, 92, 16, 16)),
    )
    diagram.glyph("p306", "entity", (275, 90, 210, 70), label="CaMKII P@T306")
    diagram.glyph("p286", "entity", (275, 205, 210, 70), label="CaMKII P@T286")
    group = diagram.arc_group()
    diagram.glyph(
        "identity_interaction", "interaction", (220, 157, 36, 36), parent=group
    )
    diagram.arc(
        "identity_source",
        "interaction",
        "camkii",
        "identity_interaction",
        [(185, 182), (220, 175)],
        parent=group,
    )
    diagram.arc(
        "identity_306",
        "interaction",
        "identity_interaction",
        "p306",
        [(256, 175), (275, 125)],
        parent=group,
    )
    diagram.arc(
        "identity_286",
        "interaction",
        "identity_interaction",
        "p286",
        [(256, 175), (275, 240)],
        parent=group,
    )
    return diagram


def build_interaction() -> DiagramBuilder:
    """Reconstruct binary, cis, cardinality, and n-ary interactions."""

    diagram = DiagramBuilder("upstream_interaction")
    diagram.glyph("binary_a", "entity", (20, 35, 120, 55), label="A")
    diagram.glyph("binary_b", "entity", (285, 35, 120, 55), label="B")
    diagram.arc(
        "binary",
        "interaction",
        "binary_a",
        "binary_b",
        [(140, 62.5), (285, 62.5)],
        outcome=("binary_outcome", (205, 54.5, 16, 16)),
    )
    diagram.glyph("cis_a", "entity", (520, 35, 120, 55), label="A")
    diagram.arc(
        "cis",
        "interaction",
        "cis_a",
        "cis_a",
        [(550, 90), (550, 135), (610, 135), (610, 90)],
        outcome=("cis_outcome", (572, 127, 16, 16)),
        cardinality=("cis_label", "cis", (570, 102, 40, 18)),
    )
    diagram.glyph("cardinality_a", "entity", (20, 250, 120, 55), label="A")
    diagram.glyph("cardinality_b", "entity", (285, 250, 120, 55), label="B")
    cardinality_group = diagram.arc_group()
    diagram.glyph(
        "cardinality_interaction",
        "interaction",
        (195, 257.5, 40, 40),
        parent=cardinality_group,
    )
    diagram.arc(
        "cardinality_left",
        "interaction",
        "cardinality_a",
        "cardinality_interaction",
        [(140, 277.5), (195, 277.5)],
        parent=cardinality_group,
        cardinality=("cardinality", "2", (155, 254, 24, 18)),
    )
    diagram.arc(
        "cardinality_right",
        "interaction",
        "cardinality_interaction",
        "cardinality_b",
        [(235, 277.5), (285, 277.5)],
        parent=cardinality_group,
        outcome=("cardinality_outcome", (247, 269.5, 16, 16)),
    )
    diagram.glyph("nary_a", "entity", (520, 220, 120, 55), label="A")
    diagram.glyph("nary_b", "entity", (520, 310, 120, 55), label="B")
    diagram.glyph("nary_c", "entity", (760, 265, 120, 55), label="C")
    nary_group = diagram.arc_group()
    diagram.glyph(
        "nary_interaction", "interaction", (685, 262.5, 40, 40), parent=nary_group
    )
    diagram.arc(
        "nary_a_arc",
        "interaction",
        "nary_a",
        "nary_interaction",
        [(640, 247.5), (685, 272.5)],
        parent=nary_group,
    )
    diagram.arc(
        "nary_b_arc",
        "interaction",
        "nary_b",
        "nary_interaction",
        [(640, 337.5), (685, 292.5)],
        parent=nary_group,
    )
    diagram.arc(
        "nary_c_arc",
        "interaction",
        "nary_interaction",
        "nary_c",
        [(725, 282.5), (760, 292.5)],
        parent=nary_group,
        outcome=("nary_outcome", (738, 274.5, 16, 16)),
    )
    return diagram


def build_logic_arc() -> DiagramBuilder:
    """Reconstruct a complete entity-to-logical-operator logic arc."""

    diagram = DiagramBuilder("upstream_logic_arc")
    diagram.glyph("origin", "entity", (20, 45, 170, 75), label="Origin EN")
    diagram.glyph("operator", "and", (330, 52.5, 60, 60))
    diagram.arc(
        "logic_arc",
        "logic arc",
        "origin",
        "operator",
        [(190, 82.5), (330, 82.5)],
    )
    return diagram


def build_nesting(variable_class: str) -> DiagramBuilder:
    """Reconstruct contained and containing entity assignment semantics.

    Args:
        variable_class: Either existence or location.

    Returns:
        Constructed nesting diagram.
    """

    diagram = DiagramBuilder(f"upstream_nesting_{variable_class}")
    symbol = "F" if variable_class == "existence" else "C"
    for prefix, offset_x, target_outer in (
        ("left", 20, False),
        ("right", 390, True),
    ):
        outer = diagram.glyph(
            f"{prefix}_x",
            "entity",
            (offset_x, 170, 310, 135),
            label="X",
        )
        entity_a = diagram.glyph(
            f"{prefix}_a",
            "entity",
            (offset_x + 20, 220, 120, 60),
            label="A",
            parent=outer,
        )
        diagram.glyph(
            f"{prefix}_variable_a",
            variable_class,
            (offset_x + 68, 208, 24, 24),
            parent=entity_a,
        )
        diagram.glyph(
            f"{prefix}_b",
            "entity",
            (offset_x + 170, 220, 120, 60),
            label="B",
            parent=outer,
        )
        if target_outer:
            target_id = f"{prefix}_variable_x"
            diagram.glyph(
                target_id,
                variable_class,
                (offset_x + 143, 158, 24, 24),
                parent=outer,
            )
        else:
            target_id = f"{prefix}_variable_a"
        diagram.glyph(
            f"{prefix}_value",
            "variable value",
            (offset_x + 120, 55, 70, 34),
            label=symbol,
        )
        diagram.arc(
            f"{prefix}_assignment",
            "assignment",
            f"{prefix}_value",
            target_id,
            [
                (offset_x + 155, 89),
                (offset_x + 155, 135),
                (
                    offset_x + 80 if not target_outer else offset_x + 155,
                    208 if not target_outer else 182,
                ),
            ],
            outcome=(f"{prefix}_outcome", (offset_x + 147, 119, 16, 16)),
        )
    return diagram


DiagramFactory = Callable[[], DiagramBuilder]

AUXILIARY_GLYPH_CLASSES = {
    "cardinality",
    "existence",
    "location",
    "outcome",
    "state variable",
    "unit of information",
}

DIAGRAMS: list[tuple[str, str, DiagramFactory | None]] = [
    ("assignment", "Assignment alternatives", build_assignment),
    ("asynchronous", "Independent consequences", build_asynchronous),
    ("entity-granularity", "Entity granularity", build_entity_granularity),
    ("entity-identity", "Entity identity", build_entity_identity),
    ("interaction", "Binary and n-ary interactions", build_interaction),
    ("logicArc", "Logic arc", build_logic_arc),
    (
        "nesting-existence",
        "Nested entity existence semantics",
        lambda: build_nesting("existence"),
    ),
    (
        "nesting-location",
        "Nested entity location semantics",
        lambda: build_nesting("location"),
    ),
    ("refcard", "Appendix B reference card", None),
    ("simultaneous", "Multiple simultaneous consequences", build_simultaneous),
    ("synchronous", "Synchronous consequences", build_synchronous),
]


def slugify(source_name: str) -> str:
    """Convert an upstream mixed-case basename to a lowercase file slug.

    Args:
        source_name: Upstream image basename.

    Returns:
        Lowercase underscore-separated slug.
    """

    characters = []
    for character in source_name:
        if character.isupper():
            characters.extend(["_", character.lower()])
        elif character == "-":
            characters.append("_")
        else:
            characters.append(character)
    return "".join(characters).strip("_")


def validate_connected_diagram(path: Path) -> tuple[int, int]:
    """Verify the requested minimum of two nodes and one relationship.

    Args:
        path: Generated SBGN-ML fixture.

    Returns:
        Semantic-node and relationship counts.
    """

    root = ET.parse(path).getroot()
    glyphs = root.findall(f".//{qualified_name('glyph')}")
    semantic_nodes = [
        glyph
        for glyph in glyphs
        if glyph.get("class", "") not in AUXILIARY_GLYPH_CLASSES
    ]
    relationships = root.findall(f".//{qualified_name('arc')}")
    if len(semantic_nodes) < 2 or not relationships:
        raise ValueError(
            f"{path} does not meet the two-node, one-relationship threshold"
        )
    return len(semantic_nodes), len(relationships)


def run(command: list[str], cwd: Path) -> None:
    """Run a required subprocess without leaking the PEP 723 environment.

    Args:
        command: Command and arguments.
        cwd: Working directory for the command.

    Returns:
        None.
    """

    environment = os.environ.copy()
    environment.pop("VIRTUAL_ENV", None)
    subprocess.run(command, cwd=cwd, env=environment, check=True)


def render_output(
    command: list[str], working_directory: Path, input_path: Path, output_path: Path
) -> None:
    """Render one SBGN input to a requested output path.

    Args:
        command: Renderer command before shared arguments.
        working_directory: Renderer working directory.
        input_path: SBGN-ML fixture.
        output_path: PNG or SVG destination.

    Returns:
        None.
    """

    output_path.parent.mkdir(parents=True, exist_ok=True)
    run(
        [*command, "-i", str(input_path), "-o", str(output_path)],
        working_directory,
    )


def validate_renderer_structure(
    renderer_name: str,
    command: list[str],
    working_directory: Path,
    input_path: Path,
    manifest_path: Path,
) -> None:
    """Require one rendered shape and line for every SBGN glyph and arc.

    Args:
        renderer_name: Human-readable renderer name for diagnostics.
        command: Renderer command before shared arguments.
        working_directory: Renderer working directory.
        input_path: SBGN-ML fixture to validate.
        manifest_path: Temporary manifest output path.

    Returns:
        None.

    Raises:
        RuntimeError: A renderer omits any glyph or arc from its manifest.
    """

    root = ET.parse(input_path).getroot()
    expected_glyphs = {
        glyph.get("id", "")
        for glyph in root.findall(f".//{qualified_name('glyph')}")
        if glyph.get("id")
    }
    expected_arcs = {
        arc.get("id", "")
        for arc in root.findall(f".//{qualified_name('arc')}")
        if arc.get("id")
    }
    run(
        [
            *command,
            "-i",
            str(input_path),
            "-o",
            str(manifest_path),
            "--generate-render-test-manifest",
        ],
        working_directory,
    )
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    elements = manifest.get("elements", [])
    rendered_glyphs = {
        element.get("owner_id", "")
        for element in elements
        if str(element.get("kind", "")).endswith("shape")
    }
    rendered_arcs = {
        element.get("owner_id", "")
        for element in elements
        if element.get("kind") == "edge_line"
    }
    missing_glyphs = sorted(expected_glyphs - rendered_glyphs)
    missing_arcs = sorted(expected_arcs - rendered_arcs)
    if missing_glyphs or missing_arcs:
        raise RuntimeError(
            f"{renderer_name} omitted structure from {input_path.name}: "
            f"glyphs={missing_glyphs}, arcs={missing_arcs}"
        )


def compose_comparison(
    title: str, original_path: Path, renderer_paths: dict[str, Path], output_path: Path
) -> None:
    """Create one original-plus-four-renderer comparison image.

    Args:
        title: Human-readable upstream figure title.
        original_path: Source-faithful upstream raster.
        renderer_paths: Renderer labels mapped to PNG paths.
        output_path: Comparison PNG destination.

    Returns:
        None.
    """

    panels = [(f"Original: {title}", original_path), *renderer_paths.items()]
    canvas_width = PANEL_GAP * 4 + PANEL_WIDTH * 3
    canvas_height = PANEL_GAP * 3 + PANEL_HEIGHT * 2
    canvas = Image.new("RGB", (canvas_width, canvas_height), "white")
    draw = ImageDraw.Draw(canvas)
    font = ImageFont.truetype(str(FONT_PATH), 30)
    top_x_positions = [
        PANEL_GAP,
        PANEL_GAP * 2 + PANEL_WIDTH,
        PANEL_GAP * 3 + PANEL_WIDTH * 2,
    ]
    bottom_start = (canvas_width - (PANEL_WIDTH * 2 + PANEL_GAP)) // 2
    positions = [
        *((x, PANEL_GAP) for x in top_x_positions),
        (bottom_start, PANEL_GAP * 2 + PANEL_HEIGHT),
        (bottom_start + PANEL_WIDTH + PANEL_GAP, PANEL_GAP * 2 + PANEL_HEIGHT),
    ]
    for (label, image_path), (panel_x, panel_y) in zip(panels, positions):
        draw.rounded_rectangle(
            (
                panel_x,
                panel_y,
                panel_x + PANEL_WIDTH,
                panel_y + PANEL_HEIGHT,
            ),
            radius=12,
            fill="white",
            outline="#d0d7de",
            width=2,
        )
        label_box = draw.textbbox((0, 0), label, font=font)
        label_width = label_box[2] - label_box[0]
        draw.text(
            (panel_x + (PANEL_WIDTH - label_width) / 2, panel_y + 10),
            label,
            fill="#24292f",
            font=font,
        )
        with Image.open(image_path) as source_image:
            rendered = source_image.convert("RGBA")
            rendered.thumbnail(
                (PANEL_WIDTH - 32, PANEL_HEIGHT - LABEL_HEIGHT - 24),
                Image.Resampling.LANCZOS,
            )
            flattened = Image.new("RGBA", rendered.size, "white")
            flattened.alpha_composite(rendered)
            image_x = panel_x + (PANEL_WIDTH - rendered.width) // 2
            image_y = (
                panel_y
                + LABEL_HEIGHT
                + (PANEL_HEIGHT - LABEL_HEIGHT - rendered.height) // 2
            )
            canvas.paste(flattened.convert("RGB"), (image_x, image_y))
    output_path.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(output_path, optimize=True)


def write_index(rows: list[dict[str, str]]) -> None:
    """Write a browsable comparison gallery.

    Args:
        rows: Manifest records for rendered upstream figures.

    Returns:
        None.
    """

    cards = []
    for row in rows:
        cards.append(f"""
<article>
  <h2>{html.escape(row['title'])}</h2>
  <a href="{row['slug']}/comparison.png">
    <img src="{row['slug']}/comparison.png"
         alt="{html.escape(row['title'])} comparison">
  </a>
  <p>
    <a href="../../examples/upstream_images/{row['slug']}.sbgn">SBGN-ML</a>
    | <a href="{html.escape(row['source_url'])}">original</a>
  </p>
</article>""")
    document = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Connected upstream ER image reconstructions</title>
<style>
body {{
  font-family: Arial, 'Liberation Sans', sans-serif;
  margin: 2rem;
  color: #24292f;
}}
main {{
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(420px, 1fr));
  gap: 1rem;
}}
article {{ border: 1px solid #d0d7de; border-radius: 8px; padding: 1rem; }}
h1 {{ margin-bottom: 0.25rem; }} h2 {{ font-size: 1.05rem; }}
img {{ width: 100%; height: 360px; object-fit: contain; background: white; }}
</style>
</head>
<body>
<h1>Connected upstream ER image reconstructions</h1>
<p>
  Original artwork compared with Python, Rust, Go, and R renderings of
  reconstructed SBGN-ML.
</p>
<main>{''.join(cards)}</main>
</body>
</html>
"""
    (OUTPUT_ROOT / "index.html").write_text(document, encoding="utf-8")


def generate_examples() -> list[dict[str, str]]:
    """Generate selected SBGN fixtures and return their catalog records.

    Returns:
        Catalog records containing source, slug, title, and fixture paths.
    """

    rows = []
    for source_name, title, factory in DIAGRAMS:
        slug = slugify(source_name)
        fixture_path = EXAMPLE_ROOT / f"{slug}.sbgn"
        if factory is None:
            shutil.copyfile(
                REPOSITORY_ROOT / "render_examples" / "er_all_glyphs.sbgn",
                fixture_path,
            )
        else:
            factory().write(fixture_path)
        node_count, relationship_count = validate_connected_diagram(fixture_path)
        rows.append(
            {
                "source": f"images/{source_name}.svg",
                "slug": slug,
                "title": title,
                "fixture": str(fixture_path.relative_to(REPOSITORY_ROOT)),
                "source_url": f"{UPSTREAM_IMAGE_FILE_URL}/{source_name}.svg",
                "semantic_nodes": str(node_count),
                "relationships": str(relationship_count),
            }
        )
    return rows


def main() -> None:
    """Generate fixtures, all renderer outputs, comparisons, and an index."""

    if not UPSTREAM_IMAGES.is_dir():
        UPSTREAM_ROOT.parent.mkdir(parents=True, exist_ok=True)
        run(
            ["git", "clone", "--depth", "1", f"{UPSTREAM_URL}.git", str(UPSTREAM_ROOT)],
            REPOSITORY_ROOT,
        )
    rows = generate_examples()
    run(["cargo", "build", "--release"], REPOSITORY_ROOT)
    rust_renderer = REPOSITORY_ROOT / "target" / "release" / "render_er"

    with tempfile.TemporaryDirectory(prefix="render-er-upstream-") as temporary:
        temporary_root = Path(temporary)
        go_renderer = temporary_root / "render_sbgn_go"
        run(["go", "build", "-o", str(go_renderer), "."], REPOSITORY_ROOT / "go")
        renderers = {
            "Python": (
                ["uv", "run", "--frozen", "render_sbgn_py", "draw_sbgnml"],
                REPOSITORY_ROOT / "python",
            ),
            "Rust": ([str(rust_renderer), "draw_sbgnml"], REPOSITORY_ROOT),
            "Go": ([str(go_renderer), "draw_sbgnml"], REPOSITORY_ROOT),
            "R": (["Rscript", "r/draw_sbgnml.R"], REPOSITORY_ROOT),
        }
        for row in rows:
            slug = row["slug"]
            fixture_path = REPOSITORY_ROOT / row["fixture"]
            output_directory = OUTPUT_ROOT / slug
            output_directory.mkdir(parents=True, exist_ok=True)
            original_path = output_directory / "original.png"
            run(
                [
                    str(rust_renderer),
                    "draw_svg",
                    "-i",
                    str(UPSTREAM_ROOT / row["source"]),
                    "-o",
                    str(original_path),
                    "--padding",
                    "8",
                    "--scale",
                    "2",
                ],
                REPOSITORY_ROOT,
            )
            renderer_png_paths = {}
            for renderer_name, (command, working_directory) in renderers.items():
                renderer_slug = renderer_name.lower()
                validate_renderer_structure(
                    renderer_name,
                    command,
                    working_directory,
                    fixture_path,
                    temporary_root / f"{slug}-{renderer_slug}.json",
                )
                for output_format in OUTPUT_FORMATS:
                    output_path = (
                        output_directory / renderer_slug / f"{slug}.{output_format}"
                    )
                    render_output(command, working_directory, fixture_path, output_path)
                    if output_format == "png":
                        renderer_png_paths[renderer_name] = output_path
            compose_comparison(
                row["title"],
                original_path,
                renderer_png_paths,
                output_directory / "comparison.png",
            )

    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    with (OUTPUT_ROOT / "manifest.csv").open(
        "w", encoding="utf-8", newline=""
    ) as stream:
        writer = csv.DictWriter(
            stream,
            fieldnames=rows[0].keys(),
            lineterminator="\n",
        )
        writer.writeheader()
        writer.writerows(rows)
    write_index(rows)
    print(f"Rendered {len(rows)} connected upstream figures to {OUTPUT_ROOT}")


if __name__ == "__main__":
    main()
