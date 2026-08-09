#!/usr/bin/env python3
"""Validate VS Code Marketplace artwork and package references."""

from __future__ import annotations

import json
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

PNG_ASSETS = {
    "editors/images/icon.png": (256, 256, True),
    "editors/images/syntax-highlighting.png": (1280, 800, False),
}

CSS_VARIABLE_DEFINITION = re.compile(r"(--sdf-[a-z0-9-]+)\s*:")
CSS_VARIABLE_REFERENCE = re.compile(r"var\((--sdf-[a-z0-9-]+)")


def read_png(path: Path) -> tuple[int, int, bool]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        raise ValueError(f"{path.relative_to(ROOT)} is not a PNG with an IHDR header")
    width, height = struct.unpack(">II", data[16:24])
    has_alpha = data[25] in (4, 6) or b"tRNS" in data
    return width, height, has_alpha


def validate_pngs() -> None:
    for relative, expected in PNG_ASSETS.items():
        actual = read_png(ROOT / relative)
        expected_width, expected_height, requires_alpha = expected
        if actual[:2] != (expected_width, expected_height):
            raise ValueError(
                f"{relative}: expected {expected_width}x{expected_height}, got {actual[:2]}"
            )
        if requires_alpha and not actual[2]:
            raise ValueError(f"{relative}: expected an alpha channel")


def validate_manifest() -> None:
    manifest = json.loads((ROOT / "editors/package.json").read_text())
    if manifest.get("icon") != "images/icon.png":
        raise ValueError("editors/package.json: icon must reference images/icon.png")
    packaged = set(manifest.get("files", []))
    for required in (
        "dist/extension.js",
        "images/icon.png",
        "images/syntax-highlighting.png",
        "vendor/sql_dialect_fmt_wasm.wasm",
    ):
        if required not in packaged:
            raise ValueError(f"editors/package.json: files must include {required}")
    if manifest.get("main") != "./dist/extension.js":
        raise ValueError("editors/package.json: main must reference the bundled extension")
    if "node_modules/**" in packaged:
        raise ValueError("editors/package.json: package the bundle, not node_modules")
    for dependency in ("vscode-languageclient", "esbuild"):
        if dependency not in manifest.get("devDependencies", {}):
            raise ValueError(f"editors/package.json: missing devDependency {dependency}")


def validate_docs_css_tokens() -> None:
    token_path = ROOT / "docs-site/theme/tokens.css"
    consumer_path = ROOT / "docs-site/theme/playground.css"
    definitions = set(CSS_VARIABLE_DEFINITION.findall(token_path.read_text()))
    missing = set(CSS_VARIABLE_REFERENCE.findall(consumer_path.read_text())) - definitions
    if missing:
        raise ValueError(f"docs-site/theme/playground.css: undefined CSS tokens: {sorted(missing)}")

    book = (ROOT / "docs-site/book.toml").read_text()
    expected = 'additional-css = ["theme/tokens.css", "theme/playground.css"]'
    if expected not in book:
        raise ValueError("docs-site/book.toml must load tokens.css before playground.css")


def main() -> int:
    try:
        validate_pngs()
        validate_manifest()
        validate_docs_css_tokens()
    except (FileNotFoundError, ValueError, json.JSONDecodeError) as error:
        print(f"VS Code Marketplace asset validation failed: {error}", file=sys.stderr)
        return 1
    print(f"VS Code Marketplace asset validation ok: {len(PNG_ASSETS)} PNGs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
