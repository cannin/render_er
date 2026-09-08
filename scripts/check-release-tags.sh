#!/usr/bin/env bash

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="${1:-}"
release_tag="${2:-v$version}"
go_tag="${3:-go/v$version}"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  printf 'Usage: %s X.Y.Z [vX.Y.Z] [go/vX.Y.Z]\n' "$0" >&2
  exit 1
fi

if [[ "$release_tag" != "v$version" || "$go_tag" != "go/v$version" ]]; then
  printf 'Expected coordinated tags v%s and go/v%s.\n' \
    "$version" "$version" >&2
  exit 1
fi

for tag in "$release_tag" "$go_tag"; do
  if ! git -C "$repository_root" rev-parse --verify --quiet \
    "refs/tags/$tag^{}" >/dev/null; then
    printf 'Missing release tag: %s\n' "$tag" >&2
    exit 1
  fi
done

release_commit="$(git -C "$repository_root" rev-parse "refs/tags/$release_tag^{}")"
go_commit="$(git -C "$repository_root" rev-parse "refs/tags/$go_tag^{}")"
checked_out_commit="$(git -C "$repository_root" rev-parse HEAD)"
if [[ "$release_commit" != "$go_commit" ]]; then
  printf 'Tags %s and %s do not point to the same commit.\n' \
    "$release_tag" "$go_tag" >&2
  exit 1
fi
if [[ "$release_commit" != "$checked_out_commit" ]]; then
  printf 'Tag %s does not point to checked-out commit %s.\n' \
    "$release_tag" "$checked_out_commit" >&2
  exit 1
fi

printf 'Coordinated tags %s and %s point to %s.\n' \
  "$release_tag" "$go_tag" "$release_commit"
