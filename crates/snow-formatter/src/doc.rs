//! The formatter's intermediate representation: a Wadler/Prettier-style **Doc**.
//!
//! A `Doc` describes *intent* ("these items are a group; break them onto separate lines only if
//! they don't fit") rather than concrete characters. The [`crate::printer`] then resolves that
//! intent against a target line width to produce the final text. This indirection is what lets a
//! short `SELECT a, b` stay on one line while a long one explodes — something a token-stream
//! formatter cannot do.
//!
//! The lineage is Philip Wadler's *A prettier printer* → Prettier's `Doc` → `rome_formatter` →
//! `biome_formatter` / `ruff_formatter`. This is a deliberately small, SQL-agnostic subset of
//! that IR; richer elements (`line_suffix` for trailing comments, `if_break`, `best_fitting`)
//! will join it when comment attachment lands.

/// A node in the document tree. Construct these via the [`crate::builder`] helpers rather than by
/// hand, so intent stays readable at the call site.
#[derive(Clone, Debug)]
pub(crate) enum Doc {
    /// A run of literal text that contains no line break.
    Text(Box<str>),
    /// A potential line break, resolved by the printer according to the enclosing mode.
    Line(LineMode),
    /// An ordered sequence of docs, printed back to back.
    Concat(Vec<Doc>),
    /// Print the contained doc one indentation level deeper.
    Indent(Box<Doc>),
    /// A unit the printer tries to keep on one line ("flat"); if it doesn't fit the remaining
    /// width, every [`Line`](Doc::Line) directly inside flips to a real newline ("broken").
    Group(Group),
    /// Content deferred to the end of the current line — the mechanism for trailing comments,
    /// so `a, -- note` prints the comma first and the comment at the line's end.
    LineSuffix(Box<Doc>),
    /// Forces every enclosing group to break. A trailing line comment pairs a [`Doc::LineSuffix`]
    /// with this so the line actually ends after the comment instead of swallowing what follows.
    BreakParent,
}

/// A [`Doc::Group`] plus the break decision propagated into it before printing.
#[derive(Clone, Debug)]
pub(crate) struct Group {
    pub(crate) doc: Box<Doc>,
    /// Forced to `true` by [`crate::printer::propagate_breaks`] when the group contains a hard
    /// line (or a nested already-broken group), so the printer skips the width check for it.
    pub(crate) should_break: bool,
}

/// How a [`Doc::Line`] renders, depending on whether its enclosing group is flat or broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineMode {
    /// A space when flat, a newline when broken. (Prettier's `line`.)
    Space,
    /// Nothing when flat, a newline when broken. (Prettier's `softline`.)
    Soft,
    /// Always a newline, and forces every enclosing group to break. (Prettier's `hardline`.)
    Hard,
}
