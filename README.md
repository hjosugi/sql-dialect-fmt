<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt

[![CI](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/ci.yml/badge.svg)](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/ci.yml)

English | [日本語](README.ja.md)

`sql-dialect-fmt` is an opinionated Rust formatter and editor toolchain centered on Snowflake SQL
and Databricks SQL. It follows the `gofmt` / Prettier / Biome style: stable output, and
formatting that is safe to run in CI. Lexing and syntax highlighting follow the
[`sql-formatter`](https://github.com/sql-formatter-org/sql-formatter) vocabulary across 21
dialects — including BigQuery, ClickHouse, DB2, DB2 for i, DuckDB, Hive, MariaDB, MySQL, TiDB,
N1QL, Oracle PL/SQL, PostgreSQL, Redshift, SingleStoreDB, Spark, SQLite, Transact-SQL, Trino, and
standard SQL — with per-dialect lexical rules (deep formatting quality leads with
Snowflake/Databricks).

Formatting is mechanically **lossless and idempotent**. Inputs that cannot be parsed pass through
unchanged, significant tokens and comments are preserved, and `format(format(x)) == format(x)`.

## Install

```sh
# From crates.io
cargo install sql-dialect-fmt --version 1.26.0 --locked

# Directly from this repository
cargo install --git https://github.com/hjosugi/sql-dialect-fmt sql-dialect-fmt

# From a local checkout
cargo install --path crates/sql-dialect-fmt-cli

# Binary install, when using release assets with cargo-binstall
cargo binstall sql-dialect-fmt

# Homebrew, using this repository as a tap
brew tap hjosugi/sql-dialect-fmt https://github.com/hjosugi/sql-dialect-fmt
brew install sql-dialect-fmt
```

CI can use the bundled composite action or the GHCR image.

```yaml
- uses: hjosugi/sql-dialect-fmt@v1
  with:
    args: "sql/**/*.sql"
```

```sh
docker run --rm -v "$PWD:/work" -w /work ghcr.io/hjosugi/sql-dialect-fmt:1.26.0 --check .
```

Try the browser playground from the docs site:
<https://hjosugi.github.io/sql-dialect-fmt/playground.html>

## Usage

```sh
sql-dialect-fmt query.sql                 # format to stdout
sql-dialect-fmt --write *.sql             # rewrite files in place
sql-dialect-fmt --check src/**/*.sql      # non-zero when files are not formatted
sql-dialect-fmt --check --diff query.sql  # show a unified diff for unformatted input
sql-dialect-fmt --lint src/               # lint (SDF001-SDF007): path:line:col findings, exit 1 when any
cat query.sql | sql-dialect-fmt           # stdin to stdout
cat query.sql | sql-dialect-fmt -         # explicitly read stdin with `-`
sql-dialect-fmt --stdin-filepath src/query.sql < query.sql  # use a path for config discovery
cat query.sql | sql-dialect-fmt --range 40:120  # reformat only statements in a byte range (stdin)

# Style: --keyword-case upper|lower|preserve / --select-item-layout auto|vertical
#        --comma-style trailing|leading / --line-width N / --indent-width N
#        --line-ending auto|lf|crlf
```

The shared defaults are `line_width = 80`, `indent_width = 2`, upper-case keywords, vertical
top-level `SELECT` items, trailing commas, and automatic preservation of the input line ending.
CLI, config, LSP, Wasm playground, and VS Code use the same option model and defaults.

pre-commit users can enable the official hooks:

```yaml
repos:
  - repo: https://github.com/hjosugi/sql-dialect-fmt
    rev: v1.26.0
    hooks:
      - id: sql-dialect-fmt
```

Use `sql-dialect-fmt-check` instead when a hook should only verify formatting.

## VS Code Extension

The VS Code extension in `editors` adds Snowflake SQL syntax highlighting **and formatting**. It
registers a formatter for `snowflake-sql` files, so **Format Document**, **Format Selection**, and
`editor.formatOnSave` all work with no external binary. It bundles the Rust formatter as
WebAssembly and formats entirely on your machine.

```sh
./scripts/build-vscode-extension.sh
```

Then press <kbd>F5</kbd> in `editors/` (or install the packaged VSIX) and run **Format Document** on
a `.sql` file. Formatting honors keyword case, vertical/automatic SELECT layout, leading/trailing
commas, line width, line endings, and the active editor indentation.

## Development

```sh
task test
task clippy
task vscode:build
task vscode:package
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
task fmt:check
```

### Formatter feature flags

The default build keeps the core SQL formatter small and fast. Embedded JavaScript/Python body
formatting remains available as an explicit feature while basic formatter and VS Code quality are
the active priority:

```sh
cargo test -p sql-dialect-fmt-formatter --features external-formatters
```

Fine-grained features are available for embedders:

| feature | default | effect |
| --- | --- | --- |
| `external-formatters` | no | enables both `embedded-javascript` and `embedded-python` |
| `embedded-javascript` | no | formats `LANGUAGE JAVASCRIPT AS $$...$$` with Biome |
| `embedded-python` | no | formats `LANGUAGE PYTHON AS $$...$$` with Ruff |
| `embedded-brace-formatters` | no | opts into the simple Java/Scala brace-aware formatter |

## Status

Snowflake support covers SELECT, DML (`INSERT`/`UPDATE`/`DELETE`/`MERGE`), `COPY`, major DDL and
object DDL (including Snowpipe `CREATE PIPE ... AS COPY INTO`), Semantic View, and
`CREATE PROCEDURE`/`CREATE FUNCTION` bodies in SQL, JavaScript,
Python, Java, and Scala. Non-SQL embedded body formatting is opt-in and otherwise stays verbatim.
Databricks mode covers LATERAL VIEW, Delta DDL options,
`VERSION`/`TIMESTAMP AS OF`, higher-order-function lambdas, SQL scripting blocks, and backtick
identifiers.

Beyond those two, a 21-dialect registry drives per-dialect lexical rules and highlighting:
identifier quoting (`"…"`, `` `…` ``, `[…]`), line comments (`--`, `//`, `#`), nested block
comments, string prefixes (`E''`, `N''`, `X''`, `B''`, `R''`), `@var`/`@@var`, `:bind`, `#temp`
identifiers, `$$…$$`, and `<=>`. Dialect names resolve the same way in `sql-dialect-fmt.toml`, the
CLI (`--dialect`), and the Wasm ABI, with aliases such as `oracle`, `tsql`, `postgres`, and
`presto`. Configuration covers `keyword_case`, `data_type_case`, `function_case`, `identifier_case`,
`indent_width`/`tab_width`, `use_tabs`, `line_ending`, `select_item_layout`, `comma_style`,
`logical_operator_newline`, `dense_operators`, `newline_before_semicolon`, `lines_between_queries`,
`expression_width`, `params`, `[param_types]`, and `dialect`. A JSON Schema for the file lives
at [`docs/schema.json`](docs/schema.json) (editors that understand TOML schema, such as Taplo
or the VS Code TOML extension via `#:schema`, use it for completion).

SQL lifted out of a host language keeps formatting and highlighting: a `${ ... }` template
placeholder — a JavaScript template literal (`` `SELECT ${cfg.col} FROM ${cfg.t}` ``) or a
Databricks/Spark/dbt `${var}` substitution — is treated as one atomic token, with nested braces,
quoted `}`, and nested template literals balanced so the statement still parses and the placeholder
round-trips verbatim.

The workspace also includes an LSP server, semantic tokens, hover text, a CLI, and VS Code
packaging. Tree-sitter sources remain in the repository but are paused outside the active workspace
and CI. The LSP server discovers and applies the same
`sql-dialect-fmt.toml` as the CLI (with editor settings layered on top), so an editor formats
consistently with CI. The headline formatter feature is **magic trailing comma**. See
[ROADMAP.md](ROADMAP.md) for the detailed coverage map.

## Crates

| crate | role |
| --- | --- |
| `sql-dialect-fmt-syntax` | `SyntaxKind`, keyword recognition, and `rowan` language definition |
| `sql-dialect-fmt-lexer` | hand-written lossless lexer |
| `sql-dialect-fmt-parser` | resilient lossless CST parser |
| `sql-dialect-fmt-formatter` | generic Doc IR engine plus SQL formatting rules |
| `sql-dialect-fmt-highlight` | syntax highlight token classification |
| `sql-dialect-fmt-hover` | hover text for types, routines, and tasks |
| `sql-dialect-fmt-tree-sitter` | paused Rust bindings for the bundled Tree-sitter grammar (outside the active workspace) |
| `sql-dialect-fmt-config` | shared `sql-dialect-fmt.toml` model and discovery |
| `sql-dialect-fmt-lsp` | Language Server over stdio |
| `sql-dialect-fmt-wasm` | raw WebAssembly bridge bundled by the VS Code extension |
| `sql-dialect-fmt` | CLI binary crate (`crates/sql-dialect-fmt-cli`) |

## License

0BSD. You can use, copy, modify, and distribute this project for almost any purpose.
