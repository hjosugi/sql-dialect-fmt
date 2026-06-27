# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The published crates share a single workspace version (see `RELEASING.md`).

## [Unreleased]

## [1.0.0] - 2026-06-27

This is the first release line of **sql-dialect-fmt**, a Rust toolchain for formatting and
highlighting Snowflake SQL and Databricks SQL. The format is mechanically **lossless and idempotent**:
unparseable input passes through unchanged, significant tokens and comments are
preserved, and `format(format(x)) == format(x)`.

### Added

- **Lossless lexer** (`sql-dialect-fmt-lexer`): hand-written, error-resilient tokenizer
  covering Snowflake operators (`|>`, `->>`, `::`, `$$…$$` dollar-quoted bodies),
  all three comment styles, and string escapes; byte-exact round-trip.
- **Error-resilient parser** (`sql-dialect-fmt-parser`): event-based recursive-descent
  parser producing a lossless `rowan` CST. Never panics, never fails.
  - SELECT pipeline, `JOIN` / `ORDER BY` / `GROUP BY`, `CASE`, subqueries / CTEs,
    set operations.
  - Aggregate `DISTINCT`, `WITHIN GROUP`, `PIVOT` / `UNPIVOT`,
    `GROUPING SETS` / `CUBE` / `ROLLUP`, `LATERAL FLATTEN` / table functions /
    named arguments, `MATCH_RECOGNIZE`, `ASOF JOIN`, time-travel `AT` / `BEFORE`,
    `IS [NOT] DISTINCT FROM`, `FROM VALUES`, quantified comparisons, nested `WITH`.
  - DML: `INSERT` (single / `OVERWRITE` / `ALL` / `FIRST`), `UPDATE`, `DELETE`,
    `MERGE`.
  - DDL: `CREATE TABLE` / `VIEW` / CTAS, `DROP`, lenient `ALTER`,
    `CREATE PROCEDURE` / `FUNCTION` skeletons with verbatim `$$…$$` bodies.
  - `COPY INTO` (load / unload, stage paths preserved verbatim).
  - Databricks dialect slice: backtick identifiers, `LATERAL VIEW`, Delta table
    options, `VERSION` / `TIMESTAMP AS OF`, and higher-order-function lambdas.
- **Formatter** (`sql-dialect-fmt-formatter`): generic Wadler/Prettier-style Doc IR engine
  with a width-aware printer, plus Snowflake formatting rules built on the CST.
  Headline feature: **magic trailing comma**. Real comment attachment
  (leading / trailing / dangling). East Asian Width aware line measurement.
- **Highlighter** (`sql-dialect-fmt-highlight`): lexical token classification
  (keyword / type / string / comment / operator / variable) with byte ranges.
- **Hover** (`sql-dialect-fmt-hover`): editor-ready hover text for Snowflake types,
  procedures, and tasks.
- **Syntax core** (`sql-dialect-fmt-syntax`): `SyntaxKind`, keyword recognition, and the
  `rowan` language definition shared across the toolchain.
- **Encoding** (`sql-dialect-fmt-encoding`): byte-to-text decoding/re-encoding helpers so
  the CLI preserves the input's original encoding and line endings.
- **CLI** (`sql-dialect-fmt`): the `sql-dialect-fmt` binary (plus `sql-dialect-fmt`
  compatibility alias) with `--write`, `--check`, stdin/stdout, `--dialect`,
  and `--line-width` / `--indent-width` / `--no-uppercase` options.
- **LSP** (`sql-dialect-fmt-lsp`): Language Server providing formatting, semantic tokens,
  diagnostics, hover, folding ranges, and first-pass lint warnings over stdio.
- **Conformance generator** (`scripts/conformance-report.py`): mines `.sql` files and SQL
  fenced blocks from local paths or archives, then runs the external corpus harness and emits
  a parser/formatter conformance report.
- **Distribution packages**: GitHub Release assets for the CLI binary, Snowsight Chrome
  extension zip, and VS Code VSIX, plus manual Marketplace/Web Store publish workflow gates.

### Notes

- `sql-dialect-fmt-tree-sitter`, `sql-dialect-fmt-test-fixtures`, and `sql-dialect-fmt-test-support` are
  internal crates and are **not published** to crates.io.

[Unreleased]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v0.1.0...v1.0.0
