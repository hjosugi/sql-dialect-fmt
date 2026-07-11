# Snowflake SQL

Focused Snowflake SQL syntax highlighting for Visual Studio Code, backed by the same keyword and
type definitions used by [sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt).

![Snowflake SQL syntax highlighting](images/syntax-highlighting.png)

## Features

- Snowflake SQL keywords and built-in types
- Snowflake Scripting and `$$ ... $$` routine bodies
- line (`--`, `//`) and block (`/* ... */`) comments
- strings, quoted identifiers, numeric literals, and operators
- positional `$1`, session `$name`, bind `:name`, and `?` variables
- `@stage`, `@~`, `@%table`, and namespaced stage references
- `.sql`, `.snowsql`, and `.sfsql` file associations

- **Language ID:** `snowflake-sql`
- **Scope name:** `source.snowflake-sql`
- **File types:** `.sql`, `.snowsql`, `.sfsql`

The keyword and type word lists are kept in lock-step with the formatter's own
lexer/highlighter by tests in `sql-dialect-fmt-highlight` (`tests/textmate.rs`): every word the
grammar scopes as a keyword or type must be classified the same way by
`sql_dialect_fmt_highlight::classify`, so the grammar can't drift from the rest of the toolchain.

## Use

1. Install the extension.
2. Open a `.sql`, `.snowsql`, or `.sfsql` file.
3. If needed, choose **Change Language Mode** and select **Snowflake SQL**.

This extension contributes syntax highlighting and language metadata. It does not execute SQL,
connect to Snowflake, or include the browser formatter. For CLI formatting and other integrations,
see the [main project README](https://github.com/hjosugi/sql-dialect-fmt#readme).

## Privacy

The extension contains no runtime code, telemetry, analytics, network requests, or remote
formatting. It only contributes static language configuration and TextMate grammar files. See the
[privacy policy](https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md).

## Support and source

- [Report an issue](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [Source code](https://github.com/hjosugi/sql-dialect-fmt)
- License: [0BSD](LICENSE.md)
