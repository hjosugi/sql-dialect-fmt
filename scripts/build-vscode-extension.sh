#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_OUT="${CARGO_TARGET_DIR:-$ROOT_DIR/target}/wasm32-unknown-unknown/release/sql_dialect_fmt_wasm.wasm"
EXT_VENDOR="$ROOT_DIR/editors/vendor"

cargo build --release --locked -p sql-dialect-fmt-wasm --target wasm32-unknown-unknown
mkdir -p "$EXT_VENDOR"
cp "$WASM_OUT" "$EXT_VENDOR/sql_dialect_fmt_wasm.wasm"

# Runtime npm dependencies (the optional vscode-languageclient LSP client) are installed into
# node_modules/ so `vsce package` can bundle them and the packaged VSIX works offline. The
# `files` whitelist in editors/package.json must keep its `node_modules/**` entry: vsce filters
# every collected file — dependencies included — through those globs.
npm --prefix "$ROOT_DIR/editors" ci --omit=dev --no-audit --no-fund --loglevel=error

echo "VS Code extension is ready at: $ROOT_DIR/editors"
