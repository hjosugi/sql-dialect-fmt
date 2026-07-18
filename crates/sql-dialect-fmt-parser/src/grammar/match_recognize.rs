//! `MATCH_RECOGNIZE` row-pattern matching attached to a table reference.

use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{ContextualKeyword, Parser};

use super::{
    balanced_parens, column_list, expr, name_ref, order_by_clause, partition_by_clause, select_item,
};

/// `<table> MATCH_RECOGNIZE ( <body> )`. The body's clauses appear in a fixed order
/// (PARTITION BY / ORDER BY / MEASURES / {ONE ROW|ALL ROWS} PER MATCH / AFTER MATCH SKIP /
/// PATTERN / SUBSET / DEFINE) but are parsed resiliently: dispatch on the clause-introducing word
/// and, for anything unrecognized, consume one token so the rule stays total and lossless. The
/// `MATCH_RECOGNIZE` word and the body keywords (MEASURES/PATTERN/DEFINE/…) are contextual.
pub(super) fn match_recognize(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // MATCH_RECOGNIZE (contextual keyword)
    if !p.eat(L_PAREN) {
        p.error("expected '(' after MATCH_RECOGNIZE");
        m.complete(p, MATCH_RECOGNIZE);
        return;
    }
    while !p.at(R_PAREN) && !p.at_eof() {
        if p.at(PARTITION_KW) {
            partition_by_clause(p);
        } else if p.at(ORDER_KW) {
            order_by_clause(p);
        } else if p.nth_contextual(0, ContextualKeyword::Measures) {
            measures_clause(p);
        } else if p.nth_contextual(0, ContextualKeyword::Pattern) {
            pattern_clause(p);
        } else if p.nth_contextual(0, ContextualKeyword::Define) {
            define_clause(p);
        } else if p.nth_contextual(0, ContextualKeyword::Subset) {
            subset_clause(p);
        } else if at_row_match_clause(p) {
            row_match_clause(p);
        } else if p.at(AFTER_KW) {
            after_match_clause(p);
        } else {
            p.bump_any(); // lenient: never stall on unmodelled syntax
        }
    }
    p.expect(R_PAREN);
    m.complete(p, MATCH_RECOGNIZE);
}

/// `MEASURES [ { FINAL | RUNNING } ] <expr> [AS] <alias> [, ...]` (reusing the select-item shape:
/// expression + optional alias). The window-semantics prefixes are contextual: they are tagged as
/// keywords only at the start of a measure item and stay ordinary identifiers elsewhere.
fn measures_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // MEASURES
    measure_item(p);
    while p.eat(COMMA) {
        measure_item(p);
    }
    m.complete(p, MEASURES_CLAUSE);
}

fn measure_item(p: &mut Parser) {
    if p.nth_contextual(0, ContextualKeyword::Final)
        || p.nth_contextual(0, ContextualKeyword::Running)
    {
        p.bump_as(CONTEXTUAL_KEYWORD);
    }
    select_item(p);
}

/// `PATTERN ( <row pattern> )`. The pattern is a regex-like sub-language (`A+ B* (C | D){1,3}`)
/// where `+`/`*`/`?` are postfix quantifiers, not operators — capture it as a [`PATTERN_BODY`] node
/// so the formatter can emit it verbatim instead of re-spacing it.
fn pattern_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // PATTERN
    if p.at(L_PAREN) {
        let b = p.start();
        balanced_parens(p);
        b.complete(p, PATTERN_BODY);
    } else {
        p.error("expected '(' after PATTERN");
    }
    m.complete(p, PATTERN_CLAUSE);
}

/// `DEFINE <symbol> AS <predicate> [, ...]`.
fn define_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // DEFINE
    define_item(p);
    while p.eat(COMMA) {
        define_item(p);
    }
    m.complete(p, DEFINE_CLAUSE);
}

fn define_item(p: &mut Parser) {
    let m = p.start();
    name_ref(p); // pattern variable (symbol)
    p.expect(AS_KW);
    expr(p); // predicate
    m.complete(p, DEFINE_ITEM);
}

/// `SUBSET <name> = ( <symbol>, ... ) [, ...]`.
fn subset_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // SUBSET
    loop {
        name_ref(p);
        p.expect(EQ);
        if p.at(L_PAREN) {
            column_list(p);
        }
        if !p.eat(COMMA) {
            break;
        }
    }
    m.complete(p, SUBSET_CLAUSE);
}

/// `{ ONE ROW | ALL ROWS } PER MATCH` — recognized by the leading `ONE`/`ALL` plus `ROW(S)`.
fn at_row_match_clause(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::One) || p.at(ALL_KW))
        && (p.nth_at(1, ROW_KW) || p.nth_at(1, ROWS_KW))
}

/// `{ ONE ROW | ALL ROWS } PER MATCH [ SHOW EMPTY MATCHES | OMIT EMPTY MATCHES | WITH UNMATCHED
/// ROWS ]`. After the leading `ONE ROW`/`ALL ROWS`, the remaining words are all structural (no
/// pattern symbols), so consume them as soft keywords until the next clause or the closing paren.
fn row_match_clause(p: &mut Parser) {
    let m = p.start();
    if p.at(ALL_KW) {
        p.bump(ALL_KW);
    } else {
        p.bump_as(CONTEXTUAL_KEYWORD); // ONE
    }
    while !p.at(R_PAREN) && !p.at_eof() && !at_mr_clause_start(p) {
        soft_keyword_word(p);
    }
    m.complete(p, ROW_MATCH_CLAUSE);
}

/// `AFTER MATCH SKIP { PAST LAST ROW | TO NEXT ROW | TO [ FIRST | LAST ] <symbol> }`. The trailing
/// `<symbol>` keeps its case (it is a pattern variable), so it is parsed as a name, not up-cased.
fn after_match_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(AFTER_KW);
    if p.nth_contextual(0, ContextualKeyword::Match) {
        p.bump_as(CONTEXTUAL_KEYWORD); // MATCH
    }
    if p.nth_contextual(0, ContextualKeyword::Skip) {
        p.bump_as(CONTEXTUAL_KEYWORD); // SKIP
    }
    if p.nth_contextual(0, ContextualKeyword::Past) {
        p.bump_as(CONTEXTUAL_KEYWORD); // PAST
        p.eat(LAST_KW);
        p.eat(ROW_KW);
    } else if p.nth_contextual(0, ContextualKeyword::To) {
        p.bump_as(CONTEXTUAL_KEYWORD); // TO
        if p.nth_contextual(0, ContextualKeyword::Next) {
            p.bump_as(CONTEXTUAL_KEYWORD); // NEXT
            p.eat(ROW_KW);
        } else {
            if p.at(FIRST_KW) || p.at(LAST_KW) {
                p.bump_any();
            }
            if p.at_name() {
                name_ref(p); // <symbol>
            }
        }
    }
    m.complete(p, AFTER_MATCH_CLAUSE);
}

/// True at the start of any MATCH_RECOGNIZE body clause — used to bound the lenient word runs.
fn at_mr_clause_start(p: &Parser) -> bool {
    p.at(PARTITION_KW)
        || p.at(ORDER_KW)
        || p.at(AFTER_KW)
        || p.nth_contextual(0, ContextualKeyword::Measures)
        || p.nth_contextual(0, ContextualKeyword::Pattern)
        || p.nth_contextual(0, ContextualKeyword::Define)
        || p.nth_contextual(0, ContextualKeyword::Subset)
        || at_row_match_clause(p)
}

/// Consume one word of a structural option run (`PER MATCH`, `SHOW EMPTY MATCHES`, …). These runs
/// hold no pattern symbols, so every word up-cases: tag identifier-like tokens as soft keywords and
/// leave any punctuation untouched.
fn soft_keyword_word(p: &mut Parser) {
    if p.at_ident_like() {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else {
        p.bump_any();
    }
}
