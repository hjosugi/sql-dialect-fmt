#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-$("$ROOT_DIR/scripts/workspace-version.sh")}"
DIST_DIR="$ROOT_DIR/target/dist"

mkdir -p "$DIST_DIR"
rm -f \
  "$DIST_DIR/sql-dialect-fmt-v$VERSION-chrome.zip" \
  "$DIST_DIR/sql-dialect-fmt-v$VERSION.vsix"

rustup target add wasm32-unknown-unknown >/dev/null
"$ROOT_DIR/scripts/build-chrome-extension.sh"

(
  cd "$ROOT_DIR/extensions/chrome"
  zip -qr "$DIST_DIR/sql-dialect-fmt-v$VERSION-chrome.zip" \
    manifest.json \
    images \
    options.html \
    README.md \
    src \
    vendor/sql_dialect_fmt_wasm.wasm
)

(
  cd "$ROOT_DIR/editors"
  npx --yes @vscode/vsce package --no-dependencies \
    --out "$DIST_DIR/sql-dialect-fmt-v$VERSION.vsix"
)

echo "Extension packages:"
echo "  $DIST_DIR/sql-dialect-fmt-v$VERSION-chrome.zip"
echo "  $DIST_DIR/sql-dialect-fmt-v$VERSION.vsix"
