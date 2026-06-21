#!/usr/bin/env bash
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")" && pwd)}"
python3 - <<'PY_SMOKE' "$root"
import json
import pathlib
import sys
root = pathlib.Path(sys.argv[1])
manifest = json.loads((root / 'manifest.json').read_text(encoding='utf-8'))
assert len(manifest) == 30, f"expected 30 cases, got {len(manifest)}"
for item in manifest:
    inp = root / item['input_path']
    exp = root / item['expected_path']
    assert inp.exists(), inp
    assert exp.exists(), exp
    assert exp.read_text(encoding='utf-8').count(';') >= 1, exp
print(f"OK: {len(manifest)} Snowflake formatter cases found")
PY_SMOKE
