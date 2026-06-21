; Baseline locals query.
;
; Snowflake SQL identifiers are context-sensitive enough that definitions should
; come from the CST/LSP semantic layer. Marking generic references still gives
; editors a stable hook for hover and selection features without claiming false
; definitions.

(identifier) @local.reference
