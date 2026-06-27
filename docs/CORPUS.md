# Corpus regression harness

The corpus harness keeps the formatter honest against large, real-world SQL it has never seen. It
does not assert a particular formatting style for arbitrary SQL; it asserts the invariants that must
hold for every input:

1. `parse` never panics.
2. Formatting preserves the case-folded stream of significant non-trivia tokens.
3. `format(format(x)) == format(x)`.
4. Formatting clean input yields output that reparses cleanly.

The checks live in `crates/sql-dialect-fmt-formatter/tests/external_corpus.rs`. The same `check_file`
function backs both the always-on sample corpus and the opt-in external corpus, so the two paths do
not drift.

## Always-On Sample Corpus

`crates/sql-dialect-fmt-formatter/tests/corpus_sample/` holds a small curated set of `.sql` files:

| File | Covers |
| --- | --- |
| `01_select.sql` | CTE, joins, window functions, semi-structured access |
| `02_dml.sql` | `INSERT` / `UPDATE` / `DELETE` / `MERGE` |
| `03_ddl.sql` | `CREATE TABLE` / `VIEW` / `WAREHOUSE` |
| `04_copy.sql` | `COPY INTO` load and unload |
| `05_scripting.sql` | Snowflake Scripting procedure with a SQL body |
| `06_semantic_view.sql` | `CREATE SEMANTIC VIEW` |

`sample_corpus_is_clean` runs as part of `cargo test --workspace`. In addition to the invariants
above, each committed sample must already be in formatter-canonical form: `format(x) == x`.

Regenerate the samples after an intentional formatting change with:

```sh
cargo run -p sql-dialect-fmt --bin sql-dialect-fmt -- --write --no-config \
  crates/sql-dialect-fmt-formatter/tests/corpus_sample
```

Keep this set small and representative. Broad generated or private corpora belong behind the
external harness below.

## External Corpus

Point the harness at one or more local files or directories:

```sh
SQL_DIALECT_FMT_EXTERNAL_CORPUS=/path/to/sqls \
  cargo test -p sql-dialect-fmt-formatter --test external_corpus -- --ignored
```

`SQL_DIALECT_FMT_EXTERNAL_CORPUS` accepts a path-list of files and directories. Directories are
recursed for `*.sql` files case-insensitively. Relative paths are resolved from Cargo's test working
directory and, if needed, from the workspace root so CI can pass `crates/...` paths directly.
`SQL_DIALECT_FMT_EXTERNAL_CORPUS_LIMIT` caps the number of files for quick smoke runs, and non-UTF-8
files are skipped.

The wrapper script supports local paths, the committed sample corpus, and downloaded archives:

```sh
scripts/run-external-corpus.sh --sample
scripts/run-external-corpus.sh --path /path/to/sqls --limit 500
scripts/run-external-corpus.sh --url https://example.com/sql-corpus.tar.gz --limit 500
```

## Conformance Report Generator

`scripts/conformance-report.py` mines `.sql` files and SQL fenced code blocks from a local
directory/file/archive or a downloaded archive, writes a temporary corpus, runs the same external
corpus harness, and emits a Markdown parser/formatter report:

```sh
scripts/conformance-report.py --path crates/sql-dialect-fmt-formatter/tests/corpus_sample \
  --out target/conformance-report.md
scripts/conformance-report.py --url https://example.com/docs-or-examples.tar.gz --limit 500
```

This is the 1.0 lane for official-spec-derived coverage: it does not replace the handwritten CST
parser, but it gives every docs/examples sweep a repeatable parser-gap report and reuses the same
losslessness/idempotency invariants as CI.

## Continuous Operation

`.github/workflows/corpus.yml` runs on every pull request, on `main`, and weekly. By default it
checks the committed sample corpus. To make the weekly run cover a broader private or generated
corpus, configure repository variables:

| Variable | Meaning |
| --- | --- |
| `SQL_DIALECT_FMT_EXTERNAL_CORPUS_URL` | Optional `.tar.gz`, `.tgz`, `.tar`, or `.zip` archive URL containing `.sql` files. |
| `SQL_DIALECT_FMT_EXTERNAL_CORPUS_LIMIT` | Optional cap for smoke runs over very large corpora. |

The same values can be supplied through the workflow-dispatch inputs `corpus_url` and
`corpus_limit` for one-off runs.

The repository is currently configured to run the external workflow against a pinned public
dbt corpus seed:

```text
https://github.com/dbt-labs/jaffle-shop/archive/08ef1d578de5b55f226aae34f30d7077df8e9f35.tar.gz
```

That seed is intentionally not vendored into this repository; update the repository variable when
rotating to a broader public or private corpus.

The external corpus does not need to be preformatted; only the invariants are checked. The run
collects every offending file before failing, so one pass gives the full list.

## Triaging A Failure

Reproduce the file in isolation first:

```sh
cargo run -p sql-dialect-fmt --bin sql-dialect-fmt -- --no-config path/to/offender.sql > /tmp/once.sql
cargo run -p sql-dialect-fmt --bin sql-dialect-fmt -- --no-config /tmp/once.sql > /tmp/twice.sql
diff /tmp/once.sql /tmp/twice.sql
```

`not idempotent` usually points to a lowering rule that produces different structure on the second
pass. `significant tokens changed across formatting` is a losslessness bug. `formatted output does
not reparse cleanly` usually means a formatter emission bug or a parser gap for output the formatter
now emits.
