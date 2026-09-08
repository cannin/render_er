#!/usr/bin/env bash

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python_version="$(awk -F ' = ' '$1 == "version" { gsub(/"/, "", $2); print $2; exit }' "$repository_root/python/pyproject.toml")"
rust_version="$(awk -F ' = ' '$1 == "version" { gsub(/"/, "", $2); print $2; exit }' "$repository_root/rust/Cargo.toml")"
r_version="$(awk -F ': ' '$1 == "Version" { print $2; exit }' "$repository_root/r/DESCRIPTION")"
go_version="$(awk -F '"' '/rendererVersion[[:space:]]*=/{ print $2; exit }' "$repository_root/go/main.go")"
python_cli_version="$(awk -F '"' '/^RENDERER_VERSION = "/ { print $2; exit }' "$repository_root/python/render_sbgn_py/renderer.py")"
r_cli_version="$(awk -F '"' '/^RENDERER_VERSION <- "/ { print $2; exit }' "$repository_root/r/R/draw_sbgnml.R")"
expected_version="${1:-$python_version}"

if [[ ! "$expected_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  printf 'Expected version is not a supported semantic version: %s\n' \
    "$expected_version" >&2
  exit 1
fi

for version_entry in \
  "Python metadata:$python_version" \
  "Python CLI:$python_cli_version" \
  "Rust metadata:$rust_version" \
  "Go CLI:$go_version" \
  "R metadata:$r_version" \
  "R CLI:$r_cli_version"; do
  implementation="${version_entry%%:*}"
  actual_version="${version_entry#*:}"
  if [[ -z "$actual_version" ]]; then
    printf '%s version could not be read.\n' "$implementation" >&2
    exit 1
  fi
  if [[ "$actual_version" != "$expected_version" ]]; then
    printf '%s version is %s; expected %s\n' \
      "$implementation" "$actual_version" "$expected_version" >&2
    exit 1
  fi
done

printf 'All implementation versions are %s.\n' "$expected_version"
