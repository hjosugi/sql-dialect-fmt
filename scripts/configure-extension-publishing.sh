#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: scripts/configure-extension-publishing.sh [--repo owner/name] [--vscode-auth pat|azure] [--no-auto] [--dry-run]

Configures GitHub repository variables/secrets for VS Code Marketplace publishing.
The Marketplace publisher, first listing, and identity authorization must already exist.

PAT mode environment:
  VSCE_PAT

Azure mode environment:
  AZURE_CLIENT_ID
  AZURE_TENANT_ID
  AZURE_SUBSCRIPTION_ID      optional when the identity has no Azure subscription
EOF
}

repo="${GITHUB_REPOSITORY:-}"
vscode_auth="${VSCE_AUTH_MODE:-pat}"
auto_publish=true
dry_run=false

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo) repo="${2:-}"; shift 2 ;;
    --vscode-auth) vscode_auth="${2:-}"; shift 2 ;;
    --no-auto) auto_publish=false; shift ;;
    --dry-run) dry_run=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) usage; exit 2 ;;
  esac
done

case "$vscode_auth" in
  pat|azure) ;;
  *) echo "unsupported --vscode-auth: $vscode_auth" >&2; exit 2 ;;
esac

command -v gh >/dev/null 2>&1 || { echo "gh CLI is required" >&2; exit 1; }
if [ -z "$repo" ]; then
  repo="$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null || true)"
fi
if [ -z "$repo" ]; then
  echo "could not infer repository; pass --repo owner/name" >&2
  exit 2
fi
gh auth status >/dev/null 2>&1 || { echo "gh CLI is not authenticated" >&2; exit 1; }

require_env() {
  local name="$1"
  if [ -z "${!name:-}" ]; then
    echo "$name is required" >&2
    exit 1
  fi
}

set_var() {
  if [ "$dry_run" = true ]; then
    echo "DRY-RUN gh variable set $1 --repo $repo"
  else
    gh variable set "$1" --body "$2" --repo "$repo"
  fi
}

set_secret() {
  if [ "$dry_run" = true ]; then
    echo "DRY-RUN gh secret set $1 --repo $repo"
  else
    gh secret set "$1" --body "$2" --repo "$repo"
  fi
}

set_var VSCE_AUTH_MODE "$vscode_auth"
case "$vscode_auth" in
  pat)
    require_env VSCE_PAT
    set_secret VSCE_PAT "$VSCE_PAT"
    ;;
  azure)
    require_env AZURE_CLIENT_ID
    require_env AZURE_TENANT_ID
    set_var AZURE_CLIENT_ID "$AZURE_CLIENT_ID"
    set_var AZURE_TENANT_ID "$AZURE_TENANT_ID"
    if [ -n "${AZURE_SUBSCRIPTION_ID:-}" ]; then
      set_var AZURE_SUBSCRIPTION_ID "$AZURE_SUBSCRIPTION_ID"
    fi
    ;;
esac

if [ "$auto_publish" = true ]; then
  set_var VSCODE_MARKETPLACE_AUTO_PUBLISH true
fi

echo "Configured VS Code Marketplace publishing for $repo."
