#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_OUT="$ROOT_DIR/target/wasm32-unknown-unknown/release/sql_dialect_fmt_wasm.wasm"
EXT_VENDOR="$ROOT_DIR/editors/vendor"

cargo build --release --locked -p sql-dialect-fmt-wasm --target wasm32-unknown-unknown
mkdir -p "$EXT_VENDOR"
cp "$WASM_OUT" "$EXT_VENDOR/sql_dialect_fmt_wasm.wasm"

echo "VS Code extension is ready at: $ROOT_DIR/editors"
