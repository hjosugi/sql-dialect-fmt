//! Lexical syntax highlighting for Snowflake SQL.
//!
//! This layer intentionally starts from the lossless lexer. It is stable under grammar growth:
//! new keywords are highlighted by `keyword_kind`, while newly-added punctuation only needs a
//! `SyntaxKind` classification here before LSP/TextMate adapters consume it.

use snow_fmt_lexer::{tokenize, LexError};
use snow_fmt_syntax::{keyword_kind, SyntaxKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Highlighted<'a> {
    pub tokens: Vec<HighlightToken<'a>>,
    /// Embedded-language regions (e.g. a JavaScript UDF body inside `$$ … $$`). An editor or a
    /// tree-sitter `injections.scm` highlights each region with the named language's grammar.
    pub injections: Vec<Injection>,
    pub errors: Vec<LexError>,
}

/// A span of the source written in another language, to be highlighted by that language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Injection {
    /// The language, lower-cased (e.g. `"javascript"`), taken from the `LANGUAGE` clause.
    pub language: String,
    /// Byte range of the embedded body — the text *between* the `$$` delimiters.
    pub range: std::ops::Range<usize>,
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
    let mut tokens = Vec::with_capacity(lexed.tokens.len());
    let mut injections = Vec::new();

    // Track the most recent `LANGUAGE <name>` so a following `$$ … $$` body can be tagged as an
    // injection. Reset at a statement boundary (`;`) so one statement's LANGUAGE never leaks.
    let mut offset = 0usize;
    let mut current_language: Option<String> = None;
    let mut expect_language_name = false;

    for token in lexed.tokens {
        let start = offset;
        offset += token.text.len();

        if !token.kind.is_trivia() {
            if expect_language_name {
                current_language = Some(token.text.to_ascii_lowercase());
                expect_language_name = false;
            } else if token.kind == SyntaxKind::SEMICOLON {
                current_language = None;
            } else if token.kind == SyntaxKind::IDENT && token.text.eq_ignore_ascii_case("language")
            {
                expect_language_name = true;
            }

            if token.kind == SyntaxKind::DOLLAR_STRING {
                if let (Some(language), Some(range)) =
                    (&current_language, dollar_body_range(token.text, start))
                {
                    injections.push(Injection {
                        language: language.clone(),
                        range,
                    });
                }
            }
        }

        tokens.push(HighlightToken {
            kind: classify(token.kind, token.text),
            text: token.text,
            range: start..offset,
        });
    }

    Highlighted {
        tokens,
        injections,
        errors: lexed.errors,
    }
}

/// Byte range of a `$$ … $$` body's interior, given the token text and its start offset.
fn dollar_body_range(text: &str, start: usize) -> Option<std::ops::Range<usize>> {
    if text.len() >= 4 && text.starts_with("$$") && text.ends_with("$$") {
        Some((start + 2)..(start + text.len() - 2))
    } else {
        None
    }
}

pub fn classify(kind: SyntaxKind, text: &str) -> HighlightKind {
    if kind.is_trivia() {
        return match kind {
            SyntaxKind::COMMENT | SyntaxKind::BLOCK_COMMENT => HighlightKind::Comment,
            _ => HighlightKind::Whitespace,
        };
    }

    if kind.is_keyword() || (kind == SyntaxKind::IDENT && keyword_kind(text).is_some()) {
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
    fn javascript_udf_body_is_a_javascript_injection() {
        let sql = "CREATE FUNCTION f() RETURNS INT LANGUAGE JAVASCRIPT AS $$ return 1; $$";
        let h = highlight(sql);
        assert_eq!(
            h.injections.len(),
            1,
            "expected one injection: {:?}",
            h.injections
        );
        let inj = &h.injections[0];
        assert_eq!(inj.language, "javascript");
        // The injected range is the body between the `$$` delimiters.
        assert_eq!(&sql[inj.range.clone()], " return 1; ");
    }

    #[test]
    fn dollar_body_without_language_is_not_injected() {
        // No LANGUAGE clause → the `$$` body is just a dollar-quoted string, not an injection.
        let h = highlight("SELECT $$ raw text $$");
        assert!(h.injections.is_empty());
    }

    #[test]
    fn language_does_not_leak_across_statements() {
        // The JS language from the first statement must not tag the second statement's body.
        let sql = "CREATE FUNCTION a() RETURNS INT LANGUAGE JAVASCRIPT AS $$ 1 $$; SELECT $$ x $$";
        let h = highlight(sql);
        assert_eq!(h.injections.len(), 1);
        assert_eq!(h.injections[0].language, "javascript");
    }
}
