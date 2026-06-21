#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path

ORDERED_E2E_FILES = (
    "00_bootstrap.sql",
    "01_schema.sql",
    "02_seed.sql",
    "03_procedures.sql",
    "04_run_pipeline.sql",
    "05_analytics.sql",
    "06_tasks.sql",
    "07_assertions.sql",
)
LANGUAGE_PATTERN = re.compile(r"LANGUAGE\s+(JAVASCRIPT|PYTHON)\b", re.IGNORECASE)


def extract_bodies(text: str) -> dict[str, list[str]]:
    bodies: dict[str, list[str]] = {"JAVASCRIPT": [], "PYTHON": []}
    for match in LANGUAGE_PATTERN.finditer(text):
        language = match.group(1).upper()
        start = text.find("$$", match.end())
        end = text.find("$$", start + 2) if start >= 0 else -1
        if start < 0 or end < 0:
            raise ValueError(f"Unclosed {language} body")
        bodies[language].append(text[start + 2 : end])
    return bodies


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    sql_dir = root / "e2e" / "sql"
    final_dir = root / "e2e" / "final"

    component_text = "\n\n".join(
        (sql_dir / name).read_text(encoding="utf-8").rstrip("\n")
        for name in ORDERED_E2E_FILES
    ) + "\n"
    expected_full = (final_dir / "expected_formatted.sql").read_text(encoding="utf-8")
    if component_text != expected_full:
        print("FAIL: e2e/final/expected_formatted.sql is not the exact ordered component concatenation")
        return 1

    raw = (final_dir / "input_unformatted.sql").read_text(encoding="utf-8")
    expected_sql_only = (final_dir / "expected_sql_only.sql").read_text(encoding="utf-8")
    raw_bodies = extract_bodies(raw)
    sql_only_bodies = extract_bodies(expected_sql_only)
    if raw_bodies != sql_only_bodies:
        print("FAIL: expected_sql_only.sql does not preserve JavaScript/Python body bytes")
        return 1

    full_bodies = extract_bodies(expected_full)
    body_counts = {language: len(values) for language, values in full_bodies.items()}
    if body_counts != {"JAVASCRIPT": 1, "PYTHON": 1}:
        print(f"FAIL: unexpected E2E embedded body counts: {body_counts}")
        return 1

    if "CALL OPS.SP_VALIDATE_E2E();" not in expected_full:
        print("FAIL: final scenario does not invoke the semantic assertion procedure")
        return 1

    print(
        "OK: final E2E matches 8 ordered components; "
        "SQL-only answer preserves JS/Python bodies; assertions are invoked"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
