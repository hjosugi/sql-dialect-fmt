//! Error-resilient, event-based recursive-descent parser for Snowflake SQL.
//!
//! [`parse`] turns source text into a lossless `rowan` CST plus a list of [`ParseError`]s.
//! Parsing never fails: malformed input is wrapped in `ERROR` nodes and recovery continues, so
//! the tree always round-trips byte-for-byte (the basis for formatting and highlighting).
//!
//! ## Pipeline
//! `tokenize` → [`input::Input`] → [`parser::Parser`] (emits events) → [`builder::build_tree`].
//!
//! ## Modules
//! * `event` / `input` / `parser` / `grammar` / `builder` — the parsing pipeline.
//! * `ast` — typed accessors over the untyped tree.

mod ast;
mod builder;
mod event;
mod grammar;
mod input;
mod parser;

pub use ast::*;
pub use sql_dialect_fmt_syntax::{Dialect, SyntaxKind, SyntaxNode};

/// A diagnostic produced while parsing, located at a byte span into the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    /// Byte offset into the source where the offending token begins.
    pub offset: usize,
    /// Byte length of the offending token's text, so a diagnostic underlines the whole token
    /// rather than a single character. `0` for a zero-width point (e.g. an error at end of input,
    /// where there is no token to point at).
    pub len: usize,
}

impl ParseError {
    /// The byte range `offset..offset + len` this error covers in the source.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.offset..self.offset + self.len
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}

/// The result of parsing: a lossless green tree plus any diagnostics.
#[derive(Clone)]
pub struct Parse {
    green: rowan::GreenNode,
    errors: Vec<ParseError>,
}

impl Parse {
    /// The typed root of the syntax tree.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// Diagnostics gathered during parsing (empty for fully valid input).
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }
}

/// Parse Snowflake SQL source into a lossless CST. Never panics; never loses input.
///
/// Equivalent to [`parse_with_dialect`] with [`Dialect::Snowflake`].
pub fn parse(text: &str) -> Parse {
    parse_with_dialect(text, Dialect::Snowflake)
}

/// Parse `text` as `dialect` into a lossless CST. Never panics; never loses input.
///
/// The dialect drives tokenization (so the lexer's quoting/special-token rules match) and gates
/// which dialect-specific statements and operators the grammar accepts.
pub fn parse_with_dialect(text: &str, dialect: Dialect) -> Parse {
    let lexed = sql_dialect_fmt_lexer::tokenize_for_dialect(text, dialect);
    let input = input::Input::new(lexed);
    let (events, errors) = parser::Parser::new(&input, dialect).parse();
    let green = builder::build_tree(input.all(), events);
    Parse { green, errors }
}
