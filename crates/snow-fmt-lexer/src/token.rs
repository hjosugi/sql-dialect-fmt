//! The token types produced by the lexer.

use snow_fmt_syntax::SyntaxKind;

/// A single lexed token. `text` borrows the source, so the lexer allocates nothing per token.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Token<'a> {
    pub kind: SyntaxKind,
    pub text: &'a str,
}

/// A lexical diagnostic (e.g. an unterminated literal), located at a byte span.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LexError {
    pub message: String,
    /// Byte offset into the source where the offending token began.
    pub offset: usize,
    /// Byte length of the offending token's text, so a diagnostic can underline the whole token
    /// rather than a single character. May be `0` for a zero-width point at end of input.
    pub len: usize,
}

impl LexError {
    /// The byte range `offset..offset + len` this error covers in the source.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.offset..self.offset + self.len
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for LexError {}

/// The result of lexing: the token stream plus any diagnostics.
#[derive(Clone, Debug, Default)]
pub struct Lexed<'a> {
    pub tokens: Vec<Token<'a>>,
    pub errors: Vec<LexError>,
}
