# Editor integration assets

This directory holds the editor-facing highlighting assets for Snowflake SQL.

## `snowflake.tmLanguage.json`

A complete [TextMate grammar](https://macromates.com/manual/en/language_grammars) for
Snowflake SQL, consumable by any editor that loads `.tmLanguage` files (VS Code, Sublime
Text, TextMate, GitHub Linguist, Zed). It is a *lexical* grammar — it scopes tokens, not
statement structure — covering:

- line (`--`, `//`) and block (`/* */`) comments
- single-quoted strings (with `''` / `\\` escapes) and `$$ … $$` dollar-quoted bodies
- numeric literals (integers, floats, leading-dot, exponents)
- built-in types and the full reserved-keyword set
- variables: positional `$1`, session `$name`, bind `:name`, placeholder `?`
- stage references: `@stage`, `@~` (user stage), `@%table` (table stage), `@ns.stage/path`
- double-quoted identifiers (with `""` escape)
- operators including the Snowflake/GoogleSQL-specific `::`, `:`, `->`, `->>`, `=>`, `|>`,
  `||`, and `:=`
- structural punctuation

- **Scope name:** `source.snowflake-sql`
- **File types:** `.sql`, `.snowsql`

The keyword and type word lists are kept in lock-step with the formatter's own
lexer/highlighter by tests in `snow-fmt-highlight` (`tests/textmate.rs`): every word the
grammar scopes as a keyword or type must be classified the same way by
`snow_fmt_highlight::classify`, so the grammar can't drift from the rest of the toolchain.

### Using it in VS Code

```json
{
  "contributes": {
    "grammars": [
      {
        "scopeName": "source.snowflake-sql",
        "path": "./snowflake.tmLanguage.json",
        "language": "sql"
      }
    ]
  }
}
```

## `textmate/snowflake.tmLanguage.json`

The original baseline grammar (`source.sql.snowflake`). Kept for editors already wired to
that scope name; the top-level `snowflake.tmLanguage.json` above is the more complete
successor.

## Semantic tokens (LSP)

For LSP-based editors, `snow-fmt-highlight::semantic` maps the lexical highlighter onto the
standard LSP semantic-token legend (`keyword`, `type`, `variable`, `string`, `number`,
`parameter`, `operator`, `comment`, `namespace`) plus `documentation` / `defaultLibrary`
modifiers, and exposes `$$ … $$` embedded-language regions as `Injection`s (JavaScript,
Python, Java, Scala, or SQL, picked from the `LANGUAGE` clause). The `snow-fmt-lsp` server
delta-encodes these for `textDocument/semanticTokens/full`.
