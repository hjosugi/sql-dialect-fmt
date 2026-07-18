#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_OUT="${CARGO_TARGET_DIR:-$ROOT_DIR/target}/wasm32-unknown-unknown/release/sql_dialect_fmt_wasm.wasm"
EXT_VENDOR="$ROOT_DIR/extensions/chrome/vendor"

cargo build --release --locked -p sql-dialect-fmt-wasm --target wasm32-unknown-unknown
mkdir -p "$EXT_VENDOR"
cp "$WASM_OUT" "$EXT_VENDOR/sql_dialect_fmt_wasm.wasm"

echo "Chrome extension is ready at: $ROOT_DIR/extensions/chrome"
