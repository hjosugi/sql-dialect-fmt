<!-- i18n: language-switcher -->
[English](https://github.com/hjosugi/sql-dialect-fmt/blob/main/editors/README.md) |
[日本語](https://github.com/hjosugi/sql-dialect-fmt/blob/main/editors/README.ja.md)

# Snowflake SQL

Focused Snowflake SQL syntax highlighting **and formatting** for Visual Studio Code, backed by the
same engine used by [sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt).

![Snowflake SQL syntax highlighting](https://raw.githubusercontent.com/hjosugi/sql-dialect-fmt/main/editors/images/syntax-highlighting.png)

## Features

- Document and selection formatting (**Format Document** / **Format Selection**), with format-on-save
- optional, opt-in language server integration — diagnostics, hover, completion, semantic
  highlighting, outline, folding (see [Language server](#language-server-optional))
- Snowflake SQL keywords and built-in types
- Snowflake Scripting and `$$ ... $$` routine bodies
- line (`--`, `//`) and block (`/* ... */`) comments
- strings, quoted identifiers, numeric literals, and operators
- positional `$1`, session `$name`, bind `:name`, and `?` variables
- `@stage`, `@~`, `@%table`, and namespaced stage references
- `.sql`, `.snowsql`, and `.sfsql` file associations

- **Language ID:** `snowflake-sql`
- **Scope name:** `source.snowflake-sql`
- **File types:** `.sql`, `.snowsql`, `.sfsql`

## Formatting

The extension registers a formatter for `snowflake-sql` documents, so **Format Document**,
**Format Selection**, and `"editor.formatOnSave"` all work out of the box. Formatting runs entirely
on your machine: the bundled WebAssembly build of the formatter is the same engine that powers the
CLI and the Snowsight browser extension. Nothing is sent over the network.

Formatting is mechanically **lossless and idempotent** — input that cannot be parsed is passed
through unchanged, and `format(format(x)) == format(x)`.

To make it the default formatter for these files, add to your settings:

```json
"[snowflake-sql]": {
  "editor.defaultFormatter": "sql-dialect-fmt.snowflake-sql-sql-dialect-fmt",
  "editor.formatOnSave": true
}
```

### Settings

| Setting | Default | Description |
| --- | --- | --- |
| `sqlDialectFmt.dialect` | `snowflake` | SQL dialect (`snowflake` or `databricks`). |
| `sqlDialectFmt.lineWidth` | `100` | Target line width before wrapping. |
| `sqlDialectFmt.indentWidth` | `4` | Spaces per indent level. |
| `sqlDialectFmt.uppercaseKeywords` | `true` | Upper-case SQL keywords. |
| `sqlDialectFmt.lsp.enabled` | `false` | Opt in to the `sql-dialect-fmt-lsp` language server (see below). |
| `sqlDialectFmt.lsp.path` | `""` | Path to `sql-dialect-fmt-lsp`; empty looks it up on `PATH`. |

The keyword and type word lists are kept in lock-step with the formatter's own
lexer/highlighter by tests in `sql-dialect-fmt-highlight` (`tests/textmate.rs`): every word the
grammar scopes as a keyword or type must be classified the same way by
`sql_dialect_fmt_highlight::classify`, so the grammar can't drift from the rest of the toolchain.

## Language server (optional)

Everything above works out of the box with no external binary. For the features beyond
formatting — lint diagnostics, hover documentation, completion, semantic highlighting, document
symbols (outline), folding ranges, and on-type formatting — the extension can also drive the
[`sql-dialect-fmt-lsp`](https://crates.io/crates/sql-dialect-fmt-lsp) language server. This is
opt-in and off by default:

1. Install the server: `cargo install sql-dialect-fmt-lsp`.
2. Set `"sqlDialectFmt.lsp.enabled": true`. If the binary is not on `PATH`, point
   `sqlDialectFmt.lsp.path` at it.

While the server is running it also serves **Format Document** / **Format Selection** / format on
save (layering the nearest `sql-dialect-fmt.toml` under your editor settings), and the built-in
WebAssembly formatter is unregistered so the two never compete. If the server is enabled but the
binary is missing or fails to start, the extension logs the reason to the **sql-dialect-fmt**
output channel and quietly keeps the bundled WebAssembly formatter — installing the binary is
never required for the extension to work.

The `sqlDialectFmt.*` settings are forwarded to the server, which additionally honors the
`sqlDialectFmt.lint.*` toggles for individual diagnostics. Like the bundled formatter, the server
is a local process speaking LSP over stdio; it never touches the network.

## Use

1. Install the extension.
2. Open a `.sql`, `.snowsql`, or `.sfsql` file.
3. If needed, choose **Change Language Mode** and select **Snowflake SQL**.
4. Run **Format Document** (`Shift+Alt+F`) or **Format Selection**.

This extension contributes syntax highlighting, language metadata, and a local formatter. It does
not execute SQL against Snowflake or connect to any account. For CLI formatting and other
integrations, see the [main project README](https://github.com/hjosugi/sql-dialect-fmt#readme).

## Privacy

The extension runs no telemetry or analytics, makes no network requests, and performs no remote
formatting. Formatting is done locally by a bundled WebAssembly module; your SQL never leaves the
machine. The extension only contributes static language configuration, a TextMate grammar, the
local formatter, and — only if you opt in — a client for the local `sql-dialect-fmt-lsp` process,
which also runs entirely on your machine. See the
[privacy policy](https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md).

## Other editors

This directory also carries integrations for other editors, all driven by the same
`sql-dialect-fmt-lsp` language server (`cargo install sql-dialect-fmt-lsp`):

- [`nvim/`](https://github.com/hjosugi/sql-dialect-fmt/tree/main/editors/nvim) — a small Neovim plugin: `snowflake-sql` filetype, LSP setup, and
  conform.nvim/null-ls recipes for CLI-based formatting.
- [`zed/`](https://github.com/hjosugi/sql-dialect-fmt/tree/main/editors/zed) — a Zed extension (dev install): Snowflake SQL language backed by the
  bundled tree-sitter grammar plus the language server.
- [`helix/`](https://github.com/hjosugi/sql-dialect-fmt/tree/main/editors/helix) — a documented `languages.toml` snippet for Helix (no plugin system).

## Support and source

- [Report an issue](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [Source code](https://github.com/hjosugi/sql-dialect-fmt)
- License: [0BSD](LICENSE.md)
