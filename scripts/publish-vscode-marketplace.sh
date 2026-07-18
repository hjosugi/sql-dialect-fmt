#!/usr/bin/env bash
set -euo pipefail

VSIX_PATH="${1:-}"
if [ -z "$VSIX_PATH" ] || [ ! -f "$VSIX_PATH" ]; then
  echo "usage: scripts/publish-vscode-marketplace.sh PATH_TO.vsix" >&2
  exit 2
fi
VSIX_PATH="$(realpath "$VSIX_PATH")"

case "${VSCE_AUTH_MODE:-pat}" in
  pat)
    if [ -z "${VSCE_PAT:-}" ]; then
      echo "::error::VSCE_PAT secret is required when VSCE_AUTH_MODE=pat" >&2
      exit 1
    fi
    npx --yes @vscode/vsce@3.9.2 publish --packagePath "$VSIX_PATH" -p "$VSCE_PAT"
    ;;
  azure)
    npx --yes @vscode/vsce@3.9.2 publish --packagePath "$VSIX_PATH" --azure-credential
    ;;
  *)
    echo "::error::unsupported VSCE_AUTH_MODE: ${VSCE_AUTH_MODE:-}" >&2
    exit 1
    ;;
esac
