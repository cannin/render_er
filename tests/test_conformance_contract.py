"""Tests for cross-language conformance signatures."""

import importlib.util
from pathlib import Path

SCRIPT_PATH = Path(__file__).resolve().parents[1] / "scripts" / "conformance.py"
SPEC = importlib.util.spec_from_file_location("render_er_conformance", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
CONFORMANCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONFORMANCE)


def test_signature_detects_marker_behavior_mismatch() -> None:
    """Treat differing marker primitives as a conformance failure."""

    triangle = {
        "elements": [
            {
                "id": "arc::marker",
                "kind": "arc-marker",
                "text": "",
                "source": "a",
                "target": "b",
                "marker": "triangle",
            }
        ]
    }
    double_triangle = {
        "elements": [
            {
                **triangle["elements"][0],
                "marker": "double-triangle",
            }
        ]
    }

    assert CONFORMANCE.conformance_signature(triangle) != (
        CONFORMANCE.conformance_signature(double_triangle)
    )
