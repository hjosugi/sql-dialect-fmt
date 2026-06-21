//! Ergonomic constructors for [`Doc`]. Keeping these tiny and named after their *intent*
//! (`group`, `indent`, `line`, `soft_line`, `hard_line`) lets the SQL rules in [`crate::sql`]
//! read close to the layout they describe.

use crate::doc::{Doc, Group, LineMode};

/// Literal text. Must not contain a newline — use a line break doc for that.
pub(crate) fn text(s: impl Into<Box<str>>) -> Doc {
    Doc::Text(s.into())
}

/// The empty document. Handy as a "no-op" branch (e.g. an absent optional clause).
pub(crate) fn nil() -> Doc {
    Doc::Concat(Vec::new())
}

/// Concatenate docs, printed one after another.
pub(crate) fn concat(parts: Vec<Doc>) -> Doc {
    Doc::Concat(parts)
}

/// A group the printer keeps flat if it fits, otherwise breaks. Groups nest: an outer group can
/// break while an inner one stays flat.
pub(crate) fn group(doc: Doc) -> Doc {
    Doc::Group(Group {
        doc: Box::new(doc),
        should_break: false,
    })
}

/// Indent the contained doc by one level (the width is set by [`crate::FormatOptions`]).
pub(crate) fn indent(doc: Doc) -> Doc {
    Doc::Indent(Box::new(doc))
}

/// A space when flat, a newline when the enclosing group breaks.
pub(crate) fn line() -> Doc {
    Doc::Line(LineMode::Space)
}

/// Nothing when flat, a newline when the enclosing group breaks.
pub(crate) fn soft_line() -> Doc {
    Doc::Line(LineMode::Soft)
}

/// Always a newline; also forces every enclosing group to break.
pub(crate) fn hard_line() -> Doc {
    Doc::Line(LineMode::Hard)
}

/// Defer `doc` to the end of the current line (used for trailing comments).
pub(crate) fn line_suffix(doc: Doc) -> Doc {
    Doc::LineSuffix(Box::new(doc))
}

/// Force every enclosing group to break.
pub(crate) fn break_parent() -> Doc {
    Doc::BreakParent
}

/// Interleave `items` with `sep`. Empty in → empty doc; single item → that item, no separator.
pub(crate) fn join(sep: Doc, items: Vec<Doc>) -> Doc {
    let mut parts = Vec::with_capacity(items.len().saturating_mul(2).saturating_sub(1));
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            parts.push(sep.clone());
        }
        parts.push(item);
    }
    Doc::Concat(parts)
}
