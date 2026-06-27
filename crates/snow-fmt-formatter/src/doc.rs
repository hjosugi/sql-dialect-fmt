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
    /// tracking and indentation stay correct. The display width is folded in at construction time
    /// (via [`text`]) so the fit/break measurement never re-scans the string.
    Text(Cow<'static, str>, usize),
    /// A line break whose rendering depends on the enclosing group's mode (see [`LineKind`]).
    Line(LineKind),
    /// A sequence of documents laid out one after another.
    Concat(Vec<Doc>),
    /// Try candidate layouts in order and print the first one that fits on the current line; if no
    /// candidate fits, print the last one. This is the small Doc-IR escape hatch used by Prettier /
    /// Biome for constructs with genuinely different layouts rather than just flat-vs-break lines.
    BestFitting(Vec<Doc>),
    /// A layout-choice boundary. Printed flat if it fits, otherwise broken. When `expand` is set
    /// the group always breaks (Prettier's `shouldBreak`) — used for "explode this collection"
    /// decisions like a magic trailing comma. Unlike a hard line, `expand` does **not** propagate
    /// to enclosing groups, so an inner collection can explode while its parent stays flat.
    ///
    /// `must_break` is `expand || has_forced_break(content)`, computed once when the group is built
    /// (see [`group`]). Because groups are constructed bottom-up, each group carries its own
    /// answer and ancestors read it in O(1) instead of re-walking the whole subtree on every
    /// fit/break decision.
    Group {
        content: Box<Doc>,
        expand: bool,
        must_break: bool,
    },
    /// Increases the indentation level applied to line breaks inside it.
    Indent(Box<Doc>),
    /// Content deferred to just before the next newline (or the document's end). The vehicle for
    /// trailing line comments, which must not have code emitted after them on the same line.
    LineSuffix(Box<Doc>),
    /// A zero-width marker that forces every enclosing group to break, without itself emitting a
    /// newline. Pairs with [`LineSuffix`] so a trailing `--` comment actually ends its line.
    BreakParent,
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

/// Verbatim text (a borrowed `&'static str` or an owned `String`). The display width is measured
/// once here and cached on the node, so the printer's fit/break measurement never re-scans it.
pub fn text(s: impl Into<Cow<'static, str>>) -> Doc {
    let s = s.into();
    let width = text_width(&s);
    Doc::Text(s, width)
}

/// Lay out the parts one after another.
pub fn concat(parts: Vec<Doc>) -> Doc {
    Doc::Concat(parts)
}

/// A layout-choice group: flat when it fits on the line, broken otherwise.
pub fn group(inner: Doc) -> Doc {
    let must_break = has_forced_break(&inner);
    Doc::Group {
        content: Box::new(inner),
        expand: false,
        must_break,
    }
}

/// A group forced to break (its soft lines always become newlines), without forcing enclosing
/// groups to break. The building block for magic-trailing-comma "keep this exploded".
pub fn group_expanded(inner: Doc) -> Doc {
    Doc::Group {
        content: Box::new(inner),
        expand: true,
        must_break: true,
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
    Doc::Text(Cow::Borrowed(" "), 1)
}

/// The empty document (renders to nothing).
pub fn empty() -> Doc {
    Doc::Concat(Vec::new())
}

/// Defer `inner` to just before the next newline (used for trailing line comments).
pub fn line_suffix(inner: Doc) -> Doc {
    Doc::LineSuffix(Box::new(inner))
}

/// Force enclosing groups to break without emitting a newline here.
pub fn break_parent() -> Doc {
    Doc::BreakParent
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

/// Try the candidate documents in order, printing the first that fits in the remaining width and
/// falling back to the last candidate when none fit. Empty candidates render as empty.
pub fn best_fitting(candidates: Vec<Doc>) -> Doc {
    Doc::BestFitting(candidates)
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

/// Display width of a string in terminal columns: the sum of each character's column width, so a
/// line of CJK text is measured by how wide it actually renders, not by its scalar count. This only
/// feeds the fit/break decision, so refining it never changes which tokens are emitted.
///
/// Pure ASCII — the overwhelmingly common case in SQL — has width equal to its byte length, so we
/// short-circuit on it and only decode/classify code points when a non-ASCII byte is present.
fn text_width(s: &str) -> usize {
    if s.is_ascii() {
        return s.len();
    }
    s.chars().map(char_width).sum()
}

/// Column width of a single character: `2` for East Asian Wide / Fullwidth code points (per Unicode
/// Annex #11 — CJK ideographs, kana, Hangul syllables, fullwidth forms, most emoji), `1` otherwise.
/// Combining marks are not special-cased (rare in SQL), so this is an upper bound there.
fn char_width(c: char) -> usize {
    let cp = c as u32;
    let wide = matches!(cp,
        0x1100..=0x115F   // Hangul Jamo
        | 0x2E80..=0x303E // CJK radicals, Kangxi, CJK symbols & punctuation
        | 0x3041..=0x33FF // Hiragana, Katakana, Bopomofo, CJK compat
        | 0x3400..=0x4DBF // CJK Unified Ideographs Extension A
        | 0x4E00..=0x9FFF // CJK Unified Ideographs
        | 0xA000..=0xA4CF // Yi syllables
        | 0xAC00..=0xD7A3 // Hangul syllables
        | 0xF900..=0xFAFF // CJK compatibility ideographs
        | 0xFE10..=0xFE19 // vertical forms
        | 0xFE30..=0xFE6F // CJK compatibility & small form variants
        | 0xFF00..=0xFF60 // fullwidth forms
        | 0xFFE0..=0xFFE6 // fullwidth signs
        | 0x1F300..=0x1FAFF // symbols, pictographs & emoji (wide)
        | 0x20000..=0x3FFFD // supplementary ideographic planes
    );
    if wide {
        2
    } else {
        1
    }
}

/// Does `doc` contain a hard line anywhere within (propagating through nested groups, exactly like
/// Prettier's `breakParent`)? Such a document can never be printed flat.
///
/// Nested groups are consulted via their cached `must_break` flag rather than re-walked. Because
/// the builders compute this bottom-up (a group's flag is set when it is constructed, by which time
/// its children already carry theirs), the whole tree is classified in O(nodes) overall.
fn has_forced_break(doc: &Doc) -> bool {
    match doc {
        Doc::Line(LineKind::Hard) | Doc::BreakParent => true,
        Doc::Line(_) | Doc::Text(..) => false,
        // A line suffix's own content is deferred and must not force the current line to break.
        Doc::LineSuffix(_) => false,
        Doc::Concat(parts) => parts.iter().any(has_forced_break),
        Doc::BestFitting(candidates) => {
            !candidates.is_empty() && candidates.iter().all(has_forced_break)
        }
        Doc::Indent(inner) => has_forced_break(inner),
        // An exploded group propagates to ancestors: a multiline collection can't sit inline, so
        // every group containing it must break too (cf. Black's magic trailing comma). The answer
        // was precomputed when the group was built.
        Doc::Group { must_break, .. } => *must_break,
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
            Doc::Text(_, width) => {
                remaining -= *width as isize;
                if remaining < 0 {
                    return false;
                }
            }
            Doc::Concat(parts) => {
                for part in parts.iter().rev() {
                    stack.push(Cmd { doc: part, ..cmd });
                }
            }
            Doc::BestFitting(candidates) => {
                if let Some(candidate) = candidates.first() {
                    stack.push(Cmd {
                        doc: candidate,
                        ..cmd
                    });
                }
            }
            Doc::Indent(inner) => stack.push(Cmd {
                indent: cmd.indent + opts.indent_width,
                doc: inner,
                mode: cmd.mode,
            }),
            Doc::Group {
                content,
                must_break,
                ..
            } => {
                let mode = if *must_break { Mode::Break } else { Mode::Flat };
                stack.push(Cmd {
                    indent: cmd.indent,
                    mode,
                    doc: content,
                });
            }
            // A line suffix is deferred to the next newline; it does not consume current width.
            // A break parent is a zero-width marker. Neither affects whether the line fits.
            Doc::LineSuffix(_) | Doc::BreakParent => {}
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
    // Content deferred by `LineSuffix`, flushed in order just before the next newline (or at EOF).
    let mut line_suffixes: Vec<Cmd> = Vec::new();

    loop {
        let cmd = match cmds.pop() {
            Some(cmd) => cmd,
            None if !line_suffixes.is_empty() => {
                // Flush remaining suffixes at the document's end (no trailing newline followed).
                while let Some(suffix) = line_suffixes.pop() {
                    cmds.push(suffix);
                }
                continue;
            }
            None => break,
        };
        match cmd.doc {
            Doc::Text(s, width) => {
                out.push_str(s);
                col += *width;
            }
            Doc::Concat(parts) => {
                for part in parts.iter().rev() {
                    cmds.push(Cmd { doc: part, ..cmd });
                }
            }
            Doc::BestFitting(candidates) => {
                if candidates.is_empty() {
                    continue;
                }
                let mut chosen = candidates.last().expect("non-empty candidates");
                for candidate in candidates {
                    if fits(
                        opts.line_width as isize - col as isize,
                        &cmds,
                        Cmd {
                            indent: cmd.indent,
                            mode: cmd.mode,
                            doc: candidate,
                        },
                        opts,
                    ) {
                        chosen = candidate;
                        break;
                    }
                }
                cmds.push(Cmd { doc: chosen, ..cmd });
            }
            Doc::LineSuffix(inner) => line_suffixes.push(Cmd { doc: inner, ..cmd }),
            Doc::BreakParent => {}
            Doc::Indent(inner) => cmds.push(Cmd {
                indent: cmd.indent + opts.indent_width,
                doc: inner,
                mode: cmd.mode,
            }),
            Doc::Group {
                content,
                must_break,
                ..
            } => {
                let mode = if *must_break {
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
                    // Before breaking, emit any deferred line suffixes on this line, then
                    // reprocess the newline with an empty buffer.
                    if !line_suffixes.is_empty() {
                        cmds.push(cmd);
                        while let Some(suffix) = line_suffixes.pop() {
                            cmds.push(suffix);
                        }
                        continue;
                    }
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
///
/// Single streaming pass: each line is `trim_end`ed; leading blank lines are skipped, and interior
/// blank lines are buffered as `pending_blanks` so any run of them that turns out to be trailing is
/// dropped rather than emitted. Equivalent to the former collect-join-trim, without the temporary
/// `Vec<&str>` or the intermediate joined `String`.
fn finalize(raw: String) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut started = false;
    let mut pending_blanks = 0usize;
    for line in raw.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if started {
                pending_blanks += 1;
            }
            // leading blank lines are dropped entirely
            continue;
        }
        if started {
            // one separator for the previous content line, plus any buffered interior blanks
            out.push('\n');
            for _ in 0..pending_blanks {
                out.push('\n');
            }
        }
        pending_blanks = 0;
        out.push_str(line);
        started = true;
    }
    if started {
        out.push('\n');
    }
    out
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
    fn cjk_text_is_measured_double_width() {
        assert_eq!(text_width("abc"), 3);
        assert_eq!(text_width("長芋"), 4); // two wide chars
        assert_eq!(text_width("a長"), 3); // mixed
                                          // Flat width is 長(2) + space(1) + 芋(2) = 5: fits at 5, breaks at 4.
        let doc = group(concat(vec![text("長"), line(), text("芋")]));
        assert_eq!(p(&doc, 4), "長\n芋\n");
        assert_eq!(p(&doc, 5), "長 芋\n");
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
    fn best_fitting_picks_first_candidate_that_fits() {
        let doc = best_fitting(vec![
            text("short"),
            concat(vec![text("fallback"), hard_line(), text("layout")]),
        ]);
        assert_eq!(p(&doc, 10), "short\n");
    }

    #[test]
    fn best_fitting_falls_back_to_last_candidate() {
        let doc = best_fitting(vec![
            text("too-wide"),
            concat(vec![text("fallback"), hard_line(), text("layout")]),
        ]);
        assert_eq!(p(&doc, 4), "fallback\nlayout\n");
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
    fn line_suffix_defers_content_to_the_end_of_the_line() {
        // The suffix is emitted before the hard line, even though it appears first in the concat.
        let doc = concat(vec![
            line_suffix(text(" -- note")),
            text("code"),
            hard_line(),
            text("next"),
        ]);
        assert_eq!(p(&doc, 80), "code -- note\nnext\n");
    }

    #[test]
    fn line_suffix_flushes_at_end_of_document() {
        let doc = concat(vec![line_suffix(text(" -- note")), text("code")]);
        assert_eq!(p(&doc, 80), "code -- note\n");
    }

    #[test]
    fn break_parent_forces_its_group_to_break() {
        let doc = group(concat(vec![
            text("a"),
            break_parent(),
            indent(concat(vec![line(), text("b")])),
        ]));
        assert_eq!(p(&doc, 80), "a\n    b\n");
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

    #[test]
    fn text_caches_its_display_width() {
        // The width folded in by `text` must equal a fresh measurement, for both ASCII (fast path)
        // and wide CJK input — this is what the fit/break decision now reads instead of re-scanning.
        for s in ["select", "a, b, c", "長芋", "a長b", ""] {
            match text(s) {
                Doc::Text(stored, width) => {
                    assert_eq!(stored, s);
                    assert_eq!(width, text_width(s));
                }
                other => panic!("text() should build a Text node, got {other:?}"),
            }
        }
        // The ASCII fast path agrees with the per-char fold.
        assert_eq!(
            text_width("SELECT a, b"),
            "SELECT a, b".chars().map(char_width).sum::<usize>()
        );
    }

    #[test]
    fn group_precomputes_its_forced_break() {
        // A plain group over flat content stays optional; a group wrapping a hard line is marked
        // must-break at construction, which is what the printer reads in O(1).
        match group(concat(vec![text("a"), line(), text("b")])) {
            Doc::Group { must_break, .. } => assert!(!must_break),
            other => panic!("expected a group, got {other:?}"),
        }
        match group(concat(vec![text("a"), hard_line(), text("b")])) {
            Doc::Group { must_break, .. } => assert!(must_break),
            other => panic!("expected a group, got {other:?}"),
        }
        // group_expanded is always must-break.
        match group_expanded(text("x")) {
            Doc::Group { must_break, .. } => assert!(must_break),
            other => panic!("expected a group, got {other:?}"),
        }
    }

    #[test]
    fn finalize_drops_leading_and_trailing_blank_lines_but_keeps_interior() {
        // Leading and trailing blank lines vanish; an interior blank-line run is preserved; every
        // line is right-trimmed. Equivalent to the previous collect/join/trim implementation.
        let raw = String::from("\n  \na  \n\n\nb \n\n  \n");
        assert_eq!(finalize(raw), "a\n\n\nb\n");
        assert_eq!(finalize(String::new()), "");
        assert_eq!(finalize(String::from("   \n  ")), "");
        assert_eq!(finalize(String::from("only")), "only\n");
    }
}
