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
lexer/highlighter by tests in `sql-dialect-fmt-highlight` (`tests/textmate.rs`): every word the
grammar scopes as a keyword or type must be classified the same way by
`sql_dialect_fmt_highlight::classify`, so the grammar can't drift from the rest of the toolchain.

### VS Code extension package

The `editors/` directory is also a minimal VS Code extension root. Package it from this
directory with `vsce package` (or install it locally with VS Code's "Install from VSIX") to
contribute:

- language id: `snowflake-sql`
- grammar scope: `source.snowflake-sql`
- file extensions: `.sql`, `.snowsql`, `.sfsql`

### Embedding the grammar manually

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

## Semantic tokens (LSP)

For LSP-based editors, `sql-dialect-fmt-highlight::semantic` maps the lexical highlighter onto the
standard LSP semantic-token legend (`keyword`, `type`, `variable`, `string`, `number`,
`parameter`, `operator`, `comment`, `namespace`) plus `documentation` / `defaultLibrary`
modifiers, and exposes `$$ … $$` embedded-language regions as `Injection`s (JavaScript,
Python, Java, Scala, or SQL, picked from the `LANGUAGE` clause). The `sql-dialect-fmt-lsp` server
delta-encodes these for `textDocument/semanticTokens/full`.
