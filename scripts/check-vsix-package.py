#!/usr/bin/env python3
"""Validate that a VSIX is self-contained, bundled, and free of npm dependency trees."""

from __future__ import annotations

import json
import sys
import zipfile
from pathlib import Path


def validate(path: Path) -> None:
    if not path.is_file():
        raise ValueError(f"VSIX does not exist: {path}")

    with zipfile.ZipFile(path) as archive:
        names = sorted(name for name in archive.namelist() if not name.endswith("/"))
        packaged = [name for name in names if name.startswith("extension/")]
        required = {
            "extension/dist/extension.js",
            "extension/package.json",
            "extension/snowflake.tmLanguage.json",
            "extension/vendor/sql_dialect_fmt_wasm.wasm",
        }
        missing = sorted(required.difference(packaged))
        if missing:
            raise ValueError(f"VSIX is missing required files: {missing}")

        dependency_files = [name for name in packaged if "/node_modules/" in name]
        if dependency_files:
            raise ValueError(
                f"VSIX contains {len(dependency_files)} node_modules files; expected a bundle"
            )

        javascript = [name for name in packaged if name.endswith((".js", ".cjs", ".mjs"))]
        if javascript != ["extension/dist/extension.js"]:
            raise ValueError(f"VSIX should contain one JavaScript bundle, got {javascript}")
        if len(packaged) > 30:
            raise ValueError(f"VSIX contains {len(packaged)} extension files; expected <= 30")

        manifest = json.loads(archive.read("extension/package.json"))
        if manifest.get("main") != "./dist/extension.js":
            raise ValueError("packaged manifest does not point at dist/extension.js")
        grammars = manifest.get("contributes", {}).get("grammars", [])
        snowflake_grammar = next(
            (
                grammar
                for grammar in grammars
                if grammar.get("scopeName") == "source.snowflake-sql"
            ),
            None,
        )
        if snowflake_grammar is None:
            raise ValueError("packaged manifest does not contribute the Snowflake grammar")
        embedded_languages = snowflake_grammar.get("embeddedLanguages", {})
        if (
            embedded_languages.get("meta.embedded.block.javascript.snowflake")
            != "javascript"
        ):
            raise ValueError("packaged manifest does not map the embedded JavaScript scope")

        bundle = archive.read("extension/dist/extension.js")
        if len(bundle) < 100_000:
            raise ValueError(f"extension bundle is unexpectedly small ({len(bundle)} bytes)")
        if b"vscode-languageclient" not in bundle or b"LanguageClient" not in bundle:
            raise ValueError("extension bundle does not appear to contain vscode-languageclient")

    print(
        f"VSIX package validation ok: {len(packaged)} extension files, "
        f"{len(javascript)} JavaScript bundle, no node_modules"
    )


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: scripts/check-vsix-package.py PATH_TO.vsix", file=sys.stderr)
        return 2
    try:
        validate(Path(sys.argv[1]))
    except (OSError, ValueError, zipfile.BadZipFile, json.JSONDecodeError) as error:
        print(f"VSIX package validation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
