//! The parser state machine: lookahead, token matching, markers, and error recording.
//!
//! The grammar (in [`crate::grammar`]) drives this via a small vocabulary: `at`/`eat`/`bump`
//! for tokens, `start`/`complete`/`precede` for nodes. A `fuel` counter turns any accidental
//! non-advancing loop into a debug assertion instead of a hang.

use std::cell::Cell;

use sql_dialect_fmt_syntax::{keyword_kind_for, Dialect, SyntaxKind};

use crate::event::Event;
use crate::input::Input;
use crate::ParseError;

// Top-level dispatch intentionally probes the broad Snowflake/Databricks statement surface twice:
// once to decide whether a token starts a statement and once to select its grammar. Keep enough
// headroom for those bounded lookaheads while still terminating a genuinely non-advancing loop.
const INITIAL_FUEL: u32 = 1024;

pub(crate) use crate::contextual::ContextualKeyword;

pub(crate) struct Parser<'a> {
    input: &'a Input<'a>,
    dialect: Dialect,
    pos: usize,
    fuel: Cell<u32>,
    exhausted_fuel_at: Cell<Option<usize>>,
    events: Vec<Event>,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(input: &'a Input<'a>, dialect: Dialect) -> Self {
        Parser {
            input,
            dialect,
            pos: 0,
            fuel: Cell::new(INITIAL_FUEL),
            exhausted_fuel_at: Cell::new(None),
            events: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// The SQL dialect this parse targets. Grammar branches that recognize dialect-specific
    /// constructs gate on this via the [`Dialect`] predicate methods.
    pub(crate) fn dialect(&self) -> Dialect {
        self.dialect
    }

    pub(crate) fn parse(mut self) -> (Vec<Event>, Vec<ParseError>) {
        crate::grammar::source_file(&mut self);
        if let Some(pos) = self.exhausted_fuel_at.get() {
            self.errors.push(ParseError {
                message: "parser fuel exhausted; recovered at current token".into(),
                offset: self.input.offset(pos),
                len: self.input.token_len(pos),
                line_column: None,
            });
        }
        (self.events, self.errors)
    }

    // ---- lookahead ----

    pub(crate) fn at_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Raw kind `n` tokens ahead, consuming a unit of fuel (the loop guard).
    fn nth(&self, n: usize) -> SyntaxKind {
        if self.pos + n >= self.input.len() {
            return SyntaxKind::EOF;
        }
        debug_assert_ne!(
            self.fuel.get(),
            0,
            "parser stuck — no progress at pos {}",
            self.pos
        );
        if self.fuel.get() == 0 {
            if self.exhausted_fuel_at.get().is_none() {
                self.exhausted_fuel_at.set(Some(self.pos));
            }
            return SyntaxKind::EOF;
        }
        self.fuel.set(self.fuel.get() - 1);
        self.input.kind(self.pos + n)
    }

    /// Whether `text` is a *reserved* keyword in this parser's [`Dialect`], and its kind if so.
    ///
    /// The single point where keyword reservation is resolved for the active dialect: every
    /// keyword decision (`at`, `at_name`, `at_keyword`, `nth_at`, `current_remapped`) routes through
    /// here, so a Snowflake-only word like `TASK`/`FLATTEN` is a plain identifier under Databricks
    /// while Snowflake reservation (the default) is unchanged.
    #[inline]
    fn keyword_kind(&self, text: &str) -> Option<SyntaxKind> {
        keyword_kind_for(text, self.dialect)
    }

    /// Is the current token `kind`? Keyword kinds match a raw `IDENT` whose text is that keyword.
    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        if kind.is_keyword() {
            self.nth(0) == SyntaxKind::IDENT
                && self.keyword_kind(self.input.text(self.pos)) == Some(kind)
        } else {
            self.nth(0) == kind
        }
    }

    /// Is the current token a usable *name* (a non-keyword identifier or a quoted identifier)?
    pub(crate) fn at_name(&self) -> bool {
        match self.nth(0) {
            SyntaxKind::QUOTED_IDENT => true,
            SyntaxKind::IDENT => self
                .keyword_kind(self.input.text(self.pos))
                .is_none_or(is_identifier_compatible_keyword),
            _ => false,
        }
    }

    /// Is the current token identifier-like (a bare `IDENT`, keyword or not, or a quoted
    /// identifier)? Used to recognize a named-argument label before `=>`.
    pub(crate) fn at_ident_like(&self) -> bool {
        matches!(self.nth(0), SyntaxKind::IDENT | SyntaxKind::QUOTED_IDENT)
    }

    /// Is the current token a (reserved-spelled) keyword word? Used to recognize a keyword used as a
    /// function name (`first(x)`, `last(x)`), the complement of [`Self::at_name`].
    pub(crate) fn at_keyword(&self) -> bool {
        self.nth(0) == SyntaxKind::IDENT && self.keyword_kind(self.input.text(self.pos)).is_some()
    }

    /// Is the token `n` ahead a given [`ContextualKeyword`]: a bare `IDENT` whose text matches
    /// case-insensitively? Used for non-reserved words like `GROUPING`/`SETS` that must not become
    /// real keywords (they double as the `GROUPING(col)` function and ordinary identifiers).
    pub(crate) fn nth_contextual(&self, n: usize, kw: ContextualKeyword) -> bool {
        self.nth(n) == SyntaxKind::IDENT
            && self
                .input
                .text(self.pos + n)
                .eq_ignore_ascii_case(kw.text())
    }

    /// Is the token `n` ahead any of the given contextual keywords?
    pub(crate) fn nth_any_contextual(&self, n: usize, kws: &[ContextualKeyword]) -> bool {
        kws.iter().any(|&kw| self.nth_contextual(n, kw))
    }

    /// Like [`Self::at`], but `n` tokens ahead — used for the handful of two-token decisions
    /// (`NOT IN`, `NOT LIKE`, `( SELECT`, ...).
    pub(crate) fn nth_at(&self, n: usize, kind: SyntaxKind) -> bool {
        if kind.is_keyword() {
            self.nth(n) == SyntaxKind::IDENT
                && self.keyword_kind(self.input.text(self.pos + n)) == Some(kind)
        } else {
            self.nth(n) == kind
        }
    }

    /// The kind to tag the current token with: a keyword kind if the raw `IDENT` is a keyword,
    /// otherwise the raw kind. Keeps keyword tokens correctly typed in the tree for highlighting.
    fn current_remapped(&self) -> SyntaxKind {
        let raw = self.input.kind(self.pos);
        if raw == SyntaxKind::IDENT {
            self.keyword_kind(self.input.text(self.pos))
                .unwrap_or(SyntaxKind::IDENT)
        } else {
            raw
        }
    }

    // ---- consumption ----

    fn advance(&mut self, kind: SyntaxKind) {
        // Never advance past end of input. Callers guard with `at`/`!at_eof`, but stay total
        // (return rather than `assert!`) so a stray call at EOF can never panic — the parser's
        // hard never-panic invariant. `bump`/`bump_as` keep their `debug_assert` guards for tests.
        if self.at_eof() {
            return;
        }
        self.events.push(Event::Advance { kind });
        self.pos += 1;
        self.fuel.set(INITIAL_FUEL);
    }

    /// Consume the current token, tagging keywords with their keyword kind.
    pub(crate) fn bump_any(&mut self) {
        let kind = self.current_remapped();
        self.advance(kind);
    }

    /// Consume the current token, tagging it with `kind` regardless of its keyword-ness. Used for
    /// positions where a keyword-spelled word is really a plain identifier (e.g. a case-sensitive
    /// semi-structured path key like `payload:order`), so it is not later up-cased as a keyword.
    pub(crate) fn bump_as(&mut self, kind: SyntaxKind) {
        debug_assert!(!self.at_eof(), "bump_as past end of input");
        self.advance(kind);
    }

    /// Consume the current token, asserting it is `kind` (used after an `at` check).
    pub(crate) fn bump(&mut self, kind: SyntaxKind) {
        debug_assert!(self.at(kind), "bump({kind:?}) but not at it");
        self.advance(kind);
    }

    /// Consume the current token iff it is `kind`.
    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.advance(kind);
            true
        } else {
            false
        }
    }

    /// Consume `kind` if present, else record a diagnostic (no token is consumed). The message
    /// names the expected token with its human-readable spelling (`expected INTO`, `expected '('`),
    /// never the internal `SyntaxKind` debug name.
    pub(crate) fn expect(&mut self, kind: SyntaxKind) {
        if !self.eat(kind) {
            self.error(format!("expected {}", kind.describe()));
        }
    }

    // ---- errors ----

    /// Record a diagnostic spanning the current (offending) token. At end of input the span is the
    /// zero-width point at the source's end, so the message still attaches to a location.
    pub(crate) fn error(&mut self, msg: impl Into<String>) {
        let offset = self.input.offset(self.pos);
        let len = self.input.token_len(self.pos);
        self.errors.push(ParseError {
            message: msg.into(),
            offset,
            len,
            line_column: None,
        });
    }

    /// Record a diagnostic and wrap the offending token in an `ERROR` node so the tree stays
    /// lossless and parsing keeps making progress.
    pub(crate) fn err_and_bump(&mut self, msg: impl Into<String>) {
        self.error(msg);
        if self.at_eof() {
            return;
        }
        let m = self.start();
        self.bump_any();
        m.complete(self, SyntaxKind::ERROR);
    }

    // ---- markers ----

    pub(crate) fn start(&mut self) -> Marker {
        let index = self.events.len();
        self.events.push(Event::Open {
            kind: SyntaxKind::ERROR,
            forward_parent: None,
        });
        Marker {
            index,
            completed: false,
        }
    }
}

fn is_identifier_compatible_keyword(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::LANGUAGE_KW
            | SyntaxKind::JAVASCRIPT_KW
            | SyntaxKind::PYTHON_KW
            | SyntaxKind::JAVA_KW
            | SyntaxKind::SCALA_KW
            | SyntaxKind::SQL_KW
    )
}

/// A still-open node. Must be `complete`d (a `DropBomb` catches grammar bugs that forget to).
pub(crate) struct Marker {
    index: usize,
    completed: bool,
}

impl Marker {
    pub(crate) fn complete(mut self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
        self.completed = true;
        p.events[self.index] = Event::Open {
            kind,
            forward_parent: None,
        };
        p.events.push(Event::Close);
        CompletedMarker { index: self.index }
    }

    /// Discard this (speculative) wrapper: its `Open` becomes a no-op and any nodes parsed inside it
    /// stay attached to the parent. Used when a wrapper is started before knowing whether it is
    /// needed (e.g. a flow-operator chain that turns out to be a single statement).
    pub(crate) fn abandon(mut self, p: &mut Parser) {
        self.completed = true;
        p.events[self.index] = Event::Tombstone;
    }
}

impl Drop for Marker {
    fn drop(&mut self) {
        if !self.completed && !std::thread::panicking() {
            panic!("Marker dropped without being completed");
        }
    }
}

/// A finished node, which can be retroactively wrapped by a new parent via [`Self::precede`].
#[derive(Clone, Copy)]
pub(crate) struct CompletedMarker {
    index: usize,
}

impl CompletedMarker {
    /// Start a new node that begins where this one began (left-associative wrapping, e.g. for
    /// binary expressions). The new parent is appended and linked from this node's `Open`, so long
    /// expression chains stay linear instead of repeatedly inserting into the event vector.
    pub(crate) fn precede(self, p: &mut Parser) -> Marker {
        let new_index = p.events.len();
        match &mut p.events[self.index] {
            Event::Open { forward_parent, .. } => {
                debug_assert!(
                    forward_parent.is_none(),
                    "node already has a forward parent"
                );
                *forward_parent = Some(new_index - self.index);
            }
            _ => debug_assert!(false, "precede must point at an open event"),
        }
        p.events.push(Event::Open {
            kind: SyntaxKind::ERROR,
            forward_parent: None,
        });
        Marker {
            index: new_index,
            completed: false,
        }
    }
}
