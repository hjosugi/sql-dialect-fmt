//! The width-aware printer: turn a [`Doc`] into text by resolving each group to flat or broken.
//!
//! This is the Wadler/Prettier algorithm as adopted by `biome_formatter` / `ruff_formatter`:
//!
//! 1. [`propagate_breaks`] walks the tree once and marks every group that *must* break (because
//!    it transitively contains a hard line). A group's forced break also propagates to its
//!    ancestors — a hard line anywhere makes all enclosing groups multi-line.
//! 2. The main loop in [`print`] processes a stack of `(indent, mode, doc)` commands. On reaching
//!    a group it asks [`Printer::fits`] whether the group's flat rendering plus the rest of the
//!    current line fits in the remaining width; if so it prints flat, otherwise broken.
//!
//! Width is measured in Unicode scalar values (`chars().count()`). That treats a CJK character as
//! width 1; East-Asian-width / grapheme accuracy can be layered on later without touching callers.

use crate::doc::{Doc, LineMode};
use crate::FormatOptions;

/// Whether a doc is being rendered on one line (`Flat`) or split across lines (`Break`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Flat,
    Break,
}

/// One unit of pending work for the printer: a doc to render at a given indent level and mode.
#[derive(Clone, Copy)]
struct Cmd<'a> {
    indent: usize,
    mode: Mode,
    doc: &'a Doc,
}

/// Render `doc` to a string under `opts`. Takes the doc by value because [`propagate_breaks`]
/// annotates it in place first.
pub(crate) fn print(mut doc: Doc, opts: &FormatOptions) -> String {
    propagate_breaks(&mut doc);
    Printer {
        width: opts.line_width,
        indent_width: opts.indent_width,
    }
    .run(&doc)
}

/// Mark groups that must break (they contain a hard line, directly or via a nested broken group).
///
/// Returns whether `doc` contains a forced break, so a parent group learns to break too. The loop
/// over a `Concat` deliberately does not short-circuit: every nested group must be visited and
/// annotated, not just enough to prove the parent breaks.
pub(crate) fn propagate_breaks(doc: &mut Doc) -> bool {
    match doc {
        Doc::Text(_) => false,
        Doc::Line(LineMode::Hard) => true,
        Doc::Line(_) => false,
        Doc::Concat(items) => {
            let mut forced = false;
            for item in items.iter_mut() {
                if propagate_breaks(item) {
                    forced = true;
                }
            }
            forced
        }
        Doc::Indent(inner) => propagate_breaks(inner),
        Doc::LineSuffix(inner) => {
            // A line suffix's own content (a comment) doesn't force its container to break; the
            // paired BreakParent does that. Still recurse so any nested groups get annotated.
            propagate_breaks(inner);
            false
        }
        Doc::BreakParent => true,
        Doc::Group(group) => {
            if propagate_breaks(&mut group.doc) {
                group.should_break = true;
            }
            group.should_break
        }
    }
}

struct Printer {
    width: usize,
    indent_width: usize,
}

impl Printer {
    fn run(&self, root: &Doc) -> String {
        let mut out = String::new();
        let mut pos = 0usize; // current column, in chars
        let mut cmds: Vec<Cmd> = vec![Cmd {
            indent: 0,
            mode: Mode::Break,
            doc: root,
        }];
        // Deferred trailing content (comments), flushed just before the next real line break.
        let mut line_suffixes: Vec<Cmd> = Vec::new();

        loop {
            while let Some(cmd) = cmds.pop() {
                match cmd.doc {
                    Doc::Text(s) => {
                        out.push_str(s);
                        pos += char_width(s);
                    }
                    Doc::Concat(items) => {
                        for item in items.iter().rev() {
                            cmds.push(Cmd { doc: item, ..cmd });
                        }
                    }
                    Doc::Indent(inner) => cmds.push(Cmd {
                        indent: cmd.indent + 1,
                        doc: inner,
                        ..cmd
                    }),
                    Doc::LineSuffix(inner) => line_suffixes.push(Cmd { doc: inner, ..cmd }),
                    Doc::BreakParent => {}
                    Doc::Line(mode) => {
                        let do_break = matches!(mode, LineMode::Hard) || cmd.mode == Mode::Break;
                        if do_break {
                            // Flush any pending suffixes before the break: re-queue this line, then
                            // the suffixes (reversed so they print in insertion order).
                            if !line_suffixes.is_empty() {
                                cmds.push(cmd);
                                while let Some(suffix) = line_suffixes.pop() {
                                    cmds.push(suffix);
                                }
                                continue;
                            }
                            trim_trailing_spaces(&mut out);
                            out.push('\n');
                            let spaces = cmd.indent * self.indent_width;
                            out.extend(std::iter::repeat_n(' ', spaces));
                            pos = spaces;
                        } else if matches!(mode, LineMode::Space) {
                            out.push(' ');
                            pos += 1;
                        }
                        // A soft line in flat mode renders as nothing.
                    }
                    Doc::Group(group) => {
                        let remaining = self.width as isize - pos as isize;
                        let mode = if group.should_break
                            || !self.fits(&group.doc, cmd.indent, &cmds, remaining)
                        {
                            Mode::Break
                        } else {
                            Mode::Flat
                        };
                        cmds.push(Cmd {
                            mode,
                            doc: &group.doc,
                            ..cmd
                        });
                    }
                }
            }
            // Flush trailing suffixes left at end of input (e.g. a comment at EOF).
            if line_suffixes.is_empty() {
                break;
            }
            while let Some(suffix) = line_suffixes.pop() {
                cmds.push(suffix);
            }
        }

        out
    }

    /// Would `next` (rendered flat) plus the rest of the current line fit in `remaining` columns?
    ///
    /// Simulates printing forward until the first line break that *ends* the current line (a
    /// break-mode line, or a hard line): reaching one means everything so far fit. `rest` is the
    /// printer's pending stack, consumed from its top (its end) once `next`'s own content is
    /// exhausted, so trailing content on the same line is counted too.
    fn fits(&self, next: &Doc, indent: usize, rest: &[Cmd], mut remaining: isize) -> bool {
        let mut local: Vec<Cmd> = vec![Cmd {
            indent,
            mode: Mode::Flat,
            doc: next,
        }];
        let mut rest_idx = rest.len();

        loop {
            if remaining < 0 {
                return false;
            }
            let cmd = if let Some(cmd) = local.pop() {
                cmd
            } else if rest_idx == 0 {
                return true; // ran out of content before overflowing
            } else {
                rest_idx -= 1;
                rest[rest_idx]
            };

            match cmd.doc {
                Doc::Text(s) => remaining -= char_width(s) as isize,
                Doc::Concat(items) => {
                    for item in items.iter().rev() {
                        local.push(Cmd { doc: item, ..cmd });
                    }
                }
                Doc::Indent(inner) => local.push(Cmd { doc: inner, ..cmd }),
                // Deferred to line end / handled by break propagation — no width here.
                Doc::LineSuffix(_) | Doc::BreakParent => {}
                Doc::Line(mode) => match cmd.mode {
                    Mode::Flat => match mode {
                        LineMode::Space => remaining -= 1,
                        LineMode::Soft => {}
                        LineMode::Hard => return true, // line ends here ⇒ fit so far
                    },
                    Mode::Break => return true,
                },
                Doc::Group(group) => {
                    let mode = if group.should_break {
                        Mode::Break
                    } else {
                        cmd.mode
                    };
                    local.push(Cmd {
                        mode,
                        doc: &group.doc,
                        ..cmd
                    });
                }
            }
        }
    }
}

#[inline]
fn char_width(s: &str) -> usize {
    s.chars().count()
}

fn trim_trailing_spaces(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{concat, group, hard_line, indent, line, soft_line, text};

    fn opts(width: usize) -> FormatOptions {
        FormatOptions {
            line_width: width,
            indent_width: 2,
            ..FormatOptions::default()
        }
    }

    fn print_doc(doc: Doc, width: usize) -> String {
        print(doc, &opts(width))
    }

    #[test]
    fn flat_group_fits_on_one_line() {
        let doc = group(concat(vec![
            text("SELECT"),
            indent(concat(vec![
                line(),
                text("a"),
                text(","),
                line(),
                text("b"),
            ])),
        ]));
        assert_eq!(print_doc(doc, 80), "SELECT a, b");
    }

    #[test]
    fn group_breaks_when_too_wide() {
        let doc = group(concat(vec![
            text("SELECT"),
            indent(concat(vec![
                line(),
                text("a"),
                text(","),
                line(),
                text("b"),
            ])),
        ]));
        // Width 6 cannot hold "SELECT a, b" → the group breaks and indents each line.
        assert_eq!(print_doc(doc, 6), "SELECT\n  a,\n  b");
    }

    #[test]
    fn hard_line_forces_break_regardless_of_width() {
        let doc = group(concat(vec![text("a"), hard_line(), text("b")]));
        assert_eq!(print_doc(doc, 80), "a\nb");
    }

    #[test]
    fn hard_line_inside_inner_group_breaks_outer_group() {
        // The inner group has a hard line, so propagation must break the outer group's `line()`.
        let inner = group(concat(vec![text("("), hard_line(), text(")")]));
        let outer = group(concat(vec![text("x"), line(), inner]));
        assert_eq!(print_doc(outer, 80), "x\n(\n)");
    }

    #[test]
    fn soft_line_is_empty_when_flat() {
        let doc = group(concat(vec![
            text("("),
            soft_line(),
            text("a"),
            soft_line(),
            text(")"),
        ]));
        assert_eq!(print_doc(doc, 80), "(a)");
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
        assert_eq!(print_doc(doc, 80), "a\n  b\n    c");
    }

    #[test]
    fn trailing_spaces_are_trimmed_at_breaks() {
        // "a" + space-line (broken) must not leave "a " before the newline.
        let doc = group(concat(vec![text("a"), hard_line(), text("b")]));
        let out = print_doc(doc, 80);
        assert!(
            !out.contains(" \n"),
            "no trailing space before newline: {out:?}"
        );
    }
}
