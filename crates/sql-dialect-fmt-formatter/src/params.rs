//! Parameter-placeholder substitution (the `params` option).
//!
//! `sql-formatter` can replace prepared-statement placeholders with values while formatting. This
//! module implements the same idea as a post-pass: the formatter runs first (so layout and
//! keyword casing are unaffected by the values), then a lossless re-lex of the **formatted** output
//! replaces each recognized placeholder with the next supplied value, in order of appearance.
//!
//! Recognized placeholders are the tokens the lexer already marks as parameters:
//! * `?` (positional) and `?1` (numbered; the following number is swallowed),
//! * `$1` / `$name`, `:name`, and `@name` (variant-specific variable tokens).
//!
//! Values are inserted verbatim, so callers pass already-quoted SQL (for example `"'bar'"`).
//! Placeholders beyond the supplied values are left untouched.

use sql_dialect_fmt_lexer::{tokenize_with_options, LexOptions, ParamTypes, SyntaxKind};
use sql_dialect_fmt_syntax::Dialect;

/// Replace recognized placeholders in `formatted` with `params`, in order of appearance.
///
/// Returns `formatted` unchanged when `params` is empty.
#[must_use]
pub fn substitute_params(formatted: &str, dialect: Dialect, params: &[String]) -> String {
    substitute_params_with(formatted, dialect, None, params)
}

/// Like [`substitute_params`], but with an explicit placeholder-recognition override.
#[must_use]
pub fn substitute_params_with(
    formatted: &str,
    dialect: Dialect,
    param_types: Option<ParamTypes>,
    params: &[String],
) -> String {
    if params.is_empty() {
        return formatted.to_string();
    }

    let lexed = tokenize_with_options(
        formatted,
        LexOptions::default()
            .with_dialect(dialect)
            .with_param_types(param_types),
    );
    let tokens = &lexed.tokens;

    let mut out = String::with_capacity(formatted.len());
    let mut last = 0usize;
    let mut offset = 0usize;
    let mut next_param = 0usize;
    let mut i = 0usize;

    while i < tokens.len() {
        let token = &tokens[i];
        let start = offset;
        let end = offset + token.text.len();
        let mut consumed_end = end;

        let is_placeholder = matches!(token.kind, SyntaxKind::QUESTION | SyntaxKind::VARIABLE);
        if is_placeholder && next_param < params.len() {
            // `?1` lexes as `?` then `1`; swallow the number so the value replaces both.
            if token.kind == SyntaxKind::QUESTION {
                if let Some(next) = tokens.get(i + 1) {
                    if next.kind == SyntaxKind::INT_NUMBER
                        && end + next.text.len() <= formatted.len()
                    {
                        consumed_end = end + next.text.len();
                        i += 1;
                    }
                }
            }
            out.push_str(&formatted[last..start]);
            out.push_str(&params[next_param]);
            next_param += 1;
            last = consumed_end;
        }

        offset = consumed_end;
        i += 1;
    }

    out.push_str(&formatted[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::substitute_params;
    use sql_dialect_fmt_syntax::Dialect;

    #[test]
    fn replaces_positional_and_numbered_placeholders() {
        assert_eq!(
            substitute_params(
                "SELECT ? FROM t WHERE a = ?",
                Dialect::Snowflake,
                &["'x'".to_string(), "1".to_string()]
            ),
            "SELECT 'x' FROM t WHERE a = 1"
        );
        assert_eq!(
            substitute_params(
                "SELECT ?1, ?2",
                Dialect::Snowflake,
                &["'a'".to_string(), "'b'".to_string()]
            ),
            "SELECT 'a', 'b'"
        );
    }

    #[test]
    fn replaces_variables_and_leaves_extra_text() {
        assert_eq!(
            substitute_params("WHERE id = :id", Dialect::PlSql, &["42".to_string()]),
            "WHERE id = 42"
        );
        assert_eq!(
            substitute_params("WHERE id = $1", Dialect::PostgreSql, &["42".to_string()]),
            "WHERE id = 42"
        );
        // More placeholders than values: the remainder is preserved.
        assert_eq!(
            substitute_params("?, ?", Dialect::Snowflake, &["1".to_string()]),
            "1, ?"
        );
    }

    #[test]
    fn empty_params_is_identity() {
        assert_eq!(
            substitute_params("SELECT ?", Dialect::Snowflake, &[]),
            "SELECT ?"
        );
    }
}
