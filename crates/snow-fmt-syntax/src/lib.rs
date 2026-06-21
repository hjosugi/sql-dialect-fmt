#![allow(non_camel_case_types)]
//! Syntax kinds for **snow-fmt**, a Snowflake SQL formatter + highlighter toolchain.
//!
//! [`SyntaxKind`] is the single enumeration of every token kind and (eventually) every
//! node kind in the grammar. It is `#[repr(u16)]` and contiguous so it can be cheaply
//! converted to/from the `u16` that the `rowan` lossless tree stores.
//!
//! The lexer produces only *token* kinds; the parser later introduces *node* kinds and
//! assembles the lossless concrete syntax tree (CST). Keeping kinds in one crate lets the
//! lexer, parser, formatter and highlighter all speak the same vocabulary.
//!
//! ## Modules
//! * `kind` — the [`SyntaxKind`] enum plus its `u16` conversions and predicates.
//! * `keyword` — case-insensitive recognition of keyword text ([`keyword_kind`]).
//! * `lang` — `rowan` lossless-tree integration, behind the `rowan` feature.

#[macro_use]
mod macros;

mod keyword;
mod kind;
#[cfg(feature = "rowan")]
mod lang;

pub use keyword::keyword_kind;
pub use kind::SyntaxKind;

#[cfg(feature = "rowan")]
pub use lang::{SnowflakeLang, SyntaxElement, SyntaxNode, SyntaxToken};
