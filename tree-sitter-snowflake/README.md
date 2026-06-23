# tree-sitter-snowflake

Tree-sitter grammar for Snowflake SQL in `snow-fmt`.

This grammar is deliberately token-centric. Snowflake changes quickly, and the
lossless Rust CST parser remains the formatter source of truth. The Tree-sitter
grammar gives editors a robust baseline for highlighting, selection, and hover
plumbing without rejecting newer Snowflake syntax.

Over that flat token stream the grammar adds one structural layer: each
top-level statement (a run of tokens up to its `;`) is grouped into a
`statement` node. That is enough for statement-level folding (`queries/folds.scm`,
mirroring the LSP server's `textDocument/foldingRange`) and navigation, while
staying tolerant of unfamiliar syntax — it does not commit to a full expression
grammar. Expression nodes, indents, and context-aware injections remain future
work.

## Development

```sh
npm install
npm run generate
npm test
```

The generated C parser is consumed by the Rust wrapper crate:
`crates/snow-fmt-tree-sitter`.

## Publishing Shape

This directory can be published as the `tree-sitter-snowflake` grammar package
for editors and tooling that consume Tree-sitter grammars directly.

The Rust workspace exposes the same generated parser through
`snow-fmt-tree-sitter`, which is the crate to publish for Rust/LSP consumers.
Editor plugins should be thin adapters on top of this grammar:

- Neovim/Helix/Zed: grammar + `queries/*.scm`
- VS Code: extension package that bundles the grammar and maps scopes to a theme
- LSP hover/semantic tokens: use the lossless CST parser as the source of truth,
  with Tree-sitter node ranges as the fast editor baseline

This is not a Snowflake Marketplace/Native App package; it is an editor/tooling
plugin path for Snowflake SQL.
