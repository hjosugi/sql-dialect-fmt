#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: scripts/configure-crates-publishing.sh [--repo owner/name] [--no-auto] [--dry-run]

Stores CARGO_REGISTRY_TOKEN as a GitHub Actions secret and enables crates.io publishing on
future release tag pushes. Set CARGO_REGISTRY_TOKEN in the environment before running.

Options:
  --no-auto  Store the token without enabling CRATES_IO_AUTO_PUBLISH.
  --dry-run  Print the setting names without writing values.
EOF
}

repo="${GITHUB_REPOSITORY:-}"
auto_publish=true
dry_run=false

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      repo="${2:-}"
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

if [ -z "${CARGO_REGISTRY_TOKEN:-}" ]; then
  echo "CARGO_REGISTRY_TOKEN is required" >&2
  exit 1
fi

if [ "$dry_run" = true ]; then
  echo "DRY-RUN gh secret set CARGO_REGISTRY_TOKEN --repo $repo"
  if [ "$auto_publish" = true ]; then
    echo "DRY-RUN gh variable set CRATES_IO_AUTO_PUBLISH --repo $repo"
  fi
else
  gh secret set CARGO_REGISTRY_TOKEN --body "$CARGO_REGISTRY_TOKEN" --repo "$repo"
  if [ "$auto_publish" = true ]; then
    gh variable set CRATES_IO_AUTO_PUBLISH --body true --repo "$repo"
  fi
fi

echo "Configured crates.io publishing for $repo."
