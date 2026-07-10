# sql-dialect-fmt

[![CI](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/ci.yml/badge.svg)](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/ci.yml)

English | [日本語](README.ja.md)

`sql-dialect-fmt` is an opinionated Rust formatter and editor toolchain for Snowflake SQL and
Databricks SQL. It follows the `gofmt` / Prettier / Biome style: few options, stable output, and
formatting that is safe to run in CI.

Formatting is mechanically **lossless and idempotent**. Inputs that cannot be parsed pass through
unchanged, significant tokens and comments are preserved, and `format(format(x)) == format(x)`.

## Install

```sh
# From crates.io
cargo install sql-dialect-fmt --version 1.8.0 --locked

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
docker run --rm -v "$PWD:/work" -w /work ghcr.io/hjosugi/sql-dialect-fmt:1.8.0 --check .
```

Try the browser playground from the docs site:
<https://hjosugi.github.io/sql-dialect-fmt/playground.html>

## Usage

```sh
sql-dialect-fmt query.sql                 # format to stdout
sql-dialect-fmt --write *.sql             # rewrite files in place
sql-dialect-fmt --check src/**/*.sql      # non-zero when files are not formatted
sql-dialect-fmt --check --diff query.sql  # show a unified diff for unformatted input
cat query.sql | sql-dialect-fmt           # stdin to stdout
cat query.sql | sql-dialect-fmt -         # explicitly read stdin with `-`
sql-dialect-fmt --stdin-filepath src/query.sql < query.sql  # use a path for config discovery

# Options: --dialect snowflake|databricks / --line-width N / --indent-width N / --no-uppercase
```

pre-commit users can enable the official hooks:

```yaml
repos:
  - repo: https://github.com/hjosugi/sql-dialect-fmt
    rev: v1.8.0
    hooks:
      - id: sql-dialect-fmt
```

Use `sql-dialect-fmt-check` instead when a hook should only verify formatting.

## Browser Extension

The Chrome extension in `extensions/chrome` formats SQL in Snowsight and Databricks browser
editors. It bundles the Rust formatter as WebAssembly, so no local server is needed.

```sh
./scripts/build-chrome-extension.sh
```

Then open `chrome://extensions`, enable Developer mode, and load `extensions/chrome` unpacked.
Focus a SQL editor and run the formatter from the floating button, the extension action, or
`Alt+Shift+F`. The options page controls dialect, line width, indent width, and keyword casing.

Release packages for the Chrome extension and VS Code extension are built together:

```sh
./scripts/package-extensions.sh
```

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo fmt --all --check
```

## Status

Snowflake support covers SELECT, DML (`INSERT`/`UPDATE`/`DELETE`/`MERGE`), `COPY`, major DDL and
object DDL, Semantic View, and `CREATE PROCEDURE`/`CREATE FUNCTION` bodies in SQL, JavaScript,
Python, Java, and Scala. Databricks mode covers LATERAL VIEW, Delta DDL options,
`VERSION`/`TIMESTAMP AS OF`, higher-order-function lambdas, SQL scripting blocks, and backtick
identifiers.

The workspace also includes an LSP server, semantic tokens, hover text, a Tree-sitter grammar, a
CLI, VS Code packaging, and the Chrome/WASM extension. The headline formatter feature is
**magic trailing comma**. See [ROADMAP.md](ROADMAP.md) for the detailed coverage map.

## Crates

| crate | role |
| --- | --- |
| `sql-dialect-fmt-syntax` | `SyntaxKind`, keyword recognition, and `rowan` language definition |
| `sql-dialect-fmt-lexer` | hand-written lossless lexer |
| `sql-dialect-fmt-parser` | resilient lossless CST parser |
| `sql-dialect-fmt-formatter` | generic Doc IR engine plus SQL formatting rules |
| `sql-dialect-fmt-highlight` | syntax highlight token classification |
| `sql-dialect-fmt-hover` | hover text for types, routines, and tasks |
| `sql-dialect-fmt-tree-sitter` | Rust bindings for the bundled Tree-sitter grammar |
| `sql-dialect-fmt-lsp` | Language Server over stdio |
| `sql-dialect-fmt-wasm` | raw WebAssembly bridge for browser extensions |
| `sql-dialect-fmt` | CLI binary crate (`crates/sql-dialect-fmt-cli`) |

## License

0BSD. You can use, copy, modify, and distribute this project for almost any purpose.
