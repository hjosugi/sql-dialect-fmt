//! A small, self-contained pretty-printing engine in the Wadler → Prettier → rome/biome → ruff
//! lineage. The grammar-agnostic core is a `Doc` intermediate representation plus a width-aware
//! [`print`]er; SQL-specific rules live in [`crate::sql`] and only ever build `Doc`s.
//!
//! The model has three moving parts:
//! * **Builders** ([`text`], [`concat`], [`group`], [`indent`], [`line`], [`soft_line`],
//!   [`hard_line`], [`space`], [`join`]) construct the IR.
//! * **Groups** are the unit of layout choice: a group is printed *flat* (its soft lines become
//!   spaces or nothing) when it fits within the remaining width, otherwise it *breaks* (its soft
//!   lines become newlines + indentation).
//! * A **hard line** forces every group that contains it to break — the classic `breakParent`
//!   propagation — so caller-mandated line breaks are never silently collapsed.
//!
//! See `docs/research/prior-art.md` for the design rationale and references.

use std::borrow::Cow;

/// The pretty-printer intermediate representation.
///
/// Construct values through the builder functions rather than the variants directly; the shape is
/// public only so tests and future SQL rules can pattern-match if needed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Doc {
    /// Verbatim text. Must not contain newlines — use the line builders for those so width
    /// tracking and indentation stay correct.
    Text(Cow<'static, str>),
    /// A line break whose rendering depends on the enclosing group's mode (see [`LineKind`]).
    Line(LineKind),
    /// A sequence of documents laid out one after another.
    Concat(Vec<Doc>),
    /// A layout-choice boundary. Printed flat if it fits, otherwise broken. When `expand` is set
    /// the group always breaks (Prettier's `shouldBreak`) — used for "explode this collection"
    /// decisions like a magic trailing comma. Unlike a hard line, `expand` does **not** propagate
    /// to enclosing groups, so an inner collection can explode while its parent stays flat.
    Group { content: Box<Doc>, expand: bool },
    /// Increases the indentation level applied to line breaks inside it.
    Indent(Box<Doc>),
}

/// How a [`Doc::Line`] renders, as a function of the enclosing group's mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// Flat: a single space. Broken: a newline + indentation. (Prettier's `line`.)
    Space,
    /// Flat: nothing. Broken: a newline + indentation. (Prettier's `softline`.)
    Soft,
    /// Always a newline + indentation, and forces every enclosing group to break. (`hardline`.)
    Hard,
}

// ---- builders ----

/// Verbatim text (a borrowed `&'static str` or an owned `String`).
pub fn text(s: impl Into<Cow<'static, str>>) -> Doc {
    Doc::Text(s.into())
}

/// Lay out the parts one after another.
pub fn concat(parts: Vec<Doc>) -> Doc {
    Doc::Concat(parts)
}

/// A layout-choice group: flat when it fits on the line, broken otherwise.
pub fn group(inner: Doc) -> Doc {
    Doc::Group {
        content: Box::new(inner),
        expand: false,
    }
}

/// A group forced to break (its soft lines always become newlines), without forcing enclosing
/// groups to break. The building block for magic-trailing-comma "keep this exploded".
pub fn group_expanded(inner: Doc) -> Doc {
    Doc::Group {
        content: Box::new(inner),
        expand: true,
    }
}

/// Indent every line break that occurs inside `inner` by one more level.
pub fn indent(inner: Doc) -> Doc {
    Doc::Indent(Box::new(inner))
}

/// A space when flat, a newline when broken.
pub fn line() -> Doc {
    Doc::Line(LineKind::Space)
}

/// Nothing when flat, a newline when broken.
pub fn soft_line() -> Doc {
    Doc::Line(LineKind::Soft)
}

/// Always a newline; forces enclosing groups to break.
pub fn hard_line() -> Doc {
    Doc::Line(LineKind::Hard)
}

/// A literal, non-collapsible space.
pub fn space() -> Doc {
    Doc::Text(Cow::Borrowed(" "))
}

/// The empty document (renders to nothing).
pub fn empty() -> Doc {
    Doc::Concat(Vec::new())
}

/// Interleave `sep` between `items` (no separator before the first or after the last).
pub fn join(sep: Doc, items: Vec<Doc>) -> Doc {
    let mut parts = Vec::with_capacity(items.len().saturating_mul(2));
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            parts.push(sep.clone());
        }
        parts.push(item);
    }
    Doc::Concat(parts)
}

// ---- printing ----

/// Knobs for the printer. Opinionated by design: just a target width and an indent step.
#[derive(Clone, Copy, Debug)]
pub struct PrintOptions {
    /// The column the printer tries to keep lines within.
    pub line_width: usize,
    /// Number of spaces added per indentation level.
    pub indent_width: usize,
}

impl Default for PrintOptions {
    fn default() -> Self {
        PrintOptions {
            line_width: 100,
            indent_width: 4,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

#[derive(Clone, Copy)]
struct Cmd<'a> {
    indent: usize,
    mode: Mode,
    doc: &'a Doc,
}

/// Approximate display width of a string. Uses Unicode scalar count, which is exact for the ASCII
/// and most BMP text SQL uses; wide-CJK double-width handling can refine this later if needed.
fn text_width(s: &str) -> usize {
    s.chars().count()
}

/// Does `doc` contain a hard line anywhere within (propagating through nested groups, exactly like
/// Prettier's `breakParent`)? Such a document can never be printed flat.
fn has_forced_break(doc: &Doc) -> bool {
    match doc {
        Doc::Line(LineKind::Hard) => true,
        Doc::Line(_) | Doc::Text(_) => false,
        Doc::Concat(parts) => parts.iter().any(has_forced_break),
        Doc::Indent(inner) => has_forced_break(inner),
        // An exploded group propagates to ancestors: a multiline collection can't sit inline, so
        // every group containing it must break too (cf. Black's magic trailing comma).
        Doc::Group { content, expand } => *expand || has_forced_break(content),
    }
}

/// Would `next`, followed by the not-yet-processed `rest` of the print stack, fit on the current
/// line within `remaining` columns? Everything is measured as if flat until the first newline that
/// is actually taken (a hard line, or a soft line in an already-broken group).
fn fits(mut remaining: isize, rest: &[Cmd], next: Cmd, opts: &PrintOptions) -> bool {
    if remaining < 0 {
        return false;
    }
    let mut stack: Vec<Cmd> = vec![next];
    let mut rest_idx = rest.len();
    loop {
        let cmd = match stack.pop() {
            Some(cmd) => cmd,
            None => {
                if rest_idx == 0 {
                    return true;
                }
                rest_idx -= 1;
                rest[rest_idx]
            }
        };
        match cmd.doc {
            Doc::Text(s) => {
                remaining -= text_width(s) as isize;
                if remaining < 0 {
                    return false;
                }
            }
            Doc::Concat(parts) => {
                for part in parts.iter().rev() {
                    stack.push(Cmd { doc: part, ..cmd });
                }
            }
            Doc::Indent(inner) => stack.push(Cmd {
                indent: cmd.indent + opts.indent_width,
                doc: inner,
                mode: cmd.mode,
            }),
            Doc::Group { content, expand } => {
                let mode = if *expand || has_forced_break(content) {
                    Mode::Break
                } else {
                    Mode::Flat
                };
                stack.push(Cmd {
                    indent: cmd.indent,
                    mode,
                    doc: content,
                });
            }
            Doc::Line(kind) => match cmd.mode {
                // A newline is taken here, so everything up to it fit.
                Mode::Break => return true,
                Mode::Flat => match kind {
                    LineKind::Hard => return true,
                    LineKind::Soft => {}
                    LineKind::Space => {
                        remaining -= 1;
                        if remaining < 0 {
                            return false;
                        }
                    }
                },
            },
        }
    }
}

/// Render `doc` to a string. Trailing whitespace is trimmed from every line and the result ends
/// with exactly one newline (or is empty), so the output is stable under re-formatting.
pub fn print(doc: &Doc, opts: &PrintOptions) -> String {
    let mut out = String::new();
    let mut col = 0usize;
    let mut cmds: Vec<Cmd> = vec![Cmd {
        indent: 0,
        mode: Mode::Break,
        doc,
    }];

    while let Some(cmd) = cmds.pop() {
        match cmd.doc {
            Doc::Text(s) => {
                out.push_str(s);
                col += text_width(s);
            }
            Doc::Concat(parts) => {
                for part in parts.iter().rev() {
                    cmds.push(Cmd { doc: part, ..cmd });
                }
            }
            Doc::Indent(inner) => cmds.push(Cmd {
                indent: cmd.indent + opts.indent_width,
                doc: inner,
                mode: cmd.mode,
            }),
            Doc::Group { content, expand } => {
                let mode = if *expand || has_forced_break(content) {
                    Mode::Break
                } else if fits(
                    opts.line_width as isize - col as isize,
                    &cmds,
                    Cmd {
                        indent: cmd.indent,
                        mode: Mode::Flat,
                        doc: content,
                    },
                    opts,
                ) {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                cmds.push(Cmd {
                    indent: cmd.indent,
                    mode,
                    doc: content,
                });
            }
            Doc::Line(kind) => {
                let newline = match cmd.mode {
                    Mode::Break => true,
                    Mode::Flat => match kind {
                        LineKind::Hard => true,
                        LineKind::Space => {
                            out.push(' ');
                            col += 1;
                            false
                        }
                        LineKind::Soft => false,
                    },
                };
                if newline {
                    out.push('\n');
                    for _ in 0..cmd.indent {
                        out.push(' ');
                    }
                    col = cmd.indent;
                }
            }
        }
    }

    finalize(out)
}

/// Trim trailing whitespace from each line, drop leading/trailing blank lines, and ensure a single
/// trailing newline. This keeps output stable under re-formatting regardless of verbatim spans.
fn finalize(raw: String) -> String {
    let trimmed: Vec<&str> = raw.lines().map(|line| line.trim_end()).collect();
    let body = trimmed.join("\n");
    let body = body.trim_matches('\n');
    if body.is_empty() {
        String::new()
    } else {
        format!("{body}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(doc: &Doc, width: usize) -> String {
        print(
            doc,
            &PrintOptions {
                line_width: width,
                indent_width: 4,
            },
        )
    }

    #[test]
    fn text_and_concat() {
        let doc = concat(vec![text("a"), space(), text("b")]);
        assert_eq!(p(&doc, 80), "a b\n");
    }

    #[test]
    fn group_stays_flat_when_it_fits() {
        let doc = group(concat(vec![text("a"), line(), text("b")]));
        assert_eq!(p(&doc, 80), "a b\n");
    }

    #[test]
    fn group_breaks_when_too_wide() {
        let doc = group(concat(vec![
            text("aaaa"),
            indent(concat(vec![line(), text("bbbb")])),
        ]));
        assert_eq!(p(&doc, 5), "aaaa\n    bbbb\n");
    }

    #[test]
    fn hard_line_forces_enclosing_group_to_break() {
        let doc = group(concat(vec![text("a"), hard_line(), text("b")]));
        assert_eq!(p(&doc, 80), "a\nb\n");
    }

    #[test]
    fn soft_line_is_nothing_when_flat() {
        let doc = group(concat(vec![text("a"), soft_line(), text("b")]));
        assert_eq!(p(&doc, 80), "ab\n");
    }

    #[test]
    fn join_interleaves_separator() {
        let doc = join(text(", "), vec![text("a"), text("b"), text("c")]);
        assert_eq!(p(&doc, 80), "a, b, c\n");
    }

    #[test]
    fn nested_indent_accumulates() {
        let doc = concat(vec![
            text("a"),
            indent(concat(vec![
                hard_line(),
                text("b"),
                indent(concat(vec![hard_line(), text("c")])),
            ])),
        ]);
        assert_eq!(p(&doc, 80), "a\n    b\n        c\n");
    }

    #[test]
    fn expanded_group_breaks_even_when_it_fits() {
        let doc = group_expanded(concat(vec![
            text("("),
            indent(concat(vec![soft_line(), text("a")])),
            soft_line(),
            text(")"),
        ]));
        assert_eq!(p(&doc, 80), "(\n    a\n)\n");
    }

    #[test]
    fn expanded_inner_group_forces_outer_to_break() {
        // An exploded inner collection can't sit inline, so the outer group breaks too: the soft
        // line before the inner group becomes a newline.
        let inner = group_expanded(concat(vec![
            text("("),
            indent(concat(vec![soft_line(), text("x")])),
            soft_line(),
            text(")"),
        ]));
        let outer = group(concat(vec![text("a"), line(), inner]));
        assert_eq!(p(&outer, 80), "a\n(\n    x\n)\n");
    }

    #[test]
    fn empty_document_prints_nothing() {
        assert_eq!(p(&empty(), 80), "");
    }

    #[test]
    fn trailing_whitespace_is_trimmed() {
        // A broken group whose indented line is immediately followed by a hard line must not leave
        // spaces dangling on the blank line.
        let doc = concat(vec![text("a"), indent(hard_line()), hard_line(), text("b")]);
        assert_eq!(p(&doc, 80), "a\n\nb\n");
    }
}
