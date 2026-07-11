<!-- i18n: language-switcher -->
[English](snowflake-github-prior-art.md) | [日本語](snowflake-github-prior-art.ja.md)

# Snowflake GitHub Prior Art Notes

Last checked: 2026-06-21.

## Official Snowflake Notes

- Snowflake's current flow/pipe operator is `->>`, not `|>`. The docs describe it as the only supported flow operator and show statement chains like `<sql_statement_1> ->> <sql_statement_2>`. Source: https://docs.snowflake.com/en/sql-reference/operators-flow
- Snowflake release 9.13 introduced the pipe operator in May 2025. Source: https://docs.snowflake.com/en/release-notes/2025/9_13
- 2026 release notes continue to add parser-relevant SQL surface area: user-defined types, interval data types, procedure-scoped temp table syntax, virtual columns, semantic view changes, and Cortex AI function changes. Source: https://docs.snowflake.com/en/release-notes/new-features-2026

## GitHub Projects Checked

- `sqlfluff/sqlfluff` — dialect-flexible SQL linter/formatter with Snowflake support and dbt/Jinja awareness. Useful reference for broad dialect coverage and rule organization, but it is Python and highly configurable. Source: https://github.com/sqlfluff/sqlfluff
- `tobymao/sqlglot` — no-dependency Python parser/transpiler/optimizer with Snowflake among 31 dialects. Useful reference for dialect feature flags, parser overrides, and wide corpus testing. Source: https://github.com/tobymao/sqlglot
- `DerekStride/tree-sitter-sql` — general/permissive Tree-sitter SQL grammar. Useful reference for editor highlighting tradeoffs: permissive parsing, generated parser distribution, and known SQL highlighting edge cases. Source: https://github.com/DerekStride/tree-sitter-sql
- `sql-formatter-org/sql-formatter` — JavaScript pretty-printer with Snowflake support, but explicitly does not support stored procedures. Useful as a formatter UX reference and a warning that Snowflake Scripting/procedures are a core differentiator for sql-dialect-fmt. Source: https://github.com/sql-formatter-org/sql-formatter
- `tobilg/polyglot` — Rust/Wasm SQL parser/transpiler/formatter for 32+ dialects, inspired by sqlglot. Useful to watch for Rust-side AST/visitor/stack-safety patterns. Source: https://github.com/tobilg/polyglot

## Research Checked

- SQLFlex (SIGMOD/PACMMOD 2026) argues that dialect-specific syntax is a major
  failure mode for grammar-based SQL tooling and proposes isolating unknown
  dialect fragments behind validated segmentation. sql-dialect-fmt should keep the core
  lexer/parser deterministic, but the same lesson applies operationally: preserve
  lossless tokens/ranges around Snowflake-specific bodies and make dialect churn
  explicit in configuration and tests. Source: https://arxiv.org/abs/2603.16155

## Design Implications For sql-dialect-fmt

- Keep the lexer lossless and permissive. SQLFluff/tree-sitter-sql both show the value of surviving dialect edges; sql-dialect-fmt should continue returning tokens/ranges even when grammar support lags.
- Keep dialect changes data-driven where possible. The new `FLOW_PIPE` token and dynamic easy-test fixture discovery are examples: when Snowflake adds syntax, tests can grow without hardcoded case lists.
- Preserve a Snowflake-specific advantage: SQL Scripting and embedded JavaScript/Python bodies. Existing formatters often under-support procedures; the embedded easy fixture crate should remain the always-on regression gate, while any external generated corpus stays optional.
- Highlighting should start lexical and become semantic. The new `sql-dialect-fmt-highlight` crate gives stable ranges/scopes now; later LSP semantic tokens can layer parser context on top without replacing it.
