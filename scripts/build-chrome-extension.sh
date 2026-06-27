#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_OUT="$ROOT_DIR/target/wasm32-unknown-unknown/release/snow_fmt_wasm.wasm"
EXT_VENDOR="$ROOT_DIR/extensions/chrome/vendor"

cargo build --release -p snow-fmt-wasm --target wasm32-unknown-unknown
mkdir -p "$EXT_VENDOR"
cp "$WASM_OUT" "$EXT_VENDOR/snow_fmt_wasm.wasm"

echo "Chrome extension is ready at: $ROOT_DIR/extensions/chrome"
