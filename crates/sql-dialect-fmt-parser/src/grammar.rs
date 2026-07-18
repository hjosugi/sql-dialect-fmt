//! The Snowflake SQL grammar.
//!
//! Phase 1 covered a single `SELECT` plus a Pratt expression parser. Phase 2 grows this toward
//! "parse the most common queries completely": all single-`SELECT` clauses, `JOIN`s, subqueries
//! and derived tables, set operations, CTEs, and the compound predicates (`IS [NOT] NULL`,
//! `[NOT] IN/BETWEEN/LIKE`).
//!
//! Every rule is total: on unexpected input it records a diagnostic and recovers, never panics.
//!
//! The grammar is split into per-family submodules — `stmt` (dispatch), `query`, `expr`,
//! `ddl`, `dml`, `access`, `copy`, `scripting`, `match_recognize`, `stage`, and `delta` —
//! while this root hosts the shared name/list/token-run helpers and re-exports the rules the
//! submodules call across module boundaries.

use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{CompletedMarker, Parser};

mod access;
mod copy;
mod ddl;
mod delta;
mod dml;
mod expr;
mod match_recognize;
mod query;
mod scripting;
mod stage;
mod stmt;

// The rules shared across submodules are re-exported at the grammar root, so sibling modules
// address every cross-module rule uniformly as `super::<rule>` no matter which submodule hosts
// it.
use self::access::{grant_stmt, revoke_stmt};
use self::copy::{copy_option, copy_stmt, stage_ref};
use self::ddl::{alter_stmt, at_comment_stmt, comment_stmt, create_stmt, drop_stmt};
pub(crate) use self::expr::expr;
use self::expr::{
    arg_list, at_expr_start, expr_bp, expr_list, partition_by_clause, type_name, window_spec,
    BP_CMP,
};
use self::match_recognize::match_recognize;
use self::query::{
    from_clause, order_by_clause, query_expr, select_item, subquery, table_ref, values_clause,
    where_clause, with_query,
};
use self::scripting::{at_begin_transaction, at_block_start, block_stmt};
use self::stmt::call_stmt;

// ---- top level ----

pub(crate) fn source_file(p: &mut Parser) {
    let m = p.start();
    while !p.at_eof() {
        if p.at(SEMICOLON) {
            p.bump(SEMICOLON); // statement separator / empty statement
        } else if stmt::at_stmt_start(p) {
            stmt::statement_or_flow(p);
        } else {
            p.err_and_bump("expected a statement");
        }
    }
    m.complete(p, SOURCE_FILE);
}

/// A top-level statement ends at `;`, EOF, or the flow operator `->>` that chains it to the next
/// statement. Lenient statement parsers consult this so `->>` is left for `stmt::statement_or_flow`
/// instead of being swallowed into the preceding flat token run.
fn at_stmt_terminator(p: &Parser) -> bool {
    p.at(SEMICOLON) || (p.dialect().supports_flow_operator() && p.at(FLOW_PIPE)) || p.at_eof()
}

// ---- names ----

fn name(p: &mut Parser) {
    let m = p.start();
    if p.at_name() {
        p.bump_any();
    } else {
        p.error("expected a name");
    }
    m.complete(p, NAME);
}

fn name_ref(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    if p.at_name() {
        p.bump_any();
        while p.at(DOT) {
            p.bump(DOT);
            if p.at_name() {
                p.bump_any();
            } else if p.at(STAR) {
                p.bump(STAR); // qualified star: t.*
                break;
            } else {
                p.error("expected a name after '.'");
                break;
            }
        }
    } else {
        p.error("expected a name");
    }
    m.complete(p, NAME_REF)
}

fn column_list(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    if p.at_name() {
        name(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            name(p);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, COLUMN_LIST);
}

// ---- balanced token runs ----

/// Consume a balanced `( ... )` token run, tracking only token-level parentheses (string and
/// `$$ … $$` tokens are opaque, so their inner parens don't count).
fn balanced_parens(p: &mut Parser) {
    p.bump(L_PAREN);
    let mut depth = 1u32;
    while depth > 0 && !p.at_eof() {
        if p.at(L_PAREN) {
            depth += 1;
        } else if p.at(R_PAREN) {
            depth -= 1;
        }
        p.bump_any();
    }
}

/// Consume a token run that may contain nested parentheses, stopping when `stop` matches at the
/// top level. Callers supply the non-paren token bumping rule so contextual-word tagging remains
/// local to the grammar region being scanned.
fn balanced_token_run_until(
    p: &mut Parser,
    stop: impl Fn(&Parser) -> bool,
    mut bump_word: impl FnMut(&mut Parser),
) {
    let mut depth = 0u32;
    while !p.at_eof() {
        if depth == 0 && stop(p) {
            break;
        }
        if p.at(L_PAREN) {
            depth += 1;
            p.bump_any();
        } else if p.at(R_PAREN) && depth > 0 {
            depth -= 1;
            p.bump_any();
        } else {
            bump_word(p);
        }
    }
}
