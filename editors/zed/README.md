<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt for Zed

A [Zed](https://zed.dev) extension for [sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt):
it declares the **Snowflake SQL** language (tree-sitter highlighting via the bundled
[`tree-sitter-snowflake`](../../tree-sitter-snowflake) grammar) and launches the
`sql-dialect-fmt-lsp` language server (formatting, range formatting, on-type formatting,
diagnostics, hover, semantic tokens, completion).

By default the language claims `*.snowsql` and `*.sfsql`. Plain `*.sql` is left to other
SQL extensions; opt in from your Zed settings:

```json
{
  "file_types": {
    "Snowflake SQL": ["sql"]
  }
}
```

## Install

1. Install the language server binary so it is on your `PATH`:

   ```sh
   cargo install sql-dialect-fmt-lsp --locked
   ```

2. Install the extension as a **dev extension** (it is not on the Zed extension registry
   yet): open the command palette, run `zed: install dev extension`, and select this
   directory (`editors/zed` in a checkout of the repository). Zed compiles the Rust glue
   to WebAssembly and fetches the grammar, so a Rust toolchain with the `wasm32-wasip2`
   target is required for the build.

Formatting uses the standard Zed commands (`editor: format`, format-on-save). The server
discovers the nearest `sql-dialect-fmt.toml` per file, so projects format the same way as
the CLI in CI.

## Grammar

`extension.toml` points the `snowflake` grammar at this repository:

```toml
[grammars.snowflake]
repository = "https://github.com/hjosugi/sql-dialect-fmt"
rev = "9cd8a8c0da6f937a9d6ce417d188772bdbd5637f"
path = "tree-sitter-snowflake"
```

When developing the grammar locally, point `repository` at a `file://` URL of your
checkout and `rev` at the commit to test. Bump `rev` whenever the grammar changes.

## Settings

Zed passes per-server settings through to the language server. The server accepts its
options either at the top level or under the `sqlDialectFmt` section:

```json
{
  "lsp": {
    "sql-dialect-fmt-lsp": {
      "settings": {
        "sqlDialectFmt": {
          "lineWidth": 100,
          "indentWidth": 4,
          "dialect": "snowflake",
          "uppercaseKeywords": true
        }
      }
    }
  }
}
```

A `binary` override is also supported, e.g.
`"binary": { "path": "/path/to/sql-dialect-fmt-lsp" }`.

Settings are layered as **defaults → nearest `sql-dialect-fmt.toml` → editor settings**.

## Support and source

- [Report an issue](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [Source code](https://github.com/hjosugi/sql-dialect-fmt)
- License: [0BSD](../../LICENSE)
