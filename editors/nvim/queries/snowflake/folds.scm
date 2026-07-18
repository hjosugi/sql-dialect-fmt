; Copied from tree-sitter-snowflake/queries/folds.scm (source of truth) - keep in sync.
; Folding regions for Snowflake SQL.
;
; Now that the grammar groups each top-level statement into a `statement` node,
; editors (Neovim, Helix) can collapse a statement at a time. Block comments fold
; too. This mirrors the LSP server's `textDocument/foldingRange`, which folds the
; same statement boundaries from the rowan CST.

(statement) @fold

((comment) @fold
  (#match? @fold "^/\\*"))
