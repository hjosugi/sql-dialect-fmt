#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$("$ROOT_DIR/scripts/workspace-version.sh")"

CRATES=(
  sql-dialect-fmt-syntax
  sql-dialect-fmt-text
  sql-dialect-fmt-lexer
  sql-dialect-fmt-parser
  sql-dialect-fmt-formatter
  sql-dialect-fmt-highlight
  sql-dialect-fmt-hover
  sql-dialect-fmt-encoding
  sql-dialect-fmt
  sql-dialect-fmt-lsp
  sql-dialect-fmt-wasm
)

wait_for_index() {
  local crate="$1"

  if [ "${SQL_DIALECT_FMT_SKIP_INDEX_WAIT:-false}" = "true" ]; then
    return
  fi

  for attempt in $(seq 1 60); do
    local tmp
    tmp="$(mktemp -d)"
    cat >"$tmp/Cargo.toml" <<EOF
[package]
name = "sql-dialect-fmt-index-check"
version = "0.0.0"
edition = "2021"

[dependencies]
$crate = "=$VERSION"
EOF
    if cargo metadata --quiet --manifest-path "$tmp/Cargo.toml" --format-version 1 >/dev/null 2>&1; then
      rm -rf "$tmp"
      return
    fi
    rm -rf "$tmp"
    echo "Waiting for $crate $VERSION to appear in the crates.io index ($attempt/60)"
    sleep 10
  done

  echo "Timed out waiting for $crate $VERSION to appear in crates.io" >&2
  exit 1
}

version_is_published() {
  local crate="$1"
  cargo info "$crate@$VERSION" --registry crates-io >/dev/null 2>&1
}

for crate in "${CRATES[@]}"; do
  if version_is_published "$crate"; then
    echo "Skipping $crate $VERSION (already published)"
    continue
  fi

  echo "Publishing $crate"
  cargo publish --manifest-path "$ROOT_DIR/Cargo.toml" -p "$crate"
  wait_for_index "$crate"
done
