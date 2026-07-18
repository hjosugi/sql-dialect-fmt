; Folding regions for Snowflake SQL.
;
; The grammar groups each top-level statement into a statement-kind node
; (`select_statement`, `create_statement`, ... with `statement` as the lenient
; fallback), so editors (Neovim, Helix) can collapse a statement at a time.
; Block comments fold too. This mirrors the LSP server's
; `textDocument/foldingRange`, which folds the same statement boundaries from
; the rowan CST.

[
  (select_statement)
  (insert_statement)
  (update_statement)
  (delete_statement)
  (merge_statement)
  (create_statement)
  (drop_statement)
  (alter_statement)
  (grant_statement)
  (revoke_statement)
  (copy_statement)
  (use_statement)
  (set_statement)
  (show_statement)
  (describe_statement)
  (statement)
] @fold

((comment) @fold
  (#match? @fold "^/\\*"))
