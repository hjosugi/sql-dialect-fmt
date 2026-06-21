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
pub use snow_fmt_syntax::{SyntaxKind, SyntaxNode};

/// A diagnostic produced while parsing, located at a byte offset into the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

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
pub fn parse(text: &str) -> Parse {
    let lexed = snow_fmt_lexer::tokenize(text);
    let input = input::Input::new(lexed);
    let (events, errors) = parser::Parser::new(&input).parse();
    let green = builder::build_tree(input.all(), events);
    Parse { green, errors }
}
