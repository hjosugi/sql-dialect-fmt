//! The token view the parser reads.
//!
//! The parser only cares about *meaningful* (non-trivia) tokens, but the tree builder needs
//! the *full* token list (with trivia) to stay lossless. This holds both, plus the byte offset
//! of every token so diagnostics can point at a location.

use snow_fmt_lexer::{Lexed, Token};
use snow_fmt_syntax::SyntaxKind;

pub(crate) struct Input<'a> {
    /// Every token, including trivia, in source order.
    all: Vec<Token<'a>>,
    /// Indices into `all` of the non-trivia tokens.
    meaningful: Vec<usize>,
    /// Byte offset of each token in `all`.
    offsets: Vec<usize>,
    /// Total source length in bytes (offset of EOF).
    total: usize,
}

impl<'a> Input<'a> {
    pub(crate) fn new(lexed: Lexed<'a>) -> Self {
        let all = lexed.tokens;
        let mut offsets = Vec::with_capacity(all.len());
        let mut meaningful = Vec::new();
        let mut off = 0usize;
        for (i, t) in all.iter().enumerate() {
            offsets.push(off);
            off += t.text.len();
            if !t.kind.is_trivia() {
                meaningful.push(i);
            }
        }
        Input {
            all,
            meaningful,
            offsets,
            total: off,
        }
    }

    pub(crate) fn all(&self) -> &[Token<'a>] {
        &self.all
    }

    /// Number of meaningful tokens.
    pub(crate) fn len(&self) -> usize {
        self.meaningful.len()
    }

    /// Raw kind of the meaningful token at `pos`, or [`SyntaxKind::EOF`] past the end.
    pub(crate) fn kind(&self, pos: usize) -> SyntaxKind {
        match self.meaningful.get(pos) {
            Some(&i) => self.all[i].kind,
            None => SyntaxKind::EOF,
        }
    }

    /// Text of the meaningful token at `pos`, or `""` past the end.
    pub(crate) fn text(&self, pos: usize) -> &'a str {
        match self.meaningful.get(pos) {
            Some(&i) => self.all[i].text,
            None => "",
        }
    }

    /// Byte offset of the meaningful token at `pos`, or the source length past the end.
    pub(crate) fn offset(&self, pos: usize) -> usize {
        match self.meaningful.get(pos) {
            Some(&i) => self.offsets[i],
            None => self.total,
        }
    }
}
