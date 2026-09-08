"""Documentation contract tests for root README command examples."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
README = (ROOT / "README.md").read_text(encoding="utf-8")


def test_root_readme_contains_go_png_and_svg_examples() -> None:
    """Document runnable Go examples for both supported output formats."""

    assert "(cd go && go run . draw_sbgnml" in README
    assert "output/figure_1_1-go.png" in README
    assert "output/figure_1_1-go.svg" in README


def test_root_readme_describes_all_four_native_implementations() -> None:
    """Present Python, Rust, Go, and R as parity-tested implementations."""

    for implementation in ("Python", "Rust", "Go", "R"):
        assert f"[{implementation}]" in README
    assert "./scripts/test-all.sh" in README
