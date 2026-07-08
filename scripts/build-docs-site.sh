#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_OUT="$ROOT_DIR/target/wasm32-unknown-unknown/release/sql_dialect_fmt_wasm.wasm"
BOOK_OUT="$ROOT_DIR/target/docs-site"

cargo build --release --locked -p sql-dialect-fmt-wasm --target wasm32-unknown-unknown
mdbook build "$ROOT_DIR/docs-site"
cp "$WASM_OUT" "$BOOK_OUT/sql_dialect_fmt_wasm.wasm"

echo "Docs site is ready at: $BOOK_OUT"
