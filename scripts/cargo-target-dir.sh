#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo metadata \
  --format-version 1 \
  --no-deps \
  --manifest-path "$ROOT_DIR/Cargo.toml" \
  | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])'
