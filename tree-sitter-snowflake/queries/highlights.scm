; Tree-sitter highlight captures for Snowflake SQL.
;
; These names intentionally use common editor scopes so Neovim, Helix, Zed, and
; tree-sitter-highlight can map them without Snowflake-specific theme support.

(comment) @comment

(keyword) @keyword
; Structural anchor keywords lifted out of the catch-all `keyword` token.
(kw_create) @keyword
(kw_language) @keyword
(type) @type

(identifier) @variable
(quoted_identifier) @variable.member

(string) @string
(dollar_string) @string.special

(number) @number
(variable) @variable.parameter
(stage_reference) @string.special

(operator) @operator
(punctuation) @punctuation.delimiter
(terminator) @punctuation.delimiter
