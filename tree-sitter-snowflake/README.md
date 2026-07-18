<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# tree-sitter-snowflake

Tree-sitter grammar for Snowflake SQL in `sql-dialect-fmt`.

This grammar is deliberately token-centric. Snowflake changes quickly, and the
lossless Rust CST parser remains the formatter source of truth. The Tree-sitter
grammar gives editors a robust baseline for highlighting, selection, and hover
plumbing without rejecting newer Snowflake syntax.

Over that flat token stream the grammar adds two editor-oriented structural
layers:

- each top-level statement (a run of tokens up to its `;`) is grouped into a
  `statement` node for folding and navigation;
- balanced parentheses and immediate function-call syntax are grouped under
  lightweight `expression` nodes (`call_expression`, `parenthesized_expression`).

The query set also provides context-aware injections for Snowflake
`LANGUAGE <name> ... AS $$...$$` bodies and `EXECUTE IMMEDIATE $$...$$`. The
grammar still does not try to be the full formatter grammar; unknown Snowflake
syntax should remain tokenized and parseable. Indents remain future work.

## Development

```sh
npm install
npm run generate
npm test
```

The generated C parser is consumed by the Rust wrapper crate:
`crates/sql-dialect-fmt-tree-sitter`.

## Publishing Shape

This directory can be published as the `tree-sitter-snowflake` grammar package
for editors and tooling that consume Tree-sitter grammars directly.

The Rust workspace exposes the same generated parser through
`sql-dialect-fmt-tree-sitter`, which is the crate to publish for Rust/LSP consumers.
Editor plugins should be thin adapters on top of this grammar:

- Neovim/Helix/Zed: grammar + `queries/*.scm`
- VS Code: extension package that bundles the grammar and maps scopes to a theme
- LSP hover/semantic tokens: use the lossless CST parser as the source of truth,
  with Tree-sitter node ranges as the fast editor baseline

This is not a Snowflake Marketplace/Native App package; it is an editor/tooling
plugin path for Snowflake SQL.
