# Corpus regression harness

The corpus harness keeps the formatter honest against large, real-world SQL it has never seen. It
does not assert a particular formatting style for arbitrary SQL; it asserts the invariants that must
hold for every input:

1. `parse` never panics.
2. Formatting preserves the case-folded stream of significant non-trivia tokens.
3. `format(format(x)) == format(x)`.
4. Formatting clean input yields output that reparses cleanly.

The checks live in `crates/snow-fmt-formatter/tests/external_corpus.rs`. The same `check_file`
function backs both the always-on sample corpus and the opt-in external corpus, so the two paths do
not drift.

## Always-On Sample Corpus

`crates/snow-fmt-formatter/tests/corpus_sample/` holds a small curated set of `.sql` files:

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
cargo run -p snow-fmt-cli --bin snow-fmt -- --write --no-config \
  crates/snow-fmt-formatter/tests/corpus_sample
```

Keep this set small and representative. Broad generated or private corpora belong behind the
external harness below.

## External Corpus

Point the harness at one or more local files or directories:

```sh
SNOW_FMT_EXTERNAL_CORPUS=/path/to/sqls \
  cargo test -p snow-fmt-formatter --test external_corpus -- --ignored
```

`SNOW_FMT_EXTERNAL_CORPUS` accepts a path-list of files and directories. Directories are recursed
for `*.sql` files case-insensitively. Relative paths are resolved from Cargo's test working
directory and, if needed, from the workspace root so CI can pass `crates/...` paths directly.
`SNOW_FMT_EXTERNAL_CORPUS_LIMIT` caps the number of files for quick smoke runs, and non-UTF-8 files
are skipped.

The external corpus does not need to be preformatted; only the invariants are checked. The run
collects every offending file before failing, so one pass gives the full list.

## Triaging A Failure

Reproduce the file in isolation first:

```sh
cargo run -p snow-fmt-cli --bin snow-fmt -- --no-config path/to/offender.sql > /tmp/once.sql
cargo run -p snow-fmt-cli --bin snow-fmt -- --no-config /tmp/once.sql > /tmp/twice.sql
diff /tmp/once.sql /tmp/twice.sql
```

`not idempotent` usually points to a lowering rule that produces different structure on the second
pass. `significant tokens changed across formatting` is a losslessness bug. `formatted output does
not reparse cleanly` usually means a formatter emission bug or a parser gap for output the formatter
now emits.
