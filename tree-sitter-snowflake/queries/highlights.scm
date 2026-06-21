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
(stage_reference) @string.special

(operator) @operator
(punctuation) @punctuation.delimiter
