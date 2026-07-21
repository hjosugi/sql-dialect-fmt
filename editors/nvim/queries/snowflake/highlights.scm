; Copied from tree-sitter-snowflake/queries/highlights.scm (source of truth) - keep in sync.
; Tree-sitter highlight captures for Snowflake SQL.
;
; These names intentionally use common editor scopes so Neovim, Helix, Zed, and
; tree-sitter-highlight can map them without Snowflake-specific theme support.

(comment) @comment

(keyword) @keyword
(type) @type

(identifier) @variable
(quoted_identifier) @variable.member

(string) @string
(dollar_string) @string.special

(number) @number
(variable) @variable.parameter
(placeholder) @variable.parameter
(stage_reference) @string.special

(operator) @operator
(punctuation) @punctuation.delimiter
