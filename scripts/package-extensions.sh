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
# Reuses the cached wasm build above and vendors it into editors/ for the VSIX.
"$ROOT_DIR/scripts/build-vscode-extension.sh"

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
  # extension.js and vscode-languageclient are already bundled into dist/extension.js.
  npx --yes @vscode/vsce@3.9.2 package \
    --no-dependencies \
    --out "$DIST_DIR/sql-dialect-fmt-v$VERSION.vsix"
)
"$ROOT_DIR/scripts/check-vsix-package.py" "$DIST_DIR/sql-dialect-fmt-v$VERSION.vsix"

echo "Extension packages:"
echo "  $DIST_DIR/sql-dialect-fmt-v$VERSION-chrome.zip"
echo "  $DIST_DIR/sql-dialect-fmt-v$VERSION.vsix"
