# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The published crates share a single workspace version (see `RELEASING.md`).

## [Unreleased]

## [1.4.0] - 2026-07-09

### Added

- Added formatter keyword casing modes (`upper`, `lower`, `preserve`) and line-ending modes
  (`lf`, `crlf`, `auto`) across the API, CLI flags, and `sql-dialect-fmt.toml`.
- Added formatter off/on region directives (`-- sql-dialect-fmt: off/on`, `-- snowfmt: off/on`,
  and `-- fmt: off/on`) so intentionally hand-written SQL can be preserved verbatim.
- Added LSP support for `positionEncoding` negotiation, UTF-8 ranges, formatting options, and
  initialization / `workspace/didChangeConfiguration` settings.

### Changed

- Structured more Databricks SQL, including `<=>`, raw/hex prefixed strings, `DISTRIBUTE BY`,
  `SORT BY`, `CLUSTER BY`, Delta `RESTORE`, `ANALYZE TABLE`, `MSCK REPAIR TABLE`, and
  `CREATE TABLE [SHALLOW|DEEP] CLONE`.
- Structured Snowflake `COPY INTO` stage locations as `STAGE_REF` nodes inside `COPY_LOCATION`
  without swallowing following `FROM` clauses after trailing stage-path slashes.

### Fixed

- Made embedded SQL routine bodies format with the active dialect instead of falling back to the
  default Snowflake lexer/parser.
- Kept Databricks from treating Snowflake-style `//` as a line comment.

## [1.3.0] - 2026-07-09

### Added

- Added a release version updater (`scripts/update-version.py`) that keeps workspace, lockfile,
  extension, Homebrew, README, and docs-site version references in sync.
- Enabled Databricks SQL scripting compound blocks, including `BEGIN [NOT] ATOMIC`, in the parser
  and formatter.

### Changed

- Structured Snowflake time-travel and sampling clauses so contextual words inside those clauses
  are keyword-cased consistently.
- Hid the unused Doc composition layer from public docs while keeping it available for internal
  use and compatibility.

### Fixed

- Upper-cased recognized keywords in lenient statements such as `ALTER`, `SHOW`, and scripting
  declaration runs.
- Prevented comments immediately before synthesized closing delimiters from forcing whole-statement
  verbatim fallback.
- Wrapped long `WHERE` / `JOIN ON` logical chains and long `OVER (...)` window specs using the
  formatter's line-width-aware Doc layout.
- Reused one parser pass for CLI diagnostics and formatting on text inputs.

## [1.2.3] - 2026-07-08

### Fixed

- Limited the GHCR release image to `linux/amd64` so Docker publishing completes without the slow
  QEMU arm64 source build.

## [1.2.2] - 2026-07-08

### Fixed

- Removed the one-time GitHub Pages enablement flag from the docs workflow after provisioning the
  Pages site for GitHub Actions deployment.

## [1.2.1] - 2026-07-08

### Fixed

- Enabled GitHub Pages from the docs workflow on non-PR runs so the mdBook site can be
  provisioned and deployed by CI.

## [1.2.0] - 2026-07-08

### Added

- Added cargo-fuzz targets for lexer round-trip, parser losslessness, and formatter idempotency,
  with a scheduled/manual workflow that uploads crash artifacts.
- Added an mdBook documentation site with a browser WASM playground and GitHub Pages deployment
  workflow.
- Added a Homebrew formula so this repository can be used directly as a tap.

## [1.1.0] - 2026-07-08

### Added

- Added CLI `--diff`, `--stdin-filepath`, and directory exclude handling for safer CI usage.
- Added a Chrome extension options page backed by `chrome.storage.sync` for dialect, line width,
  indent width, and keyword casing.
- Added Databricks browser host coverage for the Chrome extension.
- Added an explicit WASM dialect API and wired the Chrome extension to pass Snowflake/Databricks
  mode through to the formatter.
- Added pre-commit hooks, a composite GitHub Action, cargo-binstall metadata, and a Docker/GHCR
  release path for CI-oriented distribution.
- Added CI gates for rustdoc warnings and formatter benchmark smoke runs.
- Added MSRV, wasm, dependency-audit, and coverage workflow coverage.

### Changed

- Improved CLI file processing by reusing decoded input for diagnostics and formatting, and by
  caching config resolution per directory during parallel runs.
- Kept the Chrome extension's Monaco editor tracking alive for delayed editor loads.
- Centralized internal crate dependency versions in `[workspace.dependencies]`.
- Made tag-push extension packaging/publishing part of the Release workflow, leaving the extension
  workflow for manual package/publish runs.
- Split corpus CI behavior so PR/push uses the committed sample and scheduled/manual runs may use
  the configured external corpus URL.
- Switched the project license metadata to `0BSD`.

### Fixed

- Fixed several formatter/parser edge cases from post-1.0 work, including malformed delimiters,
  adjacent operator boundaries, directive-comment source reuse, and formatter width measurement.

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
- **CLI** (`sql-dialect-fmt`): the `sql-dialect-fmt` binary with `--write`, `--check`,
  stdin/stdout, `--dialect`, and `--line-width` / `--indent-width` / `--no-uppercase`
  options.
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

[Unreleased]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.4.0...HEAD
[1.4.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.2.3...v1.3.0
[1.2.3]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.2.2...v1.2.3
[1.2.2]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.2.1...v1.2.2
[1.2.1]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v0.1.0...v1.0.0
