#!/usr/bin/env python3
"""Generate the spec-derived hover tables in sql-dialect-fmt-hover.

Reads `spec/seed/features.json` and `spec/seed/functions.json` (the curated spec
tracker seeds, which live outside the Cargo workspace and are not packaged with
the published crates) and writes them as static Rust tables to
`crates/sql-dialect-fmt-hover/src/generated.rs`, which is checked in so the
crate stays self-contained on crates.io.

Usage:
    python3 scripts/generate-hover-tables.py          # rewrite generated.rs
    python3 scripts/generate-hover-tables.py --check  # CI: fail if out of sync
"""
import argparse
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FEATURES_JSON = os.path.join(ROOT, "spec", "seed", "features.json")
FUNCTIONS_JSON = os.path.join(ROOT, "spec", "seed", "functions.json")
GENERATED_RS = os.path.join(
    ROOT, "crates", "sql-dialect-fmt-hover", "src", "generated.rs"
)

# Feature names whose hover trigger phrases cannot be derived from the name.
# `[]` opts a feature out (e.g. covered better by type or function hovers).
PHRASE_OVERRIDES = {
    "INNER JOIN": ["JOIN", "INNER JOIN"],
    "OUTER JOIN": [
        "LEFT JOIN",
        "RIGHT JOIN",
        "FULL JOIN",
        "LEFT OUTER JOIN",
        "RIGHT OUTER JOIN",
        "FULL OUTER JOIN",
    ],
    "GROUP BY CUBE/ROLLUP/GROUPING SETS": ["CUBE", "ROLLUP", "GROUPING SETS"],
    "INSERT ALL/FIRST": ["INSERT ALL", "INSERT FIRST"],
    "CREATE STAGE / FILE FORMAT / SEQUENCE": [
        "CREATE STAGE",
        "CREATE FILE FORMAT",
        "CREATE SEQUENCE",
    ],
    "CREATE FUNCTION/PROCEDURE": ["CREATE FUNCTION", "CREATE PROCEDURE"],
    # OBJECT/ARRAY already resolve to the richer type hover.
    "OBJECT/ARRAY constructors": [],
    # Covered by the function table (AI_COMPLETE, SNOWFLAKE.CORTEX.*).
    "Cortex / AISQL": [],
}

# A derivable phrase: uppercase keyword words separated by single spaces.
KEYWORD_PHRASE = re.compile(r"[A-Z][A-Z0-9_]*(?: [A-Z][A-Z0-9_]*)*")


def phrases_for(name: str) -> list[str]:
    if name in PHRASE_OVERRIDES:
        return PHRASE_OVERRIDES[name]
    return [
        variant.strip()
        for variant in name.split("/")
        if KEYWORD_PHRASE.fullmatch(variant.strip())
    ]


def rust_str(text: str) -> str:
    if any(ord(c) < 0x20 for c in text):
        raise SystemExit(f"control character in spec string: {text!r}")
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def feature_rows(spec: dict) -> list[str]:
    default_url = spec["source_base"]
    rows = []
    seen_phrases = {}
    for feature in spec["features"]:
        name = feature["name"]
        phrases = phrases_for(name)
        if not phrases:
            continue
        for phrase in phrases:
            if phrase in seen_phrases:
                raise SystemExit(
                    f"phrase {phrase!r} is claimed by both "
                    f"{seen_phrases[phrase]!r} and {name!r}"
                )
            seen_phrases[phrase] = name
        phrase_lists = ", ".join(
            "&[" + ", ".join(rust_str(word) for word in phrase.split(" ")) + "]"
            for phrase in phrases
        )
        notes = feature.get("notes")
        rows.append(
            "    FeatureDoc {\n"
            f"        phrases: &[{phrase_lists}],\n"
            f"        name: {rust_str(name)},\n"
            f"        syntax: {rust_str(feature['syntax'])},\n"
            f"        status: {rust_str(feature['status'])},\n"
            f"        coverage: {rust_str(feature['coverage'])},\n"
            f"        notes: {'Some(' + rust_str(notes) + ')' if notes else 'None'},\n"
            f"        docs_url: {rust_str(feature.get('source', default_url))},\n"
            "    },"
        )
    return rows


def function_rows(spec: dict) -> list[str]:
    base = spec["source_base"]
    rows = []
    seen = set()
    for function in spec["functions"]:
        name = function["name"]
        if name != name.upper():
            raise SystemExit(f"function name must be uppercase: {name!r}")
        if name in seen:
            raise SystemExit(f"duplicate function {name!r}")
        seen.add(name)
        if "." in name and "source" not in function:
            raise SystemExit(f"qualified function {name!r} needs an explicit source")
        url = function.get("source", f"{base}/{name.lower().replace('$', '_')}")
        rows.append(
            "    FunctionDoc {\n"
            f"        name: {rust_str(name)},\n"
            f"        category: {rust_str(function['category'])},\n"
            f"        signature: {rust_str(function['signature'])},\n"
            f"        returns: {rust_str(function['returns'])},\n"
            f"        summary: {rust_str(function['summary'])},\n"
            f"        status: {rust_str(function['status'])},\n"
            f"        parenless: {'true' if function.get('parenless') else 'false'},\n"
            f"        docs_url: {rust_str(url)},\n"
            "    },"
        )
    return rows


def render() -> str:
    with open(FEATURES_JSON, encoding="utf-8") as f:
        features = json.load(f)
    with open(FUNCTIONS_JSON, encoding="utf-8") as f:
        functions = json.load(f)
    lines = [
        "//! Spec-derived hover tables: Snowflake feature syntax and function signatures.",
        "//!",
        "//! @generated by `python3 scripts/generate-hover-tables.py` from",
        "//! `spec/seed/features.json` and `spec/seed/functions.json`. Do not edit by",
        "//! hand; edit the seed JSON and rerun the script (CI checks the sync).",
        "",
        "/// One spec-tracker feature, hoverable via its keyword `phrases`.",
        "pub(crate) struct FeatureDoc {",
        "    /// Keyword sequences that trigger this hover, e.g. `&[\"GROUP\", \"BY\"]`.",
        "    pub(crate) phrases: &'static [&'static [&'static str]],",
        "    pub(crate) name: &'static str,",
        "    pub(crate) syntax: &'static str,",
        "    /// Snowflake availability: `GA`, `Preview`, or `Deprecated`.",
        "    pub(crate) status: &'static str,",
        "    /// sql-dialect-fmt parser coverage: `parse`, `partial`, or `todo`.",
        "    pub(crate) coverage: &'static str,",
        "    pub(crate) notes: Option<&'static str>,",
        "    pub(crate) docs_url: &'static str,",
        "}",
        "",
        "/// One spec-tracker function signature.",
        "pub(crate) struct FunctionDoc {",
        "    /// Uppercase name; qualified names keep their dots, e.g.",
        "    /// `SNOWFLAKE.CORTEX.COMPLETE`.",
        "    pub(crate) name: &'static str,",
        "    pub(crate) category: &'static str,",
        "    pub(crate) signature: &'static str,",
        "    pub(crate) returns: &'static str,",
        "    pub(crate) summary: &'static str,",
        "    /// Snowflake availability: `GA`, `Preview`, or `Deprecated`.",
        "    pub(crate) status: &'static str,",
        "    /// Snowflake also accepts the function without parentheses.",
        "    pub(crate) parenless: bool,",
        "    pub(crate) docs_url: &'static str,",
        "}",
        "",
        "#[rustfmt::skip]",
        "pub(crate) const FEATURES: &[FeatureDoc] = &[",
        *feature_rows(features),
        "];",
        "",
        "#[rustfmt::skip]",
        "pub(crate) const FUNCTIONS: &[FunctionDoc] = &[",
        *function_rows(functions),
        "];",
    ]
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail instead of writing when generated.rs is out of sync",
    )
    args = parser.parse_args()
    rendered = render()
    try:
        with open(GENERATED_RS, encoding="utf-8", newline="") as f:
            existing = f.read()
    except FileNotFoundError:
        existing = None
    if rendered == existing:
        print(f"{os.path.relpath(GENERATED_RS, ROOT)} is up to date")
        return 0
    if args.check:
        print(
            f"{os.path.relpath(GENERATED_RS, ROOT)} is out of sync with the spec "
            "seeds; run `python3 scripts/generate-hover-tables.py`",
            file=sys.stderr,
        )
        return 1
    with open(GENERATED_RS, "w", encoding="utf-8", newline="") as f:
        f.write(rendered)
    print(f"wrote {os.path.relpath(GENERATED_RS, ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
