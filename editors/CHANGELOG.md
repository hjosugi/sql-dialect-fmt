# Changelog

## 1.22.1

- Preserved non-space control characters inside quoted SQL tokens in the bundled Wasm formatter.
- Added an output safety postcondition so comment and punctuation layout can never return a
  different meaningful token sequence; unsafe candidates remain byte-for-byte unchanged.
- Made malformed delimiter and prefix chains recover without parser panics in debug or release
  builds.

## 1.22.0

- Standardized bundled Wasm and optional LSP formatting on the same 80-column, two-space,
  upper-case, vertical-SELECT, trailing-comma, input-line-ending-preserving defaults used by the
  CLI and Rust API.
- Added lifecycle coverage for enabling and disabling the language server and for startup failure;
  the extension disposes the previous provider/client and keeps bundled Wasm formatting available.
- Kept the deprecated `uppercaseKeywords` setting compatible while making `keywordCase`,
  `selectItemLayout`, `commaStyle`, and `lineEnding` the complete public style model.

## 1.18.0

- Reworked syntax highlighting around the official Snowflake reference material: control-flow
  keywords, constants, Snowflake Scripting status variables and exceptions, and all 900 documented
  built-in functions now carry their own TextMate scopes instead of one flat keyword colour, and
  DDL statements highlight the created/altered/dropped object name.
- Highlighted `LANGUAGE SQL`, `EXECUTE IMMEDIATE`, and `AS $$` bodies as Snowflake SQL instead of
  an opaque string, and embedded `LANGUAGE PYTHON`/`JAVA`/`SCALA` bodies as their host language.
- Completed the data-type list from the official summary (`SMALLINT`, `TINYINT`, `BYTEINT`,
  `FLOAT4`/`FLOAT8`, `DOUBLE PRECISION`, `NCHAR`/`NVARCHAR`/`NVARCHAR2`, `VARBINARY`, `DECFLOAT`,
  `FILE`, `UUID`, `CURSOR`, `RESULTSET`) and added missing DDL/clause keywords.
- Fixed JavaScript stored-procedure highlighting by injecting VS Code's JavaScript grammar inside
  `LANGUAGE JAVASCRIPT ... $$ ... $$` bodies.
- Fixed formatting of JavaScript procedures containing whitespace-only lines; these previously
  caused the bundled formatter to return the complete document unchanged.
- Bundled the extension host and language client into one file, reducing the VSIX from hundreds of
  JavaScript/dependency files to one JavaScript bundle.

## 1.16.0

- Added an opt-in LSP client (`sqlDialectFmt.lsp.enabled`, default off): when the
  `sql-dialect-fmt-lsp` binary is installed, `snowflake-sql` documents gain lint diagnostics,
  hover, completion, semantic highlighting, document symbols, folding, and on-type formatting,
  and the server takes over formatting. Without the binary the extension keeps using the bundled
  WebAssembly formatter, unchanged.
- Added the `sqlDialectFmt.lsp.path` setting for pointing at a `sql-dialect-fmt-lsp` binary that
  is not on `PATH`.
- Updated the Marketplace summary to identify the extension as a formatter and fixed README image
  and integration links so they render from the monorepo layout.

## 1.14.0

- Added a local document and selection formatter for `snowflake-sql` files, powered by the bundled
  `sql-dialect-fmt` WebAssembly engine. **Format Document**, **Format Selection**, and
  `editor.formatOnSave` now work with no external binary or network access.
- Added `sqlDialectFmt.dialect`, `sqlDialectFmt.lineWidth`, `sqlDialectFmt.indentWidth`, and
  `sqlDialectFmt.uppercaseKeywords` settings.

## 1.13.0

- Reworked the Marketplace page around user-facing features and installation guidance.
- Added a high-resolution extension icon and an accurate Snowflake SQL highlighting screenshot.
- Expanded Marketplace search keywords and documented the extension's no-telemetry privacy model.

## 1.12.1

- Synchronized the VSIX package version with the sql-dialect-fmt workspace hotfix release.

## 1.9.0

- Added `.sfsql` file association alongside `.sql` and `.snowsql`.
- Kept the TextMate keyword and type tables synchronized with the Rust highlighter through CI.

## 1.0.0

- Initial Marketplace-ready package for Snowflake SQL TextMate grammar support.
