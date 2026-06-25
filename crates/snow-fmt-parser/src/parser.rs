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

/// Snowflake's *contextual keywords*: words that act as keywords only in a specific syntactic
/// position and otherwise remain ordinary identifiers. They are never lexed as keywords and never
/// reserved, so the grammar recognizes them by text via [`Parser::nth_contextual`]. Listing them in
/// one enum keeps the set discoverable and the match texts typo-proof (a misspelling is a compile
/// error rather than a silently-never-matching string).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextualKeyword {
    /// `<table> AT (...)` — time travel.
    At,
    /// `<table> BEFORE (...)` — time travel.
    Before,
    /// `ASOF JOIN` — the join-type word.
    Asof,
    /// `ASOF JOIN ... MATCH_CONDITION (...)`.
    MatchCondition,
    /// `<table> MATCH_RECOGNIZE (...)`.
    MatchRecognize,
    /// `GROUP BY GROUPING SETS (...)` — first word.
    Grouping,
    /// `GROUPING SETS (...)` — second word.
    Sets,
    // ---- MATCH_RECOGNIZE body vocabulary ----
    /// `MEASURES <expr> AS <alias> [, ...]`.
    Measures,
    /// `PATTERN ( <row pattern> )`.
    Pattern,
    /// `DEFINE <symbol> AS <predicate> [, ...]`.
    Define,
    /// `SUBSET <name> = ( <symbol>, ... )`.
    Subset,
    /// `... PER MATCH`, `AFTER MATCH SKIP`.
    Match,
    /// `ONE ROW PER MATCH`.
    One,
    /// `AFTER MATCH SKIP ...`.
    Skip,
    /// `AFTER MATCH SKIP PAST LAST ROW`.
    Past,
    /// `AFTER MATCH SKIP TO NEXT ROW`.
    Next,
    /// `AFTER MATCH SKIP TO [FIRST|LAST] <symbol>`.
    To,
    /// `CONNECT BY NOCYCLE ...`.
    NoCycle,
    /// `<table> CHANGES ( INFORMATION => ... )` — change-tracking queries.
    Changes,
    /// `COMMENT ON <object> IS '...'` — recognized only before `ON` so it never shadows the very
    /// common `comment` column/identifier.
    Comment,
    /// `BEGIN TRANSACTION` — distinguishes a transaction start from a Snowflake Scripting block.
    Transaction,
    /// `BEGIN WORK` — the SQL-standard spelling of a transaction start.
    Work,
    // ---- Snowflake Scripting structural words (not reserved: `default`/`break` are common
    // identifiers, so they up-case only in their scripting position). The counter-loop `TO` reuses
    // the `To` variant declared above. ----
    /// `FOR i IN REVERSE <start> TO <end>` — counts down.
    Reverse,
    /// `<name> [<type>] DEFAULT <expr>` — a declaration's default value (also a DDL column default).
    Default,
    /// `BREAK [<label>]` — exit a loop.
    Break,
    /// `CONTINUE [<label>]` — skip to the next loop iteration.
    Continue,
    // ---- Phase 7 object DDL kinds (contextual so they stay usable as identifiers) ----
    /// `CREATE SCHEMA …`.
    Schema,
    /// `CREATE DATABASE …`.
    Database,
    /// `CREATE STAGE …` / `… ON STAGE …`.
    Stage,
    /// `CREATE SEQUENCE …`.
    Sequence,
    /// `CREATE STREAM …`.
    Stream,
    /// `CREATE DYNAMIC TABLE …`.
    Dynamic,
    /// `CREATE FILE FORMAT …`.
    File,
    /// `CREATE FILE FORMAT …` — second word.
    Format,
    // ---- Phase 7 GRANT / REVOKE vocabulary ----
    /// `… TO ROLE r` / `… FROM ROLE r`.
    Role,
    /// `… TO USER u`.
    User,
    /// `GRANT <role> TO SHARE s`.
    Share,
    /// `REVOKE … FROM r RESTRICT|CASCADE` — cascade.
    Cascade,
    /// `REVOKE … FROM r RESTRICT` — restrict.
    Restrict,
    /// `REVOKE GRANT OPTION FOR …` / `… WITH GRANT OPTION` — option.
    Option,
    /// `GRANT ALL PRIVILEGES …`.
    Privileges,
}

impl ContextualKeyword {
    /// The lowercase source text this word matches case-insensitively.
    fn text(self) -> &'static str {
        match self {
            ContextualKeyword::At => "at",
            ContextualKeyword::Before => "before",
            ContextualKeyword::Asof => "asof",
            ContextualKeyword::MatchCondition => "match_condition",
            ContextualKeyword::MatchRecognize => "match_recognize",
            ContextualKeyword::Grouping => "grouping",
            ContextualKeyword::Sets => "sets",
            ContextualKeyword::Measures => "measures",
            ContextualKeyword::Pattern => "pattern",
            ContextualKeyword::Define => "define",
            ContextualKeyword::Subset => "subset",
            ContextualKeyword::Match => "match",
            ContextualKeyword::One => "one",
            ContextualKeyword::Skip => "skip",
            ContextualKeyword::Past => "past",
            ContextualKeyword::Next => "next",
            ContextualKeyword::To => "to",
            ContextualKeyword::NoCycle => "nocycle",
            ContextualKeyword::Changes => "changes",
            ContextualKeyword::Comment => "comment",
            ContextualKeyword::Transaction => "transaction",
            ContextualKeyword::Work => "work",
            ContextualKeyword::Reverse => "reverse",
            ContextualKeyword::Default => "default",
            ContextualKeyword::Break => "break",
            ContextualKeyword::Continue => "continue",
            ContextualKeyword::Schema => "schema",
            ContextualKeyword::Database => "database",
            ContextualKeyword::Stage => "stage",
            ContextualKeyword::Sequence => "sequence",
            ContextualKeyword::Stream => "stream",
            ContextualKeyword::Dynamic => "dynamic",
            ContextualKeyword::File => "file",
            ContextualKeyword::Format => "format",
            ContextualKeyword::Role => "role",
            ContextualKeyword::User => "user",
            ContextualKeyword::Share => "share",
            ContextualKeyword::Cascade => "cascade",
            ContextualKeyword::Restrict => "restrict",
            ContextualKeyword::Option => "option",
            ContextualKeyword::Privileges => "privileges",
        }
    }
}

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

    /// Is the current token identifier-like (a bare `IDENT`, keyword or not, or a quoted
    /// identifier)? Used to recognize a named-argument label before `=>`.
    pub(crate) fn at_ident_like(&self) -> bool {
        matches!(self.nth(0), SyntaxKind::IDENT | SyntaxKind::QUOTED_IDENT)
    }

    /// Is the current token a (reserved-spelled) keyword word? Used to recognize a keyword used as a
    /// function name (`first(x)`, `last(x)`), the complement of [`Self::at_name`].
    pub(crate) fn at_keyword(&self) -> bool {
        self.nth(0) == SyntaxKind::IDENT && keyword_kind(self.input.text(self.pos)).is_some()
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
