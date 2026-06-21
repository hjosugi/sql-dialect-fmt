# Delimiter Strategy

Snowflake procedure/function bodies are tricky because they often contain
semicolons, comments, strings, and an embedded language. The formatter must not
split or reinterpret those bodies while lexing the outer SQL.

## Current Snowflake Behavior

Snowflake documents procedure bodies as string-like procedure definitions. For
Snowflake Scripting in SnowSQL, Snowsight, Snowflake CLI, and Python Connector
contexts, Snowflake explicitly calls out string literal delimiters `'` and `$$`
around procedure definitions.

Sources:

- CREATE PROCEDURE: https://docs.snowflake.com/en/sql-reference/sql/create-procedure
- Snowflake Scripting client delimiters:
  https://docs.snowflake.com/en/developer-guide/snowflake-scripting/running-examples

snow-fmt therefore treats `$$...$$` as one lossless body token. Single-quoted
procedure bodies remain ordinary SQL strings at the lexer layer; the parser can
later decide that a string after `AS` is a procedure body.

## Prior-Art Lessons

- sqlglot keeps quotes, identifiers, comments, raw strings, and heredoc strings
  as dialect-configured tokenizer tables. This is the right shape for delimiter
  changes: data first, state machine second.
  Source: https://github.com/tobymao/sqlglot/blob/main/sqlglot/tokens.py
- sqlfluff keeps Snowflake grammar in a dialect layer and uses a lossless segment
  model, which reinforces that dialect-specific body constructs should not be
  scattered across formatter rules.
  Source:
  https://github.com/sqlfluff/sqlfluff/blob/main/src/sqlfluff/dialects/dialect_snowflake.py
- tree-sitter grammars are static, but grammar.js can still keep body delimiter
  rules in one list so additions are localized.
- sqlparser-rs is a useful syntax reference, but its AST is not lossless enough
  for formatter ownership of embedded bodies.
- Recent dialect-agnostic SQL parsing research (SQLFlex, 2026) also reinforces a
  conservative split: keep known grammar/token anchors deterministic, isolate
  dialect-specific or not-yet-supported fragments, and preserve validation hooks
  instead of guessing new syntax into the core parser.
  Source: https://arxiv.org/abs/2603.16155

## snow-fmt Rule

The lexer owns body delimiter recognition. It exposes:

- `BodyDelimiter`: opener/closer/name
- `LexOptions`: a table of body delimiters
- `DEFAULT_BODY_DELIMITERS`: currently `$$...$$`

Default behavior must match current Snowflake. Future delimiter candidates must
be opt-in until Snowflake documents them, because `$name` is a valid Snowflake
variable form and speculative `$tag$` support could otherwise change tokenization.

## Adding a Future Delimiter

1. Add a `BodyDelimiter` to `DEFAULT_BODY_DELIMITERS` only after Snowflake
   documents it.
2. Add focused lexer tests showing the new body is lossless and does not swallow
   existing variables/operators.
3. Update `tree-sitter-snowflake/grammar.js` in `BODY_DELIMITER_RULES`.
4. Regenerate Tree-sitter parser files.
5. Add highlight/hover/parser tests only if the delimiter changes user-visible
   behavior beyond tokenization.

This keeps delimiter churn away from parser recovery, formatter rules, hover
text, and editor adapters.
