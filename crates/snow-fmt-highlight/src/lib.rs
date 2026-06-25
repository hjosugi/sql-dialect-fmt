//! Lexical syntax highlighting for Snowflake SQL.
//!
//! This layer intentionally starts from the lossless lexer. It is stable under grammar growth:
//! new keywords are highlighted by `keyword_kind`, while newly-added punctuation only needs a
//! `SyntaxKind` classification here before LSP/TextMate adapters consume it.

use snow_fmt_lexer::{tokenize, LexError};
use snow_fmt_syntax::{keyword_kind, SyntaxKind};

pub mod semantic;
pub use semantic::{
    delta_encode, detect_injections, line_tokens, resolve_tokens, semantic_token, semantic_tokens,
    semantic_tokens_lsp, InjectedLanguage, Injection, LineToken, ResolvedToken,
    SemanticTokenModifiers, SemanticTokenType, SemanticTokens,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Highlighted<'a> {
    pub tokens: Vec<HighlightToken<'a>>,
    pub errors: Vec<LexError>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightToken<'a> {
    pub kind: HighlightKind,
    pub text: &'a str,
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
        SyntaxKind::INT_NUMBER | SyntaxKind::FLOAT_NUMBER => HighlightKind::Number,
        SyntaxKind::VARIABLE | SyntaxKind::DOLLAR | SyntaxKind::QUESTION => HighlightKind::Variable,
        SyntaxKind::ERROR | SyntaxKind::BANG => HighlightKind::Error,
        kind if is_operator(kind) => HighlightKind::Operator,
        _ => HighlightKind::Punctuation,
    }
}

fn is_builtin_type(text: &str) -> bool {
    const TYPES: &[&str] = &[
        "ARRAY",
        "BIGINT",
        "BINARY",
        "BOOLEAN",
        "CHAR",
        "DATE",
        "DATETIME",
        "DEC",
        "DECIMAL",
        "DOUBLE",
        "FLOAT",
        "GEOGRAPHY",
        "GEOMETRY",
        "INT",
        "INTEGER",
        "MAP",
        "NUMBER",
        "NUMERIC",
        "OBJECT",
        "REAL",
        "STRING",
        "TEXT",
        "TIME",
        "TIMESTAMP",
        "TIMESTAMP_LTZ",
        "TIMESTAMP_NTZ",
        "TIMESTAMP_TZ",
        "VARIANT",
        "VARCHAR",
        "VECTOR",
    ];
    TYPES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(text))
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

    /// Pull the alternation words out of a `(?i)\b(a|b|c)\b` grammar pattern.
    fn alternation(pattern: &str) -> Vec<String> {
        let inner = pattern
            .trim_start_matches("(?i)")
            .trim_start_matches("\\b(")
            .trim_end_matches("\\b")
            .trim_end_matches(')');
        inner.split('|').map(str::to_string).collect()
    }

    /// The committed TextMate grammar must not list any word the highlighter classifies
    /// differently — otherwise an editor using the grammar and one using the LSP/CST would
    /// disagree. We tie every keyword/type word in the grammar back to [`classify`].
    #[test]
    fn textmate_grammar_matches_the_highlighter() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../editors/textmate/snowflake.tmLanguage.json"
        );
        let grammar: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read grammar"))
                .expect("parse grammar");
        let repo = &grammar["repository"];

        let keywords = alternation(repo["keywords"]["match"].as_str().unwrap());
        assert!(keywords.len() > 100, "keyword set looks truncated");
        for word in &keywords {
            assert_eq!(
                classify(SyntaxKind::IDENT, word),
                HighlightKind::Keyword,
                "grammar lists `{word}` as a keyword but the highlighter disagrees"
            );
        }

        let types = alternation(repo["types"]["match"].as_str().unwrap());
        assert!(types.len() > 20, "type set looks truncated");
        for word in &types {
            assert_eq!(
                classify(SyntaxKind::IDENT, word),
                HighlightKind::Type,
                "grammar lists `{word}` as a type but the highlighter disagrees"
            );
        }
    }

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
}
