//! The parser state machine: lookahead, token matching, markers, and error recording.
//!
//! The grammar (in [`crate::grammar`]) drives this via a small vocabulary: `at`/`eat`/`bump`
//! for tokens, `start`/`complete`/`precede` for nodes. A `fuel` counter turns any accidental
//! non-advancing loop into a loud panic instead of a hang.

use std::cell::Cell;

use snow_fmt_syntax::{keyword_kind, SyntaxKind};

use crate::event::Event;
use crate::input::Input;
use crate::ParseError;

const INITIAL_FUEL: u32 = 256;

pub(crate) struct Parser<'a> {
    input: &'a Input<'a>,
    pos: usize,
    fuel: Cell<u32>,
    events: Vec<Event>,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(input: &'a Input<'a>) -> Self {
        Parser {
            input,
            pos: 0,
            fuel: Cell::new(INITIAL_FUEL),
            events: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub(crate) fn parse(mut self) -> (Vec<Event>, Vec<ParseError>) {
        crate::grammar::source_file(&mut self);
        (self.events, self.errors)
    }

    // ---- lookahead ----

    pub(crate) fn at_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    /// Raw kind `n` tokens ahead, consuming a unit of fuel (the loop guard).
    fn nth(&self, n: usize) -> SyntaxKind {
        assert_ne!(
            self.fuel.get(),
            0,
            "parser stuck — no progress at pos {}",
            self.pos
        );
        self.fuel.set(self.fuel.get() - 1);
        self.input.kind(self.pos + n)
    }

    /// Is the current token `kind`? Keyword kinds match a raw `IDENT` whose text is that keyword.
    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        if kind.is_keyword() {
            self.nth(0) == SyntaxKind::IDENT
                && keyword_kind(self.input.text(self.pos)) == Some(kind)
        } else {
            self.nth(0) == kind
        }
    }

    /// Is the current token a usable *name* (a non-keyword identifier or a quoted identifier)?
    pub(crate) fn at_name(&self) -> bool {
        match self.nth(0) {
            SyntaxKind::QUOTED_IDENT => true,
            SyntaxKind::IDENT => keyword_kind(self.input.text(self.pos)).is_none(),
            _ => false,
        }
    }

    /// Like [`Self::at`], but `n` tokens ahead — used for the handful of two-token decisions
    /// (`NOT IN`, `NOT LIKE`, `( SELECT`, ...).
    pub(crate) fn nth_at(&self, n: usize, kind: SyntaxKind) -> bool {
        if kind.is_keyword() {
            self.nth(n) == SyntaxKind::IDENT
                && keyword_kind(self.input.text(self.pos + n)) == Some(kind)
        } else {
            self.nth(n) == kind
        }
    }

    /// The kind to tag the current token with: a keyword kind if the raw `IDENT` is a keyword,
    /// otherwise the raw kind. Keeps keyword tokens correctly typed in the tree for highlighting.
    fn current_remapped(&self) -> SyntaxKind {
        let raw = self.input.kind(self.pos);
        if raw == SyntaxKind::IDENT {
            keyword_kind(self.input.text(self.pos)).unwrap_or(SyntaxKind::IDENT)
        } else {
            raw
        }
    }

    // ---- consumption ----

    fn advance(&mut self, kind: SyntaxKind) {
        assert!(!self.at_eof(), "advance past end of input");
        self.events.push(Event::Advance { kind });
        self.pos += 1;
        self.fuel.set(INITIAL_FUEL);
    }

    /// Consume the current token, tagging keywords with their keyword kind.
    pub(crate) fn bump_any(&mut self) {
        let kind = self.current_remapped();
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

    /// Consume `kind` if present, else record a diagnostic (no token is consumed).
    pub(crate) fn expect(&mut self, kind: SyntaxKind) {
        if !self.eat(kind) {
            self.error(format!("expected {kind:?}"));
        }
    }

    // ---- errors ----

    pub(crate) fn error(&mut self, msg: impl Into<String>) {
        let offset = self.input.offset(self.pos);
        self.errors.push(ParseError {
            message: msg.into(),
            offset,
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
        });
        Marker {
            index,
            completed: false,
        }
    }
}

/// A still-open node. Must be `complete`d (a `DropBomb` catches grammar bugs that forget to).
pub(crate) struct Marker {
    index: usize,
    completed: bool,
}

impl Marker {
    pub(crate) fn complete(mut self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
        self.completed = true;
        p.events[self.index] = Event::Open { kind };
        p.events.push(Event::Close);
        CompletedMarker { index: self.index }
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
    /// binary expressions). Inserts an `Open` before this node's `Open`.
    pub(crate) fn precede(self, p: &mut Parser) -> Marker {
        p.events.insert(
            self.index,
            Event::Open {
                kind: SyntaxKind::ERROR,
            },
        );
        Marker {
            index: self.index,
            completed: false,
        }
    }
}
