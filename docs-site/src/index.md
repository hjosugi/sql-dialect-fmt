# sql-dialect-fmt

`sql-dialect-fmt` is an opinionated Rust formatter and editor toolchain for Snowflake SQL and
Databricks SQL. It is built around a lossless lexer/parser pipeline: unsupported or malformed input
is preserved, comments survive formatting, and repeated formatting is stable.

Use the [playground](playground.md) to run the WebAssembly formatter in the browser, or install the
CLI for local and CI usage.

```sh
cargo install sql-dialect-fmt --version 1.2.0 --locked
sql-dialect-fmt --check sql/**/*.sql
sql-dialect-fmt --write sql/**/*.sql
```

## Coverage

- Snowflake SELECT, DML, COPY, major DDL and object DDL.
- Snowflake Scripting and routine bodies in SQL, JavaScript, Python, Java, and Scala.
- Databricks mode for backtick identifiers, LATERAL VIEW, Delta DDL options, time travel, and
  higher-order function lambdas.
- LSP, semantic tokens, hover text, Tree-sitter grammar, VS Code packaging, and Chrome/WASM
  extension packaging.

The detailed tracker lives in `spec/seed/features.json`; run:

```sh
python3 spec/snowflake_spec.py coverage
```
