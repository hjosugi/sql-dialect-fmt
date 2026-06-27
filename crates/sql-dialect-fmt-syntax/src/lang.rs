//! `rowan` lossless-tree integration (compiled only with the `rowan` feature).
//!
//! The parser builds a green tree of [`SyntaxKind`]-tagged nodes/tokens; these aliases give
//! the rest of the toolchain a typed handle onto that tree.

use crate::SyntaxKind;

/// The language marker that ties [`SyntaxKind`] to the rowan tree.
///
/// `rowan::Language` requires `Ord`, so we derive the full ordering set even though this is an
/// uninhabited (field-less) marker type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnowflakeLang {}

impl rowan::Language for SnowflakeLang {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::from_u16(raw.0)
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind.to_u16())
    }
}

pub type SyntaxNode = rowan::SyntaxNode<SnowflakeLang>;
pub type SyntaxToken = rowan::SyntaxToken<SnowflakeLang>;
pub type SyntaxElement = rowan::SyntaxElement<SnowflakeLang>;
