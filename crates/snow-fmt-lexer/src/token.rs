//! The token types produced by the lexer.

use snow_fmt_syntax::SyntaxKind;

/// A single lexed token. `text` borrows the source, so the lexer allocates nothing per token.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Token<'a> {
    pub kind: SyntaxKind,
    pub text: &'a str,
}

/// A lexical diagnostic (e.g. an unterminated literal), located at a byte offset.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LexError {
    pub message: String,
    /// Byte offset into the source where the offending token began.
    pub offset: usize,
}

/// The result of lexing: the token stream plus any diagnostics.
#[derive(Clone, Debug, Default)]
pub struct Lexed<'a> {
    pub tokens: Vec<Token<'a>>,
    pub errors: Vec<LexError>,
}
