#!/usr/bin/env python3
"""Regenerate the committed TextMate grammar keyword alternation.

The editor grammar (`editors/snowflake.tmLanguage.json`) is what colours `.sql`
files, which this extension claims as `snowflake-sql`. To keep other dialects
readable there, the keyword rule lists every reserved word from the Rust keyword
table (all dialects), not just Snowflake's. This script is the single writer of
that rule so the grammar and the parser cannot drift.

It also derives `editors/sql.tmLanguage.json`, the dialect-neutral grammar the
per-dialect language ids share (name/scope differ; the rules are identical).

Usage:
    python3 scripts/generate-textmate-keywords.py          # rewrite in place
    python3 scripts/generate-textmate-keywords.py --check  # non-zero if stale
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
KEYWORD_RS = ROOT / "crates" / "sql-dialect-fmt-syntax" / "src" / "keyword.rs"
GRAMMAR_JSON = ROOT / "editors" / "snowflake.tmLanguage.json"
NEUTRAL_GRAMMAR_JSON = ROOT / "editors" / "sql.tmLanguage.json"

# `("select", SELECT_KW, ...)` / multi-line `(\n  "ilike",\n  ILIKE_KW,` — the lowercase
# spelling is followed by the `_KW` enum variant (possibly on the next line).
ENTRY_RE = re.compile(r'"([a-z0-9_]+)",\s*([A-Z][A-Z0-9_]*_KW)')


def keyword_words() -> list[str]:
    text = KEYWORD_RS.read_text(encoding="utf-8")
    start = text.find("const KEYWORDS")
    end = text.find("const MAX_KEYWORD_LEN")
    block = text[start:end] if start != -1 and end != -1 else text
    words = sorted({match[0] for match in ENTRY_RE.findall(block)})
    if not words:
        raise SystemExit("no keywords parsed from keyword.rs")
    return words


def desired_match(words: list[str]) -> str:
    # The pattern is JSON-escaped `(?i)\b(a|b|…)\b`.
    return "(?i)\\\\b(" + "|".join(words) + ")\\\\b"


def current_match(text: str) -> tuple[int, int, str]:
    """Return (value_start, value_end, value) for the keywords rule's `match` string."""
    anchor = '"keywords": {'
    i = text.find(anchor)
    if i < 0:
        raise SystemExit("could not find the `keywords` rule")
    j = text.find('"match": "', i)
    if j < 0:
        raise SystemExit("could not find the `keywords` match")
    start = j + len('"match": "')
    end = text.find('"', start)
    if end < 0:
        raise SystemExit("unterminated keywords match")
    return start, end, text[start:end]


def neutral_grammar(snowflake_text: str) -> str:
    grammar = json.loads(snowflake_text)
    grammar["name"] = "SQL (sql-dialect-fmt)"
    grammar["scopeName"] = "source.sql.dialect-fmt"
    return json.dumps(grammar, indent=2, ensure_ascii=False) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit non-zero if the grammar is stale instead of rewriting it",
    )
    args = parser.parse_args()

    words = keyword_words()
    text = GRAMMAR_JSON.read_text(encoding="utf-8")
    start, end, existing = current_match(text)
    desired = desired_match(words)

    stale = existing != desired
    updated = text[:start] + desired + text[end:]
    neutral = neutral_grammar(updated)

    if args.check:
        if stale:
            print(
                "grammar keyword list is stale; run scripts/generate-textmate-keywords.py",
                file=sys.stderr,
            )
            return 1
        if not NEUTRAL_GRAMMAR_JSON.exists() or NEUTRAL_GRAMMAR_JSON.read_text(
            encoding="utf-8"
        ) != neutral:
            print(
                "editors/sql.tmLanguage.json is stale; run scripts/generate-textmate-keywords.py",
                file=sys.stderr,
            )
            return 1
        print(f"grammar keyword lists are current ({len(words)} words)")
        return 0

    if stale:
        GRAMMAR_JSON.write_text(updated, encoding="utf-8")
        print(f"updated grammar keyword list ({len(words)} words)")
    else:
        print(f"grammar keyword list is current ({len(words)} words)")

    if not NEUTRAL_GRAMMAR_JSON.exists() or NEUTRAL_GRAMMAR_JSON.read_text(
        encoding="utf-8"
    ) != neutral:
        NEUTRAL_GRAMMAR_JSON.write_text(neutral, encoding="utf-8")
        print("updated editors/sql.tmLanguage.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
