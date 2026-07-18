<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt for Helix

Helix has no plugin system, so this directory is documentation rather than a package:
[`languages.toml`](languages.toml) is a ready-to-copy snippet that wires the
`sql-dialect-fmt-lsp` language server (formatting, diagnostics, hover, semantic tokens,
completion) and the bundled [`tree-sitter-snowflake`](../../tree-sitter-snowflake) grammar
into Helix.

## Setup

1. Install the language server so it is on your `PATH`:

   ```sh
   cargo install sql-dialect-fmt-lsp --locked
   ```

2. Copy the parts you want from [`languages.toml`](languages.toml) into
   `~/.config/helix/languages.toml` (or a project-local `.helix/languages.toml`).

3. If you added the `[[grammar]]` block, build the grammar and install the queries:

   ```sh
   hx --grammar fetch
   hx --grammar build

   # Highlight queries live per-language under the Helix runtime directory:
   mkdir -p ~/.config/helix/runtime/queries/snowflake
   cp tree-sitter-snowflake/queries/{highlights,injections,locals}.scm \
     ~/.config/helix/runtime/queries/snowflake/
   ```

   The queries use common capture names on purpose; Helix maps most of them directly
   (`@number` is the main gap — Helix themes use `@constant.numeric`).

4. Check the result with `hx --health snowflake-sql`.

The snippet defines a dedicated `snowflake-sql` language for `*.snowsql` / `*.sfsql` and
deliberately leaves plain `*.sql` to Helix's built-in `sql` language.

## Using it for plain `*.sql` too

Either add `"sql"` to the `file-types` list of the `snowflake-sql` language, or keep the
built-in SQL language and just attach the server to it:

```toml
[[language]]
name = "sql"
language-servers = ["sql-dialect-fmt-lsp"]
auto-format = true
```

## Formatting via the CLI instead

If you only want formatting (no diagnostics/hover/completion), skip the language server
and use the CLI as the language formatter — it reads stdin and writes stdout:

```toml
[[language]]
name = "sql"
formatter = { command = "sql-dialect-fmt" }
auto-format = true
```

Install it with `cargo install sql-dialect-fmt --locked`, Homebrew
(`brew tap hjosugi/sql-dialect-fmt https://github.com/hjosugi/sql-dialect-fmt && brew install sql-dialect-fmt`),
or a [release tarball](https://github.com/hjosugi/sql-dialect-fmt/releases).

## Settings

Editor-side settings go under `[language-server.sql-dialect-fmt-lsp.config.sqlDialectFmt]`
(`lineWidth`, `indentWidth`, `dialect`, `uppercaseKeywords`, `keywordCase`, `lineEnding`,
`lint.*`). The server layers options as **defaults → nearest `sql-dialect-fmt.toml` →
editor settings**, so a project config file keeps Helix and CI consistent.

## Support and source

- [Report an issue](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [Source code](https://github.com/hjosugi/sql-dialect-fmt)
- License: [0BSD](../../LICENSE)
