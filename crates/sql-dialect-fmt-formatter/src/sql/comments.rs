//! Comment attachment for SQL formatting.
//!
//! The formatter emits many punctuation tokens synthetically, so comments are attached to nearby
//! significant source tokens and consumed as those tokens are rendered. Any comment left unconsumed
//! makes the enclosing statement fall back to verbatim output.

use std::collections::HashMap;

use sql_dialect_fmt_syntax::{SyntaxKind::*, SyntaxNode, SyntaxToken};

/// A single comment, ready to render.
pub(super) struct CommentInfo {
    pub(super) text: String,
    /// A `--`/`//` line comment (must end its line) vs a `/* */` block comment (can sit inline).
    pub(super) is_line: bool,
    /// Line-level tool directives such as `-- noqa` and `-- sql-dialect-fmt:` must stay associated with
    /// the code line they annotate when the formatter synthesizes a statement terminator.
    pub(super) is_directive: bool,
}

/// Comments of one statement, keyed by the start offset of the significant token they attach to.
/// Entries are removed as they are emitted, so a non-empty map afterwards means something was
/// left unplaced.
#[derive(Default)]
pub(super) struct Comments {
    leading: HashMap<u32, Vec<CommentInfo>>,
    trailing: HashMap<u32, Vec<CommentInfo>>,
}

impl Comments {
    /// Walk the statement's tokens in order, assigning each comment to a significant token:
    /// trailing the previous token when on the same line, otherwise leading the next one.
    pub(super) fn build(stmt: &SyntaxNode) -> Self {
        let mut comments = Comments::default();
        let mut last_significant: Option<u32> = None;
        let mut newline_since = true; // statement start behaves like "on its own line"
        let mut pending_leading: Vec<CommentInfo> = Vec::new();

        for token in stmt
            .descendants_with_tokens()
            .filter_map(|el| el.into_token())
        {
            let kind = token.kind();
            if kind == NEWLINE {
                newline_since = true;
                continue;
            }
            if kind == WHITESPACE {
                continue;
            }
            if kind.is_comment() {
                let info = CommentInfo {
                    text: token.text().trim_end().to_string(),
                    is_line: kind == COMMENT,
                    is_directive: is_directive_comment(token.text()),
                };
                match last_significant {
                    Some(anchor) if !newline_since => {
                        comments.trailing.entry(anchor).or_default().push(info);
                    }
                    _ => pending_leading.push(info),
                }
                newline_since = false;
                continue;
            }
            // A comma is transparent: we synthesize list separators ourselves and never emit the
            // real comma token, so a comment written after one (`col, -- note`) belongs to the item
            // before it. Keep the anchor and pending leads pointed at the surrounding real tokens.
            if kind == COMMA {
                newline_since = false;
                continue;
            }
            // Closing delimiters are often re-synthesized by structural list formatters. A comment
            // immediately before one would otherwise attach to a token that is never emitted and
            // force verbatim fallback for the whole statement. Keep that trailing-list comment on
            // the previous real token instead.
            if matches!(kind, R_PAREN | R_BRACKET | R_BRACE) && !pending_leading.is_empty() {
                if let Some(anchor) = last_significant {
                    comments
                        .trailing
                        .entry(anchor)
                        .or_default()
                        .append(&mut pending_leading);
                }
            }

            // A significant token: it owns any pending leading comments and becomes the new anchor.
            let start = offset(&token);
            if !pending_leading.is_empty() {
                comments
                    .leading
                    .entry(start)
                    .or_default()
                    .append(&mut pending_leading);
            }
            last_significant = Some(start);
            newline_since = false;
        }

        // Comments with no following token become trailing of the last significant token.
        if !pending_leading.is_empty() {
            if let Some(anchor) = last_significant {
                comments
                    .trailing
                    .entry(anchor)
                    .or_default()
                    .append(&mut pending_leading);
            }
        }
        comments
    }

    pub(super) fn all_placed(&self) -> bool {
        self.leading.is_empty() && self.trailing.is_empty()
    }

    pub(super) fn take_leading(&mut self, token: &SyntaxToken) -> Vec<CommentInfo> {
        self.leading.remove(&offset(token)).unwrap_or_default()
    }

    pub(super) fn take_trailing(&mut self, token: &SyntaxToken) -> Vec<CommentInfo> {
        self.trailing.remove(&offset(token)).unwrap_or_default()
    }

    /// Pull comments attached to the final significant token out of the statement body. The source
    /// has no semicolon token there, but the formatter synthesizes one.
    pub(super) fn take_statement_end_comments(&mut self, stmt: &SyntaxNode) -> Vec<CommentInfo> {
        let last = stmt
            .descendants_with_tokens()
            .filter_map(|el| el.into_token())
            .filter(|t| !t.kind().is_trivia() && t.kind() != COMMA)
            .last();
        last.and_then(|token| self.trailing.remove(&offset(&token)))
            .unwrap_or_default()
    }
}

pub(super) fn directive_comment_same_line_after_stmt(
    last_stmt_end: Option<usize>,
    token: &SyntaxToken,
) -> bool {
    is_directive_comment(token.text())
        && last_stmt_end.is_some_and(|end| {
            let mut previous = token.prev_sibling_or_token();
            while let Some(element) = previous {
                let element_end: usize = element.text_range().end().into();
                if element_end <= end {
                    return true;
                }
                if element
                    .as_token()
                    .is_some_and(|token| token.text().contains(['\n', '\r']))
                {
                    return false;
                }
                previous = element.prev_sibling_or_token();
            }
            false
        })
}

fn offset(token: &SyntaxToken) -> u32 {
    token.text_range().start().into()
}

fn is_directive_comment(text: &str) -> bool {
    let Some(body) = text
        .trim_start()
        .strip_prefix("--")
        .or_else(|| text.trim_start().strip_prefix("//"))
    else {
        return false;
    };
    let lower = body.trim_start().to_ascii_lowercase();
    lower.starts_with("noqa")
        || lower.starts_with("sql-dialect-fmt:")
        || lower.starts_with("snowfmt:")
        || lower.starts_with("fmt:")
}
