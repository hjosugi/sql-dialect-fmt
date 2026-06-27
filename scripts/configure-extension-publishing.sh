#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: scripts/configure-extension-publishing.sh [--repo owner/name] [--target all|vscode|chrome] [--vscode-auth pat|azure|skip] [--no-auto] [--dry-run]

Configures GitHub repository variables/secrets for automated VS Code Marketplace and Chrome Web Store publishing.
Store account creation, first listing/privacy/distribution forms, and publisher/managed-identity authorization must still be done in the store dashboards.

Environment for VS Code PAT mode:
  VSCE_PAT

Environment for VS Code Azure mode:
  AZURE_CLIENT_ID
  AZURE_TENANT_ID
  AZURE_SUBSCRIPTION_ID      optional when the identity has no Azure subscription

Environment for Chrome Web Store:
  CHROME_PUBLISHER_ID
  CHROME_EXTENSION_ID
  CHROME_CLIENT_ID
  CHROME_CLIENT_SECRET
  CHROME_REFRESH_TOKEN
EOF
}

repo="${GITHUB_REPOSITORY:-}"
target="all"
vscode_auth="${VSCE_AUTH_MODE:-pat}"
auto_publish=true
dry_run=false

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      repo="${2:-}"
      shift 2
      ;;
    --target)
      target="${2:-}"
      shift 2
      ;;
    --vscode-auth)
      vscode_auth="${2:-}"
      shift 2
      ;;
    --no-auto)
      auto_publish=false
      shift
      ;;
    --dry-run)
      dry_run=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

case "$target" in
  all|vscode|chrome) ;;
  *)
    echo "unsupported --target: $target" >&2
    exit 2
    ;;
esac

case "$vscode_auth" in
  pat|azure|skip) ;;
  *)
    echo "unsupported --vscode-auth: $vscode_auth" >&2
    exit 2
    ;;
esac

if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI is required" >&2
  exit 1
fi

if [ -z "$repo" ]; then
  repo="$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null || true)"
fi

if [ -z "$repo" ]; then
  echo "could not infer repository; pass --repo owner/name" >&2
  exit 2
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "gh CLI is not authenticated; run gh auth login first" >&2
  exit 1
fi

require_env() {
  local name="$1"
  if [ -z "${!name:-}" ]; then
    echo "$name is required" >&2
    exit 1
  fi
}

set_var() {
  local name="$1"
  local value="$2"
  if [ "$dry_run" = true ]; then
    echo "DRY-RUN gh variable set $name --repo $repo"
  else
    gh variable set "$name" --body "$value" --repo "$repo"
  fi
}

set_secret() {
  local name="$1"
  local value="$2"
  if [ "$dry_run" = true ]; then
    echo "DRY-RUN gh secret set $name --repo $repo"
  else
    gh secret set "$name" --body "$value" --repo "$repo"
  fi
}

configure_vscode() {
  if [ "$vscode_auth" = "skip" ]; then
    return
  fi

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
}

configure_chrome() {
  require_env CHROME_PUBLISHER_ID
  require_env CHROME_EXTENSION_ID
  require_env CHROME_CLIENT_ID
  require_env CHROME_CLIENT_SECRET
  require_env CHROME_REFRESH_TOKEN

  set_var CHROME_PUBLISHER_ID "$CHROME_PUBLISHER_ID"
  set_var CHROME_EXTENSION_ID "$CHROME_EXTENSION_ID"
  set_secret CHROME_CLIENT_ID "$CHROME_CLIENT_ID"
  set_secret CHROME_CLIENT_SECRET "$CHROME_CLIENT_SECRET"
  set_secret CHROME_REFRESH_TOKEN "$CHROME_REFRESH_TOKEN"
}

case "$target" in
  all)
    configure_vscode
    configure_chrome
    if [ "$auto_publish" = true ]; then
      set_var EXTENSIONS_AUTO_PUBLISH true
    fi
    ;;
  vscode)
    configure_vscode
    if [ "$auto_publish" = true ]; then
      set_var VSCODE_MARKETPLACE_AUTO_PUBLISH true
    fi
    ;;
  chrome)
    configure_chrome
    if [ "$auto_publish" = true ]; then
      set_var CHROME_WEBSTORE_AUTO_PUBLISH true
    fi
    ;;
esac

echo "Configured extension publishing for $repo ($target)."
