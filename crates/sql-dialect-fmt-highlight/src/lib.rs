//! Lexical syntax highlighting for Snowflake SQL.
//!
//! This layer intentionally starts from the lossless lexer. It is stable under grammar growth:
//! new keywords are highlighted by `keyword_kind`, while newly-added punctuation only needs a
//! `SyntaxKind` classification here before LSP/TextMate adapters consume it.

use sql_dialect_fmt_lexer::{tokenize, LexError};
use sql_dialect_fmt_syntax::{is_builtin_type, keyword_kind, SyntaxKind};

pub mod semantic;
pub use semantic::{
    delta_encode, detect_injections, line_tokens, resolve_tokens, semantic_token, semantic_tokens,
    semantic_tokens_lsp, semantic_tokens_lsp_utf8, InjectedLanguage, Injection, LineToken,
    ResolvedToken, SemanticTokenModifiers, SemanticTokenType, SemanticTokens,
};

/// The result of [`highlight`]: a lossless token stream plus any lexer errors. `#[non_exhaustive]`
/// so fields can be added without breaking downstream matches.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Highlighted<'a> {
    /// Every token in source order; concatenating their `text` reproduces the input byte-for-byte.
    pub tokens: Vec<HighlightToken<'a>>,
    /// Lexer errors recovered while tokenizing (empty for clean input).
    pub errors: Vec<LexError>,
}

/// A single highlighted token: a classified [`HighlightKind`] over a byte range of the source.
/// `#[non_exhaustive]` so fields can be added without breaking downstream matches.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct HighlightToken<'a> {
    /// The highlight classification of this token.
    pub kind: HighlightKind,
    /// The token's exact source text (the slice `&input[range]`).
    pub text: &'a str,
    /// Byte range of the token in the source.
    pub range: std::ops::Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Whitespace,
    Comment,
    Keyword,
    Type,
    Identifier,
    QuotedIdentifier,
    String,
    DollarString,
    Number,
    Variable,
    Operator,
    Punctuation,
    Error,
}

impl HighlightKind {
    pub const fn scope(self) -> &'static str {
        match self {
            HighlightKind::Whitespace => "text.whitespace",
            HighlightKind::Comment => "comment.line-or-block",
            HighlightKind::Keyword => "keyword.sql",
            HighlightKind::Type => "support.type.sql",
            HighlightKind::Identifier => "identifier.sql",
            HighlightKind::QuotedIdentifier => "identifier.quoted.sql",
            HighlightKind::String => "string.quoted.sql",
            HighlightKind::DollarString => "string.dollar-quoted.sql",
            HighlightKind::Number => "constant.numeric.sql",
            HighlightKind::Variable => "variable.parameter.sql",
            HighlightKind::Operator => "operator.sql",
            HighlightKind::Punctuation => "punctuation.sql",
            HighlightKind::Error => "invalid.illegal.sql",
        }
    }
}

pub fn highlight(input: &str) -> Highlighted<'_> {
    let lexed = tokenize(input);
    let tokens = lexed
        .tokens
        .into_iter()
        .scan(0usize, |offset, token| {
            let start = *offset;
            *offset += token.text.len();
            Some(HighlightToken {
                kind: classify(token.kind, token.text),
                text: token.text,
                range: start..*offset,
            })
        })
        .collect();

    Highlighted {
        tokens,
        errors: lexed.errors,
    }
}

pub fn classify(kind: SyntaxKind, text: &str) -> HighlightKind {
    if kind.is_trivia() {
        return match kind {
            SyntaxKind::COMMENT | SyntaxKind::BLOCK_COMMENT => HighlightKind::Comment,
            _ => HighlightKind::Whitespace,
        };
    }

    if kind.is_keyword()
        || kind == SyntaxKind::CONTEXTUAL_KEYWORD
        || (kind == SyntaxKind::IDENT && keyword_kind(text).is_some())
    {
        return HighlightKind::Keyword;
    }
    if kind == SyntaxKind::IDENT && is_builtin_type(text) {
        return HighlightKind::Type;
    }

    match kind {
        SyntaxKind::IDENT => HighlightKind::Identifier,
        SyntaxKind::QUOTED_IDENT => HighlightKind::QuotedIdentifier,
        SyntaxKind::STRING => HighlightKind::String,
        SyntaxKind::DOLLAR_STRING => HighlightKind::DollarString,
        SyntaxKind::FILE_URI => HighlightKind::String,
        SyntaxKind::INT_NUMBER | SyntaxKind::FLOAT_NUMBER => HighlightKind::Number,
        SyntaxKind::VARIABLE
        | SyntaxKind::PLACEHOLDER
        | SyntaxKind::DOLLAR
        | SyntaxKind::QUESTION => HighlightKind::Variable,
        SyntaxKind::ERROR | SyntaxKind::BANG => HighlightKind::Error,
        kind if is_operator(kind) => HighlightKind::Operator,
        _ => HighlightKind::Punctuation,
    }
}

fn is_operator(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::COLON
            | SyntaxKind::COLON2
            | SyntaxKind::ASSIGN
            | SyntaxKind::EQ
            | SyntaxKind::NEQ
            | SyntaxKind::LT
            | SyntaxKind::LTE
            | SyntaxKind::GT
            | SyntaxKind::GTE
            | SyntaxKind::PLUS
            | SyntaxKind::MINUS
            | SyntaxKind::STAR
            | SyntaxKind::SLASH
            | SyntaxKind::PERCENT
            | SyntaxKind::CONCAT
            | SyntaxKind::PIPE
            | SyntaxKind::PIPE_GT
            | SyntaxKind::FLOW_PIPE
            | SyntaxKind::ARROW
            | SyntaxKind::FAT_ARROW
            | SyntaxKind::AMP
            | SyntaxKind::CARET
            | SyntaxKind::TILDE
            | SyntaxKind::AT
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // The TextMate-grammar / highlighter consistency check now lives in `tests/textmate.rs`
    // against the canonical `editors/snowflake.tmLanguage.json`. The old, superseded grammar
    // under `editors/textmate/` (scope `source.sql.snowflake`) has been removed, so the
    // `textmate_grammar_matches_the_highlighter` test that pinned it is gone with it.

    #[test]
    fn classifies_core_snowflake_tokens() {
        let h = highlight("SELECT $1::NUMBER ->> SELECT \"name\" FROM $1 -- ok\n");
        assert!(h.errors.is_empty());

        let interesting: Vec<_> = h
            .tokens
            .iter()
            .filter(|token| !matches!(token.kind, HighlightKind::Whitespace))
            .map(|token| (token.text, token.kind))
            .collect();

        assert_eq!(
            interesting,
            vec![
                ("SELECT", HighlightKind::Keyword),
                ("$1", HighlightKind::Variable),
                ("::", HighlightKind::Operator),
                ("NUMBER", HighlightKind::Type),
                ("->>", HighlightKind::Operator),
                ("SELECT", HighlightKind::Keyword),
                ("\"name\"", HighlightKind::QuotedIdentifier),
                ("FROM", HighlightKind::Keyword),
                ("$1", HighlightKind::Variable),
                ("-- ok", HighlightKind::Comment),
            ]
        );
    }

    #[test]
    fn ranges_cover_input_losslessly() {
        let sql = "SELECT payload:items[0]::VARIANT FROM raw";
        let highlighted = highlight(sql);
        assert!(highlighted.errors.is_empty());
        let joined: String = highlighted.tokens.iter().map(|token| token.text).collect();
        assert_eq!(joined, sql);
        for token in &highlighted.tokens {
            assert_eq!(&sql[token.range.clone()], token.text);
        }
    }

    #[test]
    fn unquoted_file_uri_is_string_like_not_a_comment() {
        let highlighted = highlight("PUT file:///tmp/data/mydata.csv @stage");
        assert!(highlighted.errors.is_empty());
        let uri = highlighted
            .tokens
            .iter()
            .find(|token| token.text.starts_with("file://"))
            .expect("a file URI token");
        assert_eq!(uri.kind, HighlightKind::String);
        assert!(!highlighted
            .tokens
            .iter()
            .any(|token| token.kind == HighlightKind::Comment));
    }
}
