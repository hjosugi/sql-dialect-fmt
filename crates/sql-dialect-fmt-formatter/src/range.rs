//! Statement-level range formatting.
//!
//! Whole-document formatting reflows everything; range formatting reflows only the **top-level
//! statements that intersect a caller-supplied byte range** and leaves the rest of the document
//! byte-for-byte unchanged. This mirrors editor "Format Selection" and an LSP `rangeFormatting`
//! request, and is built on the same lossless CST as [`crate::format`].
//!
//! The unit of work is a whole statement: a statement is reformatted if the range touches any of it.
//! The returned [`RangeEdit`] replaces the span from the first selected statement's first significant
//! token through the last selected statement's terminating semicolon, so leading blank lines and
//! same-line trailing comments around the selection are preserved exactly.

use std::ops::Range;

use sql_dialect_fmt_syntax::{SyntaxKind, SyntaxNode};

use crate::{format, FormatOptions};

/// A minimal edit produced by [`format_range`]: replace `range` (byte offsets into the original
/// source) with `new_text`. Everything outside `range` is unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeEdit {
    /// Byte offsets into the original source that should be replaced.
    pub range: Range<usize>,
    /// Replacement text for `range`.
    pub new_text: String,
}

/// Reformat only the top-level statements intersecting `target` (byte offsets into `source`),
/// leaving the rest of the document unchanged.
///
/// Returns `None` when nothing should change: the source cannot be parsed cleanly (same conservative
/// contract as [`crate::format`]), the range touches no statement, or the selection is already
/// formatted.
pub fn format_range(
    source: &str,
    target: Range<usize>,
    options: &FormatOptions,
) -> Option<RangeEdit> {
    // Only reflow input the parser fully accepts, exactly like whole-document formatting: a dirty
    // parse means we keep everything verbatim rather than risk mangling a fragmented tree.
    let parse = sql_dialect_fmt_parser::parse_with_dialect(source, options.dialect);
    if !parse.errors().is_empty() {
        return None;
    }

    let statements = collect_statements(&parse.syntax());
    let mut selected = statements
        .iter()
        .filter(|statement| statement.intersects(&target));
    let first = selected.next()?;
    let last = selected.next_back().unwrap_or(first);

    let block_range = first.sig_start..last.full_end;
    let block = source.get(block_range.clone())?;
    let formatted = format(block, options);
    // The block ends at a semicolon (or statement end), never a newline, so `format` always appends
    // one that the untouched tail already provides — drop it so a same-line trailing comment in the
    // tail stays on the statement's line instead of being pushed down.
    let new_text = strip_one_trailing_newline(&formatted);

    if new_text == block {
        return None;
    }
    Some(RangeEdit {
        range: block_range,
        new_text: new_text.to_string(),
    })
}

/// A top-level statement's significant span: from its first non-trivia token through its terminating
/// semicolon (or the node's end when it has none).
struct StatementSpan {
    sig_start: usize,
    full_end: usize,
}

impl StatementSpan {
    /// Whether `target` overlaps `[sig_start, full_end]` (inclusive, so a cursor at either edge or
    /// anywhere inside the statement counts).
    fn intersects(&self, target: &Range<usize>) -> bool {
        self.sig_start <= target.end && target.start <= self.full_end
    }
}

fn collect_statements(root: &SyntaxNode) -> Vec<StatementSpan> {
    let mut spans = Vec::new();
    // A statement node is followed by a root-level `;` token; pair them up as we walk in order.
    let mut pending: Option<(usize, usize)> = None; // (sig_start, node_end)
    for element in root.children_with_tokens() {
        if let Some(node) = element.as_node() {
            if let Some((sig_start, node_end)) = pending.take() {
                spans.push(StatementSpan {
                    sig_start,
                    full_end: node_end,
                });
            }
            let node_start = usize::from(node.text_range().start());
            let node_end = usize::from(node.text_range().end());
            pending = Some((significant_start(node).unwrap_or(node_start), node_end));
        } else if let Some(token) = element.as_token() {
            if token.kind() == SyntaxKind::SEMICOLON {
                if let Some((sig_start, _node_end)) = pending.take() {
                    spans.push(StatementSpan {
                        sig_start,
                        full_end: usize::from(token.text_range().end()),
                    });
                }
            }
        }
    }
    if let Some((sig_start, node_end)) = pending.take() {
        spans.push(StatementSpan {
            sig_start,
            full_end: node_end,
        });
    }
    spans
}

/// Byte offset of a statement node's first non-trivia token — past any leading whitespace, blank
/// lines, and comments, which stay verbatim in the untouched head.
fn significant_start(node: &SyntaxNode) -> Option<usize> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| usize::from(token.text_range().start()))
}

fn strip_one_trailing_newline(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(source: &str, edit: &RangeEdit) -> String {
        let mut out = String::new();
        out.push_str(&source[..edit.range.start]);
        out.push_str(&edit.new_text);
        out.push_str(&source[edit.range.end..]);
        out
    }

    fn range_format(source: &str, target: Range<usize>) -> Option<String> {
        format_range(source, target, &FormatOptions::default()).map(|edit| apply(source, &edit))
    }

    #[test]
    fn reformats_only_the_selected_statement() {
        let source = "select 1 ;\n\n\nselect a,b from t;\n\nSELECT 3;\n";
        // Target the middle statement only.
        let start = source.find("select a").unwrap();
        let out = range_format(source, start..start + 5).expect("edit");
        assert_eq!(out, "select 1 ;\n\n\nSELECT a, b\nFROM t;\n\nSELECT 3;\n");
        // The untouched statements are byte-identical.
        assert!(out.starts_with("select 1 ;\n\n\n"));
        assert!(out.ends_with("\n\nSELECT 3;\n"));
    }

    #[test]
    fn keeps_same_line_trailing_comment_attached() {
        let source = "select a,b from t;  -- keep me\nselect 2;\n";
        let out = range_format(source, 0..3).expect("edit");
        assert_eq!(out, "SELECT a, b\nFROM t;  -- keep me\nselect 2;\n");
    }

    #[test]
    fn full_range_matches_whole_document_format() {
        let source = "select 1;\nselect 2;\n";
        let out = range_format(source, 0..source.len()).expect("edit");
        assert_eq!(out, format(source, &FormatOptions::default()));
    }

    #[test]
    fn cursor_between_statements_makes_no_edit() {
        let source = "select 1;\n\nselect 2;\n";
        // Offset in the blank line between the two statements.
        let gap = source.find("\n\n").unwrap() + 1;
        assert_eq!(
            format_range(source, gap..gap, &FormatOptions::default()),
            None
        );
    }

    #[test]
    fn already_formatted_selection_makes_no_edit() {
        let source = "SELECT 1;\nSELECT a, b\nFROM t;\n";
        assert_eq!(format_range(source, 0..8, &FormatOptions::default()), None);
    }

    #[test]
    fn unparseable_source_makes_no_edit() {
        let source = "ALTER TABLE t ADD COLUMN c INT;\n";
        assert_eq!(format_range(source, 0..5, &FormatOptions::default()), None);
    }

    #[test]
    fn range_formatting_is_idempotent() {
        let source = "select 1 ;\n\nselect a,b from t;\n";
        let start = source.find("select a").unwrap();
        let once = range_format(source, start..start + 5).expect("edit");
        // Re-formatting the same statement in the already-formatted output is a no-op.
        let new_start = once.find("SELECT a").unwrap();
        assert_eq!(
            format_range(&once, new_start..new_start + 5, &FormatOptions::default()),
            None
        );
    }

    #[test]
    fn selection_spanning_two_statements_reformats_both() {
        let source = "select 1;select 2;\nSELECT 3;\n";
        let out = range_format(source, 0..12).expect("edit");
        assert_eq!(out, "SELECT 1;\nSELECT 2;\nSELECT 3;\n");
    }
}
