//! Comment attachment: decide, for each comment in the tree, which node it belongs to and how.
//!
//! The formatter regenerates all whitespace, so comments can't just ride along in the token
//! stream — each must be re-anchored to a node and re-emitted in the right place. This is the
//! Prettier/Ruff model: per-comment, find the **enclosing** node and the **preceding** /
//! **following** child nodes around it, then bucket it as:
//!
//! * **leading** of the following node — an own-line comment that should print above it;
//! * **trailing** of the preceding node — an end-of-line comment that should print after it
//!   (via a line suffix), or, if it sits on its own line with nothing following, on its own line;
//! * **dangling** of the enclosing node — a comment with neither neighbor (e.g. inside an
//!   otherwise-empty construct).
//!
//! Attaching to *nodes* (not raw tokens) keeps the number of positions small and the result
//! idempotent, exactly as Ruff notes for its own formatter.

use std::collections::HashMap;

use rowan::TextRange;
use snow_fmt_syntax::{SyntaxNode, SyntaxToken};

/// A trailing comment plus whether it stood on its own line (no token before it on that line).
pub(crate) struct Trailing {
    pub(crate) text: String,
    pub(crate) own_line: bool,
}

/// Comment attachments for one parse tree, keyed by the node each comment belongs to.
#[derive(Default)]
pub(crate) struct Comments {
    leading: HashMap<SyntaxNode, Vec<String>>,
    trailing: HashMap<SyntaxNode, Vec<Trailing>>,
    dangling: HashMap<SyntaxNode, Vec<String>>,
}

impl Comments {
    /// Attach every comment token in `root` to a node. `src` is the original source, used only to
    /// tell own-line comments from end-of-line ones.
    pub(crate) fn build(root: &SyntaxNode, src: &str) -> Comments {
        let mut comments = Comments::default();
        for token in root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if !token.kind().is_comment() {
                continue;
            }
            let range = token.text_range();
            let (enclosing, preceding, following) = locate(root, range);
            let own_line = newline_before(src, u32::from(range.start()) as usize);
            let text = token.text().to_string();

            if let Some(prev) = preceding {
                if !own_line {
                    push_trailing(&mut comments, prev, text, false);
                } else if let Some(next) = following {
                    comments.leading.entry(next).or_default().push(text);
                } else {
                    push_trailing(&mut comments, prev, text, true);
                }
            } else if let Some(next) = following {
                comments.leading.entry(next).or_default().push(text);
            } else {
                comments.dangling.entry(enclosing).or_default().push(text);
            }
        }
        comments
    }

    pub(crate) fn leading(&self, node: &SyntaxNode) -> &[String] {
        self.leading.get(node).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn trailing(&self, node: &SyntaxNode) -> &[Trailing] {
        self.trailing.get(node).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn dangling(&self, node: &SyntaxNode) -> &[String] {
        self.dangling.get(node).map_or(&[], Vec::as_slice)
    }
}

fn push_trailing(comments: &mut Comments, node: SyntaxNode, text: String, own_line: bool) {
    comments
        .trailing
        .entry(node)
        .or_default()
        .push(Trailing { text, own_line });
}

/// Descend into the deepest node whose child nodes straddle `range`, returning that enclosing node
/// together with the child node ending before the comment and the child node starting after it.
///
/// Child positions use each node's **meaningful** span (first to last non-trivia token), not its
/// full `text_range`. A node's `text_range` includes its leading trivia, so a comment that is
/// really *between* two siblings would otherwise look like it lives inside the following node and
/// get mis-attached deep in that subtree.
fn locate(
    node: &SyntaxNode,
    range: TextRange,
) -> (SyntaxNode, Option<SyntaxNode>, Option<SyntaxNode>) {
    let mut preceding = None;
    let mut following = None;
    for child in node.children() {
        let Some(cr) = meaningful_range(&child) else {
            continue; // a node with no meaningful tokens can't anchor a comment
        };
        if cr.start() <= range.start() && range.end() <= cr.end() {
            return locate(&child, range); // comment lives inside this child → recurse
        } else if cr.end() <= range.start() {
            preceding = Some(child); // keep the latest such child
        } else if range.end() <= cr.start() && following.is_none() {
            following = Some(child); // first child starting after the comment
        }
    }
    (node.clone(), preceding, following)
}

/// The span from a node's first non-trivia token to its last, or `None` if it has none.
fn meaningful_range(node: &SyntaxNode) -> Option<TextRange> {
    let mut tokens = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t: &SyntaxToken| !t.kind().is_trivia());
    let first = tokens.next()?;
    let start = first.text_range().start();
    let end = tokens
        .last()
        .map_or_else(|| first.text_range().end(), |t| t.text_range().end());
    Some(TextRange::new(start, end))
}

/// Is there a line break between the comment at `offset` and the previous non-blank character?
/// (Start of file counts as own-line.)
fn newline_before(src: &str, offset: usize) -> bool {
    let bytes = src.as_bytes();
    let mut i = offset.min(bytes.len());
    while i > 0 && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
        i -= 1;
    }
    i == 0 || bytes[i - 1] == b'\n' || bytes[i - 1] == b'\r'
}
