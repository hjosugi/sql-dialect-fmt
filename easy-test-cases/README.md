# Snowflake Formatter Torture Suite

A large Snowflake SQL formatter corpus with deliberately ugly inputs and canonical answers.
It covers standalone SQL, Snowflake Scripting, JavaScript and Python stored procedures,
semi-structured data, `COPY INTO`, `MERGE`, streams, task graphs, analytical syntax, quoted
Unicode identifiers, and multilingual data. See `REFERENCES.md` for the official syntax sources.

## Layout

```text
cases/<case>/input.sql                 intentionally damaged layout
cases/<case>/expected.sql              canonical full-format answer
cases/<case>/expected_sql_only.sql     optional answer that preserves JS/Python body bytes
e2e/sql/*.sql                          runnable scenario in execution order
e2e/final/input_unformatted.sql        one huge scenario input
e2e/final/expected_formatted.sql       one huge canonical answer
e2e/final/expected_sql_only.sql        huge answer preserving JS/Python body bytes
e2e/final/expected_results.md          semantic row-count expectations
tools/assert_formatter.py              command-based golden-file harness
tools/lexical_checks.py                UTF-8, delimiter, and fixture checks
```

## What is intentionally difficult

- `DECLARE`, loops, nested blocks, `RESULTSET`, transaction control, custom exceptions,
  `SQLCODE` / `SQLERRM` / `SQLSTATE`, dynamic SQL, binds, and `MERGE`.
- JavaScript template strings containing SQL, regex literals, JSON objects, and Snowflake
  statement/result-set APIs.
- Python type hints, comprehensions, dictionaries, Unicode regex ranges, NFKC normalization,
  Snowpark DataFrames, and nested indentation inside a `$$` body.
- JSON path operators, `::` casts, named arguments with `=>`, `LATERAL FLATTEN`, windows,
  `QUALIFY`, recursive CTEs, `MATCH_RECOGNIZE`, `PIVOT`, and `UNPIVOT`.
- Japanese, Korean, Simplified Chinese, Arabic, Hebrew, Hindi, Thai, French accents,
  apostrophes, RTL text, full-width characters, emoji ZWJ sequences, and decomposed accents.
- `COPY INTO` metadata columns, regex patterns, file-format options, streams, triggered tasks,
  child tasks, and a finalizer task.

## Golden test

Replace the example formatter command with yours. The harness copies every input to a temporary
location, runs the formatter, and compares it byte-for-byte with the selected answer.

```bash
python tools/assert_formatter.py \
  --formatter 'your-formatter --dialect snowflake --write {file}' \
  --profile full
```

For a formatter that treats `$$` bodies as opaque:

```bash
python tools/assert_formatter.py \
  --formatter 'your-formatter --dialect snowflake --write {file}' \
  --profile sql-only
```

`{file}` is replaced with a shell-quoted temporary file path. A unified diff is printed on
failure.

## E2E run order

Run with a role allowed to create a disposable database and warehouse:

```text
00_bootstrap.sql
01_schema.sql
02_seed.sql
03_procedures.sql
04_run_pipeline.sql
05_analytics.sql
06_tasks.sql
07_assertions.sql
```

The scenario creates an X-SMALL auto-suspending warehouse and a disposable database named
`SNOWFLAKE_FORMATTER_LAB`. Tasks are created suspended and are not resumed. Use
`99_cleanup.sql` when finished.

`20_optional_copy.sql` and `e2e/data/*` are additional `COPY INTO` fixtures. They are excluded
from deterministic assertions because local `PUT` paths and client capabilities vary.

## Meaning of “correct”

Formatting has no universal single answer. Here, “correct” means:

1. exact conformance to `STYLE_GUIDE.md`, and
2. no semantic damage to Snowflake SQL or embedded-language bodies.

The E2E assertions provide a second correctness layer for environments where the scenario is
executed against Snowflake. The final E2E pair is also registered as case
`07_e2e_final_scenario`, so the same golden-file harness tests it.

## Safety

The E2E setup uses `CREATE OR REPLACE` and the cleanup file drops the entire lab database and
warehouse. Do not rename them to production objects without reviewing every statement.

## Local fixture validation

```bash
make check
```

This checks UTF-8/LF, delimiters, manifest paths, Python syntax, JavaScript syntax when
Node.js is available, and final-scenario composition. It does not replace execution against a
real Snowflake account; no Snowflake account was available during artifact generation.
