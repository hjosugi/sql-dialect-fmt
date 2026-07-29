#!/usr/bin/env bash
# One-command release driver: the RELEASING.md runbook, minus the typing.
#
#   scripts/release.sh 1.21.0              # bump, gate, commit, tag, push
#   scripts/release.sh 1.21.0 --dry-run    # print the plan, change nothing
#   scripts/release.sh 1.21.0 --via-ci     # bump + commit, then dispatch Release (no local tag)
#
# See `scripts/release.sh --help` for the full flag list.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

VERSION=""
DRY_RUN=false
VIA_CI=false
RUN_GATE=true
PUSH=true
PUBLISH_CRATES=false
RELEASE_BRANCH="main"

usage() {
  cat <<'EOF'
Usage: scripts/release.sh <version> [options]

Runs the release runbook end to end: version bump, changelog heading, the green
gate, extension packaging, the release commit, the tag, and the push that fires
the Release workflow.

Arguments:
  <version>           Release version, e.g. 1.21.0 (a leading "v" is accepted)

Options:
  --dry-run           Print every step without running or changing anything
  --via-ci            Do not push a tag; dispatch the Release workflow instead.
                      Use when tag pushes are blocked locally. Requires `gh` and
                      the release commit to be on the remote release branch.
  --publish-crates    With --via-ci, also publish to crates.io in that run
  --no-gate           Skip the test/lint/doc/bench gate (CI still runs it)
  --no-push           Commit and tag locally, push nothing
  --branch <name>     Release branch to require (default: main)
  -h, --help          Show this help

Steps (RELEASING.md 1-6):
  1. preflight    clean worktree, on the release branch, synced, version unused
  2. bump         scripts/update-version.py <version> --changelog
  3. gate         fmt, test, clippy, doc, bench, manifest + version consistency,
                  external corpus sample
  4. package      scripts/package-extensions.sh
  5. commit+tag   "release: v<version>" and tag v<version>
  6. push         branch and tag, or a Release workflow dispatch with --via-ci

Publication (crates.io, VS Code Marketplace, Chrome Web Store) stays with the
Release workflow and its *_AUTO_PUBLISH repository variables. This script never
uploads a package itself.
EOF
}

log() { printf '\033[1m==> %s\033[0m\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# Echo the command, then run it — unless this is a dry run, where echoing is all
# there is. Every mutating step goes through here so --dry-run is total.
run() {
  if [ "$DRY_RUN" = true ]; then
    printf '  [dry-run] %s\n' "$*"
    return 0
  fi
  printf '  + %s\n' "$*"
  "$@"
}

while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --dry-run) DRY_RUN=true ;;
    --via-ci) VIA_CI=true ;;
    --publish-crates) PUBLISH_CRATES=true ;;
    --no-gate) RUN_GATE=false ;;
    --no-push) PUSH=false ;;
    --branch)
      [ $# -ge 2 ] || die "--branch needs a value"
      RELEASE_BRANCH="$2"
      shift
      ;;
    -*) die "unknown option: $1 (try --help)" ;;
    *)
      [ -z "$VERSION" ] || die "unexpected extra argument: $1"
      VERSION="$1"
      ;;
  esac
  shift
done

[ -n "$VERSION" ] || { usage >&2; exit 2; }

# Accept both `1.21.0` and `v1.21.0`; everything below works in the bare form
# and prefixes where a tag is meant.
VERSION="${VERSION#v}"
if ! printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$'; then
  die "not a semver version: $VERSION"
fi
TAG="v$VERSION"

if [ "$PUBLISH_CRATES" = true ] && [ "$VIA_CI" = false ]; then
  die "--publish-crates only applies with --via-ci (a tag push uses CRATES_IO_AUTO_PUBLISH)"
fi

# A broken PATH can leave ROOT_DIR pointing at "/" (the `dirname` above fails and
# `cd /.. && pwd` still succeeds), which would run the whole release against the
# wrong tree. Anchor on a file only this repository has.
[ -f "$ROOT_DIR/scripts/workspace-version.sh" ] ||
  die "cannot locate the repository root (got '$ROOT_DIR')"
cd "$ROOT_DIR"

# ---- 1. preflight ----------------------------------------------------------
log "Preflight for $TAG"

current_branch="$(git rev-parse --abbrev-ref HEAD)"
[ "$current_branch" = "$RELEASE_BRANCH" ] ||
  die "on branch '$current_branch', expected '$RELEASE_BRANCH' (override with --branch)"

if [ -n "$(git status --porcelain)" ]; then
  die "working tree is dirty; commit or stash first"
fi

if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  die "tag $TAG already exists locally"
fi

git fetch --quiet origin "$RELEASE_BRANCH" || warn "could not fetch origin/$RELEASE_BRANCH"
if git rev-parse -q --verify "refs/remotes/origin/$RELEASE_BRANCH" >/dev/null; then
  behind="$(git rev-list --count "HEAD..origin/$RELEASE_BRANCH")"
  [ "$behind" = "0" ] ||
    die "$RELEASE_BRANCH is $behind commit(s) behind origin; pull first"
fi

if git ls-remote --exit-code --tags origin "$TAG" >/dev/null 2>&1; then
  die "tag $TAG already exists on origin; releases are immutable"
fi

current_version="$("$ROOT_DIR/scripts/workspace-version.sh")"
log "Workspace version $current_version -> $VERSION"
[ "$current_version" != "$VERSION" ] ||
  warn "workspace is already at $VERSION; the bump will be a no-op"

if [ "$VIA_CI" = true ]; then
  command -v gh >/dev/null ||
    die "--via-ci needs the GitHub CLI (gh); install it or drop the flag"
fi

# ---- 2. bump ---------------------------------------------------------------
log "Bumping version and changelog"
run python3 scripts/update-version.py "$VERSION" --changelog

# ---- 3. gate ---------------------------------------------------------------
if [ "$RUN_GATE" = true ]; then
  log "Green gate (this is the slow part; --no-gate skips it)"
  run cargo fmt --all --check
  run cargo test --workspace
  run cargo clippy --workspace --all-targets -- -D warnings
  run env RUSTDOCFLAGS=-D\ warnings cargo doc --workspace --no-deps
  run cargo bench -p sql-dialect-fmt-formatter --bench format -- --test
  run python3 scripts/check-publish-manifests.py
  run python3 scripts/check-version-consistency.py "$TAG"
  run ./scripts/run-external-corpus.sh --sample
else
  log "Skipping the green gate (--no-gate)"
  run python3 scripts/check-version-consistency.py "$TAG"
fi

# ---- 4. package ------------------------------------------------------------
log "Packaging editor extensions"
run ./scripts/package-extensions.sh "$VERSION"

# ---- 5. commit + tag -------------------------------------------------------
# A re-run at the same version leaves nothing to commit; tag the existing commit
# rather than aborting on git's empty-commit error.
if [ "$DRY_RUN" = false ] && [ -z "$(git status --porcelain)" ]; then
  log "No version changes to commit; tagging the current commit"
else
  log "Committing the release"
  run git commit -am "release: $TAG"
fi

if [ "$VIA_CI" = false ]; then
  log "Tagging $TAG"
  run git tag "$TAG"
fi

# ---- 6. push ---------------------------------------------------------------
if [ "$PUSH" = false ]; then
  log "Skipping push (--no-push)"
  echo
  echo "Release commit is local. Finish with:"
  if [ "$VIA_CI" = true ]; then
    echo "  git push origin $RELEASE_BRANCH && gh workflow run release.yml -f version=$TAG"
  else
    echo "  git push origin $RELEASE_BRANCH && git push origin $TAG"
  fi
  exit 0
fi

log "Pushing $RELEASE_BRANCH"
run git push origin "$RELEASE_BRANCH"

if [ "$VIA_CI" = true ]; then
  log "Dispatching the Release workflow for $TAG"
  run gh workflow run release.yml \
    -f "version=$TAG" \
    -f "publish_crates=$PUBLISH_CRATES"
  echo
  echo "Dispatched. The workflow creates $TAG at the pushed commit."
  echo "Watch it with: gh run watch \$(gh run list --workflow=release.yml --limit=1 --json databaseId -q '.[0].databaseId')"
else
  log "Pushing $TAG"
  run git push origin "$TAG"
  echo
  echo "Pushed $TAG. The Release workflow builds the assets and publishes"
  echo "according to the repository's *_AUTO_PUBLISH variables."
fi
