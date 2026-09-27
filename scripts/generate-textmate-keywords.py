#!/usr/bin/env python3
"""Regenerate the keyword alternation in the committed TextMate grammar.

The editor grammar (`editors/snowflake.tmLanguage.json`) is what colours `.sql`
files, which this extension claims as `snowflake-sql`. To keep other dialects
readable there, the keyword rule lists every reserved word from the Rust keyword
table (all dialects), not just Snowflake's. This script is the single writer of
that rule so the grammar and the parser cannot drift.

Usage:
    python3 scripts/generate-textmate-keywords.py          # rewrite in place
    python3 scripts/generate-textmate-keywords.py --check  # non-zero if stale
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
KEYWORD_RS = ROOT / "crates" / "sql-dialect-fmt-syntax" / "src" / "keyword.rs"
GRAMMAR_JSON = ROOT / "editors" / "snowflake.tmLanguage.json"

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

    if existing == desired:
        print(f"grammar keyword list is current ({len(words)} words)")
        return 0

    if args.check:
        print(
            "grammar keyword list is stale; run scripts/generate-textmate-keywords.py",
            file=sys.stderr,
        )
        return 1

    GRAMMAR_JSON.write_text(text[:start] + desired + text[end:], encoding="utf-8")
    print(f"updated grammar keyword list ({len(words)} words)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
