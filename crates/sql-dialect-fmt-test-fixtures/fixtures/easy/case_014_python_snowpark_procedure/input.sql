-- Case 014: Python Snowpark procedure with multilingual text profiling and nested DataFrame SQL
CREATE OR REPLACE PROCEDURE OPS.SP_PY_PROFILE_TEXT_COLUMNS(P_SOURCE_TABLE STRING,P_TEXT_COLUMN STRING,P_LIMIT NUMBER DEFAULT 5000) RETURNS VARIANT LANGUAGE PYTHON RUNTIME_VERSION = '3.12' PACKAGES =( 'snowflake-snowpark-python') HANDLER = 'main' EXECUTE AS CALLER AS
$$
import re
import unicodedata
from snowflake.snowpark import Session
from snowflake.snowpark.functions import col, length, lit, regexp_count

SCRIPT_RE = {
    "latin": re.compile(r"[A-Za-z]"),
    "hiragana": re.compile(r"[\u3040-\u309F]"),
    "katakana": re.compile(r"[\u30A0-\u30FF]"),
    "hangul": re.compile(r"[\uAC00-\uD7AF]"),
    "arabic": re.compile(r"[\u0600-\u06FF]"),
}

def normalize_text(value):
    if value is None:
        return ""
    return unicodedata.normalize("NFKC", str(value)).strip()

def detect_scripts(value):
    normalized = normalize_text(value)
    return [name for name, pattern in SCRIPT_RE.items() if pattern.search(normalized)]

def main(session: Session, p_source_table: str, p_text_column: str, p_limit: int):
    quoted_table = '"' + p_source_table.replace('"', '""') + '"' if '.' not in p_source_table else p_source_table
    sql = f'''
        SELECT
            {p_text_column} AS text_value,
            COUNT(*) AS row_count,
            MIN(created_at) AS first_seen_at,
            MAX(created_at) AS last_seen_at
        FROM {quoted_table}
        WHERE {p_text_column} IS NOT NULL
        GROUP BY {p_text_column}
        ORDER BY row_count DESC, text_value
        LIMIT ?
    '''
    rows = session.sql(sql, params=[int(p_limit)]).collect()
    summary = {
        "source_table": p_source_table,
        "text_column": p_text_column,
        "sample_size": len(rows),
        "scripts": {},
        "examples": []
    }

    for row in rows:
        text = normalize_text(row["TEXT_VALUE"])
        scripts = detect_scripts(text)
        for script in scripts or ["unknown"]:
            summary["scripts"].setdefault(script, 0)
            summary["scripts"][script] += int(row["ROW_COUNT"])
        if len(summary["examples"]) < 25:
            summary["examples"].append({
                "text": text,
                "scripts": scripts,
                "row_count": int(row["ROW_COUNT"]),
                "first_seen_at": str(row["FIRST_SEEN_AT"]),
                "last_seen_at": str(row["LAST_SEEN_AT"])
            })

    return summary
$$;
