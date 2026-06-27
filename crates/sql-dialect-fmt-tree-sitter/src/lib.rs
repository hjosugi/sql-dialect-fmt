//! Rust bindings for the bundled Tree-sitter Snowflake SQL grammar.
//!
//! The formatter/parser crates keep using the lossless rowan CST as their
//! source of truth. This crate exposes the generated Tree-sitter language for
//! editor integrations that need fast incremental parsing and query-based
//! highlighting.

use tree_sitter::{Language, LanguageError, Parser};
use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_snowflake() -> *const ();
}

/// The Tree-sitter language function for Snowflake SQL.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_snowflake) };

/// The generated node-types metadata for editor integrations.
pub const NODE_TYPES: &str = include_str!("../../../tree-sitter-snowflake/src/node-types.json");

/// Highlight query compatible with Tree-sitter's standard highlight engine.
pub const HIGHLIGHTS_QUERY: &str =
    include_str!("../../../tree-sitter-snowflake/queries/highlights.scm");

/// Locals query for editor reference/hover plumbing.
pub const LOCALS_QUERY: &str = include_str!("../../../tree-sitter-snowflake/queries/locals.scm");

/// Injection query placeholder. Language-aware injections are handled above the token grammar.
pub const INJECTIONS_QUERY: &str =
    include_str!("../../../tree-sitter-snowflake/queries/injections.scm");

/// Folding query: collapses each `statement` node (and block comments), mirroring the LSP server's
/// `textDocument/foldingRange`.
pub const FOLDS_QUERY: &str = include_str!("../../../tree-sitter-snowflake/queries/folds.scm");

/// Indentation query for editors that consume Tree-sitter `@indent` / `@dedent` captures.
pub const INDENTS_QUERY: &str = include_str!("../../../tree-sitter-snowflake/queries/indents.scm");

/// Construct an owned [`Language`] from the grammar function.
pub fn language() -> Language {
    LANGUAGE.into()
}

/// Construct a parser with the Snowflake language already loaded.
pub fn parser() -> Result<Parser, LanguageError> {
    let mut parser = Parser::new();
    parser.set_language(&language())?;
    Ok(parser)
}
