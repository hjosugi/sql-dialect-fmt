# Changelog

## Unreleased

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
