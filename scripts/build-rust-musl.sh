#!/usr/bin/env bash

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -n "${RUST_MUSL_TARGET:-}" ]]; then
  musl_target="$RUST_MUSL_TARGET"
else
  case "$(uname -m)" in
    x86_64)
      musl_target="x86_64-unknown-linux-musl"
      ;;
    aarch64 | arm64)
      musl_target="aarch64-unknown-linux-musl"
      ;;
    *)
      printf 'Unsupported host architecture; set RUST_MUSL_TARGET explicitly.\n' >&2
      exit 1
      ;;
  esac
fi

rustup target add "$musl_target"
(
  cd "$repository_root/rust"
  cargo build --release --target "$musl_target" --bin render_er
)

printf 'Built %s\n' \
  "$repository_root/rust/target/$musl_target/release/render_er"
