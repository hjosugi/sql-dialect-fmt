# Snowflake SQL — TextMate grammar

`snowflake.tmLanguage.json` is a baseline [TextMate grammar](https://macromates.com/manual/en/language_grammars)
for Snowflake SQL, for plain editors that consume `.tmLanguage` files (VS Code,
Sublime Text, TextMate, GitHub Linguist). It is a *lexical* grammar: it scopes
comments, strings (including `$$…$$`), numbers, types, keywords, variables, and
operators, but does not parse statement structure — for that, use the
[Tree-sitter grammar](../../tree-sitter-snowflake/) or the
[LSP server](../../crates/snow-fmt-lsp/).

- Scope name: `source.sql.snowflake`
- File types: `.sql`

The keyword and type word lists are kept consistent with the formatter's own
lexer/highlighter by a test in `snow-fmt-highlight`
(`textmate_grammar_matches_the_highlighter`): every word the grammar scopes as a
keyword or type must be classified the same way by `snow_fmt_highlight::classify`,
so the grammar can't drift away from the rest of the toolchain.

## Using it in VS Code

Reference the file from a small extension's `package.json`:

```json
{
  "contributes": {
    "grammars": [
      {
        "scopeName": "source.sql.snowflake",
        "path": "./snowflake.tmLanguage.json",
        "language": "sql"
      }
    ]
  }
}
```
