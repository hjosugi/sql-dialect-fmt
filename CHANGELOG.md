# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The published crates share a single workspace version (see `RELEASING.md`).

## [Unreleased]

### Fixed

- Hardened the generic token walker against eight additional compound-operator boundary fusions,
  preserving the original token sequence when adjacent `=`, `<`, `>`, `->`, `|`, or `!` tokens
  could otherwise be re-lexed as a different multi-character operator.

## [1.17.0] - 2026-07-21

### Added

- Recognized `${ ... }` template-substitution placeholders as first-class tokens, so SQL embedded
  in a host language now formats and highlights cleanly: a JavaScript template literal
  (`` `SELECT ${cfg.col} FROM ${cfg.t}` ``) or a Databricks / Spark / dbt `${var}` substitution.
  The lexer keeps each placeholder as one atomic token, balancing nested braces and skipping quoted
  `}` and nested template literals so `SELECT ${ fn({a: 1}, '}') }` stays intact; the parser accepts
  a placeholder wherever a name or value can appear; the formatter lays the statement out normally
  while preserving the placeholder verbatim; and the lexical highlighter, TextMate grammar, and
  Tree-sitter grammar colour it as a parameter (with nine new lexer/parser/formatter/highlight cases
  and four Tree-sitter corpus cases).

## [1.16.2] - 2026-07-19

### Changed

- Centralized Chrome extension, docs playground, and store-demo typography and repeated visual
  values in namespaced CSS custom properties; the store-asset validator now prevents consumers
  from bypassing the shared font tokens.
- Updated the bundled VS Code language client to 10.1.0, aligned the extension's minimum VS Code
  version to 1.91.0, and refreshed the Rust lockfile for serde 1.0.229 and cc 1.3.0.

## [1.16.1] - 2026-07-18

### Added

- Added end-to-end LSP stdio coverage for initialization, diagnostics, document lifecycle,
  formatting, hover, symbols, semantic tokens, configuration changes, and clean shutdown; expanded
  the raw WebAssembly ABI tests and added 24 Tree-sitter corpus cases across six statement
  families (#81).
- Added a strict realistic JavaScript-procedure regression fixture and exercised its exact output
  through the formatter API, CLI, LSP stdio server, raw WebAssembly ABI, bundled VS Code provider,
  and the VS Code TextMate tokenization engine.
- Added a CI gate that builds the VS Code WebAssembly artifact and JavaScript bundle, runs the
  editor integration tests, packages a VSIX, and inspects the archive contents.

### Changed

- Bundled the VS Code extension and `vscode-languageclient` into one `dist/extension.js`; VSIX
  packages now contain 12 files instead of hundreds of transitive `node_modules` files, and a
  package validator prevents dependency trees from returning.
- Reused routine bodies prepared by the trailing-whitespace safety pass during lowering, avoiding
  a second embedded-formatter run; the realistic JavaScript-procedure benchmark improved by about
  31% on the development machine.
- Measured multiline embedded bodies by their final line instead of their full byte length, so a
  short routine signature stays inline when it fits the configured line width.

### Fixed

- Preserved the token boundary between unary `-` and `>=` in lenient statements; a minimized
  property-test regression had previously fused them into `->=`, breaking idempotency.
- Allowed supported embedded-language formatters to normalize whitespace-only lines inside
  multiline routine bodies instead of silently returning the entire SQL document unchanged.
- Injected VS Code's JavaScript grammar inside `LANGUAGE JAVASCRIPT ... $$ ... $$` routine bodies,
  including multiline headers, instead of coloring the whole body as a dollar-quoted SQL string.

## [1.16.0] - 2026-07-18

### Added

- Structured parsing and formatting for the common `ALTER TABLE / VIEW / SESSION / WAREHOUSE /
  TASK` (plus SCHEMA / DATABASE / MATERIALIZED VIEW / DYNAMIC TABLE) statements: the object head
  and each action clause (`ADD/DROP/RENAME COLUMN`, `RENAME TO`, `SET`/`UNSET`,
  `SUSPEND`/`RESUME`, `SWAP WITH`, …) are now CST nodes, so multiple actions stack one per line
  and `ALTER SESSION SET` property lists wrap with proper keyword casing; unmodeled ALTER kinds
  keep the lossless lenient token run.
- Added an opt-in LSP client to the VS Code extension (#82): with `sqlDialectFmt.lsp.enabled` set
  and the `sql-dialect-fmt-lsp` binary installed (`sqlDialectFmt.lsp.path` or `PATH`),
  `snowflake-sql` documents get lint diagnostics, hover, completion, semantic tokens, document
  symbols, folding, and on-type formatting, and the server takes over formatting from the bundled
  WebAssembly engine; when the binary is missing the extension logs and falls back to wasm-only,
  so the Marketplace package keeps working offline with no external dependency.

### Changed

- Structured the bundled Tree-sitter grammar with coarse statement-kind nodes
  (`select_statement`, `insert_statement`, `create_statement`, ..., with a lenient `statement`
  fallback), improving editor folding, outlines, and text objects while keeping the tolerant
  token-run parsing unchanged.

## [1.15.0] - 2026-07-18

### Added

- Added spec-driven rich hover (#92): keywords, clauses, and other constructs tracked in
  `spec/seed/features.json` now hover with their syntax, GA/Preview status, parser coverage, and a
  Snowflake docs link — including multi-word phrases such as `GROUP BY` or `LEFT OUTER JOIN`, and
  `CREATE OR REPLACE ...` / `IS NOT NULL` forms.
- Added `spec/seed/functions.json`, a curated function signature table (109 common Snowflake
  functions), and hover for function calls: signature, return type, summary, and docs link.
  Qualified names such as `SNOWFLAKE.CORTEX.SENTIMENT` and parenthesis-free context functions such
  as `CURRENT_TIMESTAMP` are recognized.
- Added `scripts/generate-hover-tables.py`, which generates the hover tables from the spec seeds
  into `crates/sql-dialect-fmt-hover/src/generated.rs`; CI fails when the file is out of sync.
- Added `textDocument/onTypeFormatting` to the language server: typing `;` or a newline reformats
  the statement that just ended, using the same statement-level range formatting engine, and leaves
  already formatted statements untouched.
- Added four lint rules, each individually toggleable and suppressible with
  `-- sql-dialect-fmt: disable-next-line SDFxxx`: `DELETE` without `WHERE` (SDF004), `UPDATE`
  without `WHERE` (SDF005), comma join in `FROM` — implicit cross join, with the Snowflake
  `, LATERAL ...` / `, TABLE(...)` idioms exempt (SDF006), and `ORDER BY` ordinal (SDF007).
- Added a CLI `--lint` flag that lints inputs instead of formatting them, printing findings as
  `path:line:col: SDFxxx message` (1-based) and exiting `1` when any exist; it honors `--dialect`
  and `sql-dialect-fmt.toml` dialect discovery.
- Added Neovim (`editors/nvim`, filetype + LSP plugin), Zed (`editors/zed`, dev-installable
  extension), and Helix (`editors/helix`, documented `languages.toml` snippet) packaging for the
  `sql-dialect-fmt-lsp` language server and the bundled tree-sitter grammar.

### Changed

- Split the formatter's SQL lowering module into focused query/DML/DDL/scripting/expression
  submodules so the statement-family rules no longer live in one large file.
- Split the parser grammar module into focused per-family submodules (queries, expressions, DDL,
  access control, COPY INTO, scripting, MATCH_RECOGNIZE) so the grammar no longer lives in one
  large file.
- Extracted the lint engine into a published, LSP-independent `sql-dialect-fmt-lint` crate
  (byte-ranged diagnostics; publishes after `parser`, before the CLI and LSP crates). The LSP
  crate keeps its public lint API (`LintOptions`, `LintCode`, `diagnostic_lint_code`, …) as a
  thin adapter over the new crate.

## [1.14.0] - 2026-07-16

### Added

- Added a local document and selection formatter to the VS Code extension, powered by the bundled
  `sql-dialect-fmt` WebAssembly engine, so **Format Document**, **Format Selection**, and
  `editor.formatOnSave` work with no external binary and no network access.
- Added `sqlDialectFmt.dialect`, `sqlDialectFmt.lineWidth`, `sqlDialectFmt.indentWidth`, and
  `sqlDialectFmt.uppercaseKeywords` settings to the VS Code extension.
- Added `sql-dialect-fmt.toml` discovery to the language server, which now layers configuration as
  defaults → `sql-dialect-fmt.toml` → editor settings, so an editor formats consistently with CI.
- Added `sql-dialect-fmt-config`, a published crate holding the shared `sql-dialect-fmt.toml`
  model and discovery used by both the CLI and the language server.
- Added statement-level range formatting: `sql_dialect_fmt_formatter::format_range` reformats only
  the statements intersecting a byte range and leaves the rest of the document byte-identical,
  preserving leading blank lines and same-line trailing comments.
- Added `textDocument/rangeFormatting` to the language server, backing **Format Selection** in
  LSP-driven editors.
- Added a CLI `--range START:END` flag that reformats only the statements intersecting a byte
  range when reading from stdin.

### Changed

- Added `sql-dialect-fmt-config` to the ordered crates.io publish list and the version updater, so
  releases publish it before the dependent CLI and LSP crates.

## [1.13.0] - 2026-07-11

### Added

- Added complete Chrome Web Store artwork: 128×128 icon, two 1280×800 product screenshots,
  440×280 promo tile, optional 1400×560 marquee, YouTube-ready demo video and thumbnail.
- Added a copy/paste Chrome review submission sheet, asset provenance, and CI validation for image
  dimensions, alpha channels, manifest references, privacy copy, and video encoding.
- Added a VS Code Marketplace icon and accurate Snowflake SQL highlighting screenshot.

### Changed

- Reworked the VS Code Marketplace README and changelog around user-facing features, setup,
  support, and the extension's static/no-telemetry behavior.
- Documented Databricks access and `chrome.storage.sync` formatter preferences in the privacy
  policy and Chrome review permission justifications.
- Updated initial store upload and workflow examples to v1.13.0 and packaged Chrome runtime icons.

## [1.12.1] - 2026-07-11

### Fixed

- Fixed crates.io index polling so it queries the exact published crate/version instead of an
  invalid temporary Cargo package.
- Excluded unpublished internal test helpers from published crate metadata, allowing dependent
  workspace crates to pass `cargo publish` verification.

## [1.12.0] - 2026-07-11

### Added

- Added redacting publication credential preflight checks and a helper that configures the
  crates.io token and tag-triggered GitHub Actions publication.
- Added isolated CI coverage and char-literal regressions for the opt-in Java/Scala brace-aware
  embedded formatter.

### Changed

- Made ordered crates.io publishing resumable by skipping crate versions that are already
  published, so a partially completed release can be retried safely.
- Documented first publication and subsequent Marketplace, Web Store, and crates.io update flows.

## [1.11.0] - 2026-07-10

### Added

- Structured unambiguous `CREATE ... AS <query>` bodies for object kinds without a specialized
  grammar, while leaving non-query `AS (...)` surfaces lossless and verbatim.
- Added dedicated CST nodes for routine `RETURNS <type>` and `LANGUAGE <language>` clauses.

### Changed

- Added CI coverage for minimal, JavaScript-only, and Python-only formatter feature sets, including
  dependency-tree checks that keep disabled Biome/Ruff formatter graphs out of each build.

### Fixed

- Increased bounded parser lookahead headroom so broad statement dispatch cannot trip the progress
  guard on malformed token sequences; the minimized regression remains covered by proptest.

## [1.10.0] - 2026-07-10

### Added

- Structured Snowflake `PUT`, `GET`, `LIST`, and `REMOVE` stage file operations, including
  contextual command/option casing and lossless local `file://` locations.
- Modeled `FINAL` and `RUNNING` window semantics on `MATCH_RECOGNIZE` measure items.

### Fixed

- Kept `//` inside unquoted Snowflake `file://` locations from being lexed as a line comment.

## [1.9.0] - 2026-07-10

### Added

- Added formatter Cargo features for embedded language formatting:
  `embedded-javascript`, `embedded-python`, the default aggregate `external-formatters`, and the
  opt-in `embedded-brace-formatters`.

### Changed

- Made the Biome/Ruff formatter dependency graph optional; `sql-dialect-fmt-formatter` now builds
  without those dependencies under `--no-default-features`.
- Made the simple Java/Scala brace-aware embedded body formatter opt-in. Java/Scala bodies are
  preserved verbatim by default unless `embedded-brace-formatters` is enabled.

## [1.8.0] - 2026-07-10

### Changed

- Preserved statement grouping in the formatter: adjacent statements stay adjacent, while one or
  more source blank lines are retained as a single blank line.
- Formatted `EXECUTE IMMEDIATE $$...$$` bodies as embedded SQL/Snowflake Scripting when they parse
  cleanly, with the existing verbatim fallback for unsupported bodies.

## [1.7.1] - 2026-07-10

### Changed

- Archived the stale `HANDOFF.md` snapshot and replaced it with current restart,
  validation, release, and source-of-truth guidance.
- Updated the roadmap status summary to reflect the current release lane and issue-based follow-up
  tracking.

## [1.7.0] - 2026-07-10

### Changed

- Centralized built-in type and keyword list consumption through the syntax crate for highlighter
  and LSP completions, and added TextMate/tree-sitter synchronization tests.
- Brought the bundled tree-sitter keyword table back in sync with the syntax keyword table.

## [1.6.1] - 2026-07-10

### Changed

- Split the hover crate into focused scan/data modules so token scanning, static hover tables, and
  object-hover logic no longer live in one large file.

## [1.6.0] - 2026-07-10

### Added

- Added LSP `textDocument/codeAction` quick fixes for lint diagnostics, starting with suppression
  comments such as `-- sql-dialect-fmt: disable-next-line SDF001`.

### Changed

- Moved LSP lint rules into a dedicated trait-based `lint` module with shared rule-code helpers,
  keeping diagnostics and quick fixes on the same `SDF001`-`SDF003` contract.

## [1.5.0] - 2026-07-10

### Added

- Added LSP `textDocument/documentSymbol` support for top-level SQL statement outlines.
- Added LSP `textDocument/completion` support for SQL keywords, Snowflake-style data types, and
  common statement snippets.
- Added LSP semantic token range support and advertised full/delta semantic token requests with
  stable result ids.

### Changed

- LSP lint diagnostics now include stable rule codes (`SDF001`-`SDF003`) and configurable lint
  settings for enabling rules and tuning the large `IN (...)` list threshold.

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

[Unreleased]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.17.0...HEAD
[1.17.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.16.2...v1.17.0
[1.16.2]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.16.1...v1.16.2
[1.16.1]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.16.0...v1.16.1
[1.16.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.15.0...v1.16.0
[1.15.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.14.0...v1.15.0
[1.14.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.13.0...v1.14.0
[1.13.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.12.1...v1.13.0
[1.12.1]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.12.0...v1.12.1
[1.12.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.11.0...v1.12.0
[1.11.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.10.0...v1.11.0
[1.10.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.9.0...v1.10.0
[1.9.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.8.0...v1.9.0
[1.8.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.7.1...v1.8.0
[1.7.1]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.7.0...v1.7.1
[1.7.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.6.1...v1.7.0
[1.6.1]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.6.0...v1.6.1
[1.6.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.4.0...v1.5.0
[1.4.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.2.3...v1.3.0
[1.2.3]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.2.2...v1.2.3
[1.2.2]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.2.1...v1.2.2
[1.2.1]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/hjosugi/sql-dialect-fmt/compare/v0.1.0...v1.0.0
