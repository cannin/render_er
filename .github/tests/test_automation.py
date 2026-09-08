"""Contract tests for repository CI and release automation."""

from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]


def coordinated_version() -> str:
    """Read the canonical package version from Python metadata.

    Returns:
        Current coordinated version.
    """
    metadata_path = REPOSITORY_ROOT / "python" / "pyproject.toml"
    with metadata_path.open("rb") as metadata_file:
        return tomllib.load(metadata_file)["project"]["version"]


def run_script(name: str, *arguments: str) -> subprocess.CompletedProcess[str]:
    """Run an automation script from the repository root.

    Args:
        name: Script filename below ``scripts``.
        *arguments: Arguments forwarded to the script.

    Returns:
        Completed subprocess result with captured text output.
    """
    return subprocess.run(
        [str(REPOSITORY_ROOT / "scripts" / name), *arguments],
        cwd=REPOSITORY_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def test_coordinated_version_is_derived_and_can_be_asserted() -> None:
    """Derive the current version by default and reject a mismatched tag."""
    version = coordinated_version()
    derived = run_script("check-versions.sh")
    assert derived.returncode == 0, derived.stderr
    assert f"All implementation versions are {version}." in derived.stdout

    mismatch = run_script("check-versions.sh", "9.9.9")
    assert mismatch.returncode != 0
    assert "expected 9.9.9" in mismatch.stderr


def test_source_archives_use_render_er_names_and_layout() -> None:
    """Package the coordinated source tree with stable release filenames."""
    version = coordinated_version()
    with tempfile.TemporaryDirectory() as temporary_directory:
        result = run_script("package-sources.sh", temporary_directory, "HEAD")
        assert result.returncode == 0, result.stderr
        archive_names = sorted(
            path.name for path in Path(temporary_directory).glob("*.tar.gz")
        )

    assert archive_names == [
        f"render_er-{version}-all-source.tar.gz",
        f"render_er-go-{version}-source.tar.gz",
        f"render_er-python-{version}-source.tar.gz",
        f"render_er-r-{version}-source.tar.gz",
        f"render_er-rust-{version}-source.tar.gz",
    ]


def test_coordinated_release_tags_must_resolve_to_one_commit() -> None:
    """Accept paired annotated release and Go module tags on one commit."""
    with tempfile.TemporaryDirectory() as temporary_directory:
        temporary_root = Path(temporary_directory)
        scripts_directory = temporary_root / "scripts"
        scripts_directory.mkdir()
        shutil.copy2(
            REPOSITORY_ROOT / "scripts" / "check-release-tags.sh",
            scripts_directory,
        )
        commands = (
            ("git", "init", "--quiet"),
            ("git", "config", "user.name", "automation-test"),
            ("git", "config", "user.email", "automation@example.invalid"),
        )
        for command in commands:
            subprocess.run(command, cwd=temporary_root, check=True)
        (temporary_root / "fixture.txt").write_text("fixture\n", encoding="utf-8")
        subprocess.run(
            ("git", "add", "fixture.txt", "scripts/check-release-tags.sh"),
            cwd=temporary_root,
            check=True,
        )
        subprocess.run(
            ("git", "commit", "--quiet", "-m", "fixture"),
            cwd=temporary_root,
            check=True,
        )
        for tag in ("v1.2.3", "go/v1.2.3"):
            subprocess.run(
                ("git", "tag", "--annotate", tag, "-m", tag),
                cwd=temporary_root,
                check=True,
            )
        result = subprocess.run(
            [
                str(scripts_directory / "check-release-tags.sh"),
                "1.2.3",
                "v1.2.3",
                "go/v1.2.3",
            ],
            cwd=temporary_root,
            check=False,
            capture_output=True,
            text=True,
        )

    assert result.returncode == 0, result.stderr
    assert "Coordinated tags v1.2.3 and go/v1.2.3" in result.stdout


def test_ci_paths_cover_automation_and_render_fixtures() -> None:
    """Ensure changes to workflows, helpers, and fixtures trigger CI."""
    ci_text = (REPOSITORY_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    for path_filter in (
        '".github/workflows/**"',
        '"scripts/**"',
        '".github/tests/**"',
        '"examples/**"',
        '"render_examples/**"',
    ):
        assert path_filter in ci_text


def test_release_contract_includes_every_artifact_family() -> None:
    """Keep coordinated release jobs, tag checks, and checksums together."""
    release_text = (REPOSITORY_ROOT / ".github/workflows/release.yml").read_text(
        encoding="utf-8"
    )
    for required_text in (
        "tags:",
        '"v*"',
        "check-release-tags.sh",
        "source-archives",
        "python-wheel",
        "r-source-package",
        "go-binaries",
        "rust-linux",
        "rust-macos",
        "rust-windows",
        "SHA256SUMS.txt",
    ):
        assert required_text in release_text
    assert '"go/v*"' not in release_text
    assert coordinated_version() not in release_text


def test_go_makefile_builds_every_required_release_target() -> None:
    """Build Go release executables for each supported OS and architecture."""
    makefile_text = (REPOSITORY_ROOT / "go" / "Makefile").read_text(encoding="utf-8")
    for artifact_suffix in (
        "linux-amd64",
        "linux-arm64",
        "darwin-amd64",
        "darwin-arm64",
        "windows-amd64.exe",
        "windows-arm64.exe",
    ):
        assert f"$(APPNAME)-{artifact_suffix}" in makefile_text


def test_er_conformance_fixture_and_rust_binary_are_selected() -> None:
    """Target the repository's canonical ER fixture and Rust executable."""
    conformance_text = (REPOSITORY_ROOT / "scripts/conformance.py").read_text(
        encoding="utf-8"
    )
    assert 'DEFAULT_INPUT_PATH = REPOSITORY_ROOT / "render_examples" / (' in (
        conformance_text
    )
    assert '"er_all_glyphs.sbgn"' in conformance_text
    assert '"render_er"' in conformance_text
    assert '"render_sbgn_rs"' not in conformance_text
