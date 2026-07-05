#!/usr/bin/env python3
"""Check that release/package versions are kept in sync."""

from __future__ import annotations

import json
import pathlib
import re
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]


def load_cargo_version() -> str:
    with (ROOT / "Cargo.toml").open("rb") as handle:
        return tomllib.load(handle)["workspace"]["package"]["version"]


def load_json_version(path: pathlib.Path) -> str:
    with path.open(encoding="utf-8") as handle:
        value = json.load(handle)["version"]
    if not isinstance(value, str):
        raise TypeError(f"{path.relative_to(ROOT)} version must be a string")
    return value


def normalize_expected(raw: str) -> str:
    version = raw[1:] if raw.startswith("v") else raw
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", version):
        raise ValueError(f"expected version must look like v1.2.3 or 1.2.3: {raw}")
    return version


def main() -> int:
    versions = {
        "Cargo.toml": load_cargo_version(),
        "editors/package.json": load_json_version(ROOT / "editors" / "package.json"),
        "extensions/chrome/manifest.json": load_json_version(
            ROOT / "extensions" / "chrome" / "manifest.json"
        ),
    }

    if len(sys.argv) > 1 and sys.argv[1]:
        versions["release version"] = normalize_expected(sys.argv[1])

    expected = next(iter(versions.values()))
    mismatches = {name: version for name, version in versions.items() if version != expected}
    if mismatches:
        print("version mismatch:", file=sys.stderr)
        for name, version in versions.items():
            print(f"  {name}: {version}", file=sys.stderr)
        return 1

    print(f"version consistency ok: {expected}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
