//! The flat event stream the parser emits.
//!
//! The grammar never touches the tree directly: it pushes [`Event`]s, and a separate pass
//! ([`crate::builder::build_tree`]) replays them into a `rowan` `GreenNodeBuilder`, re-inserting
//! trivia (whitespace/comments) so the resulting CST is byte-exact-lossless.

use sql_dialect_fmt_syntax::SyntaxKind;

pub(crate) enum Event {
    /// Open a node. The kind is a placeholder ([`SyntaxKind::ERROR`]) until the matching
    /// `Marker` is completed, at which point it is overwritten with the real kind.
    Open {
        kind: SyntaxKind,
        /// Distance to a later `Open` event that should wrap this node. This keeps
        /// `CompletedMarker::precede` O(1): the parser appends the parent and the
        /// builder follows this link when replaying the child open.
        forward_parent: Option<usize>,
    },
    /// Close the most recently opened, still-open node.
    Close,
    /// Consume the next meaningful (non-trivia) token, tagging it with `kind` (which may be a
    /// keyword kind remapped from a raw `IDENT`).
    Advance { kind: SyntaxKind },
    /// An abandoned `Open`: the builder skips it, leaving any children attached to the parent. Used
    /// for speculative wrappers that turn out not to be needed (see [`crate::parser::Marker::abandon`]).
    Tombstone,
}
