//! Formatting configuration.
//!
//! snow-fmt follows the `gofmt`/Prettier philosophy of being opinionated with very few knobs:
//! the only decisions a user makes are the target line width, the indent width, and how to case
//! keywords. Everything else is fixed so that any two snow-fmt users produce identical output.

/// How SQL keywords are cased in the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeywordCase {
    /// `SELECT`, `FROM`, … (the SQL convention, and the default).
    Upper,
    /// `select`, `from`, …
    Lower,
    /// Leave each keyword's casing exactly as written in the source.
    ///
    /// Note: keywords that snow-fmt *generates* (e.g. a normalized `AS`) are emitted upper-case,
    /// since there is no source token to copy a casing from.
    Preserve,
}

impl KeywordCase {
    /// Apply this casing policy to a keyword's text.
    pub(crate) fn apply(self, keyword: &str) -> String {
        match self {
            KeywordCase::Upper => keyword.to_ascii_uppercase(),
            KeywordCase::Lower => keyword.to_ascii_lowercase(),
            KeywordCase::Preserve => keyword.to_string(),
        }
    }
}

/// Options controlling the formatter's output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatOptions {
    /// The column the printer tries to keep lines within. Groups that would exceed it break.
    pub line_width: usize,
    /// Number of spaces per indentation level.
    pub indent_width: usize,
    /// How to case SQL keywords.
    pub keyword_case: KeywordCase,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            line_width: 80,
            indent_width: 2,
            keyword_case: KeywordCase::Upper,
        }
    }
}
