//! Hand-written, lossless, error-resilient lexer for Snowflake SQL.
//!
//! Design goals (mirroring Biome / rust-analyzer):
//! * **Lossless** — every byte of the input belongs to exactly one token, so the original
//!   source can always be reconstructed by concatenating token texts. This is what makes
//!   accurate formatting, comment preservation, and syntax highlighting possible.
//! * **Error-resilient** — malformed input (unterminated strings/comments) still produces
//!   tokens plus diagnostics, never a panic. Mid-edit SQL must still lex.
//! * **Fast** — single pass over the bytes, zero allocation per token (texts borrow the
//!   input), no regex.
//!
//! The lexer is deliberately "dumb": it emits [`SyntaxKind::IDENT`] for every keyword-like
//! word. The parser reclassifies keywords contextually via [`snow_fmt_syntax::keyword_kind`],
//! because many SQL keywords (LEFT, ROW, VALUE, ...) are contextual and may be identifiers.
//!
//! ## Modules
//! * `token` — the [`Token`] / [`LexError`] / [`Lexed`] types.
//! * `lexer` — the single-pass tokenizer ([`tokenize`]).

mod lexer;
mod token;

pub use lexer::tokenize;
pub use token::{LexError, Lexed, Token};

// Re-exported so downstream crates and integration tests can name the kind through the lexer.
pub use snow_fmt_syntax::SyntaxKind;
