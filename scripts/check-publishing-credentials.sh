#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: scripts/check-publishing-credentials.sh vscode|chrome|crates

Checks that the environment contains the credentials required by a publishing target.
Secret values are never printed.
EOF
}

require_env() {
  local name="$1"
  if [ -z "${!name:-}" ]; then
    echo "::error::$name is required for $target publishing" >&2
    missing=true
  fi
}

target="${1:-}"
missing=false

case "$target" in
  vscode)
    case "${VSCE_AUTH_MODE:-pat}" in
      pat)
        require_env VSCE_PAT
        ;;
      azure)
        require_env AZURE_CLIENT_ID
        require_env AZURE_TENANT_ID
        ;;
      *)
        echo "::error::unsupported VSCE_AUTH_MODE: ${VSCE_AUTH_MODE}" >&2
        exit 2
        ;;
    esac
    ;;
  chrome)
    require_env CHROME_PUBLISHER_ID
    require_env CHROME_EXTENSION_ID
    require_env CHROME_CLIENT_ID
    require_env CHROME_CLIENT_SECRET
    require_env CHROME_REFRESH_TOKEN
    ;;
  crates)
    require_env CARGO_REGISTRY_TOKEN
    ;;
  *)
    usage
    exit 2
    ;;
esac

if [ "$missing" = true ]; then
  exit 1
fi

echo "Publishing credentials are configured for $target."
