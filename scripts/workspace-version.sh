#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$ROOT_DIR/Cargo.toml" <<'PY'
import sys
import tomllib

with open(sys.argv[1], "rb") as f:
    manifest = tomllib.load(f)

print(manifest["workspace"]["package"]["version"])
PY
