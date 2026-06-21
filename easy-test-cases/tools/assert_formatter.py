#!/usr/bin/env python3
from __future__ import annotations

import argparse
import difflib
import json
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--formatter",
        required=True,
        help="Command template; include {file} where the temporary SQL path belongs.",
    )
    parser.add_argument("--profile", choices=("full", "sql-only"), default="full")
    parser.add_argument("--case", action="append", dest="cases")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    selected = set(args.cases or [])
    failures = 0

    for case in manifest["cases"]:
        if selected and case["name"] not in selected:
            continue

        expected_key = "expected_full"
        if args.profile == "sql-only" and "expected_sql_only" in case:
            expected_key = "expected_sql_only"

        source = root / case["input"]
        expected_path = root / case[expected_key]
        expected = expected_path.read_text(encoding="utf-8")

        with tempfile.TemporaryDirectory(prefix="sf-format-") as tmp:
            work = Path(tmp) / source.name
            work.write_bytes(source.read_bytes())
            command = args.formatter.replace("{file}", shlex.quote(str(work)))
            result = subprocess.run(command, shell=True, text=True, capture_output=True)

            if result.returncode != 0:
                failures += 1
                print(f"FAIL {case['name']}: formatter exited {result.returncode}")
                print(result.stdout)
                print(result.stderr, file=sys.stderr)
                continue

            actual = work.read_text(encoding="utf-8")
            if actual != expected:
                failures += 1
                print(f"FAIL {case['name']}: output differs from {expected_path.name}")
                diff = difflib.unified_diff(
                    expected.splitlines(True),
                    actual.splitlines(True),
                    fromfile=str(expected_path),
                    tofile=str(work),
                )
                sys.stdout.writelines(diff)
            else:
                print(f"PASS {case['name']}")

    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
