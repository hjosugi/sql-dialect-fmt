//! Databricks/Delta maintenance + cache statements.
//!
//! These statements are recognized **only** under the Databricks dialect. Their leading words
//! (`VACUUM`, `OPTIMIZE`, `CACHE`, `UNCACHE`, `REFRESH`, and `HISTORY`/`ZORDER`/`RETAIN`/…) are not
//! reserved keywords — they are matched contextually (a bare `IDENT` whose text equals the word,
//! via [`ContextualKeyword`]) at statement start. Under Snowflake the same words stay ordinary
//! identifiers, so Snowflake parsing/formatting is byte-identical and these constructs round-trip
//! verbatim there exactly as they did before.
//!
//! Every rule is total: a recognized leading word always advances at least one token, so the
//! never-fail / no-panic and lossless-round-trip invariants hold even on malformed tails.

use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{ContextualKeyword, Parser};

const DELTA_COMMAND_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Vacuum,
    ContextualKeyword::Optimize,
    ContextualKeyword::Cache,
    ContextualKeyword::Uncache,
    ContextualKeyword::Refresh,
];

/// At the start of a Delta maintenance/cache statement (`VACUUM`/`OPTIMIZE`/`CACHE`/`UNCACHE`/
/// `REFRESH`), or a `DESCRIBE HISTORY`/`DESC HISTORY`. Only meaningful under the Databricks dialect;
/// callers gate on [`crate::Dialect::supports_delta_commands`].
pub(in crate::grammar) fn at_delta_stmt_start(p: &Parser) -> bool {
    at_command_word(p) || at_describe_history(p)
}

/// At one of the bare command words (`VACUUM`/`OPTIMIZE`/`CACHE`/`UNCACHE`/`REFRESH`) used as a
/// statement leader. We require the word to be followed by a plausible operand (a name, a string
/// path, `TABLE`, `LAZY`, or `IF`) so a lone identifier expression — `SELECT vacuum` has already
/// been handled, but a top-level `vacuum` expression statement — is not stolen.
fn at_command_word(p: &Parser) -> bool {
    p.nth_any_contextual(0, DELTA_COMMAND_WORDS) && at_command_operand(p, 1)
}

/// Is the token `n` ahead a plausible operand for a Delta command word: a name (identifier / quoted
/// identifier), a string path, or one of the `TABLE`/`LAZY`/`IF` lead-ins?
fn at_command_operand(p: &Parser, n: usize) -> bool {
    p.nth_at(n, IDENT)
        || p.nth_at(n, QUOTED_IDENT)
        || p.nth_at(n, STRING)
        || p.nth_at(n, TABLE_KW)
        || p.nth_at(n, IF_KW)
        || p.nth_contextual(n, ContextualKeyword::Lazy)
}

/// At `DESCRIBE HISTORY …` / `DESC HISTORY …` — the Delta change-history form of DESCRIBE.
pub(in crate::grammar) fn at_describe_history(p: &Parser) -> bool {
    (p.at(DESCRIBE_KW) || p.at(DESC_KW)) && p.nth_contextual(1, ContextualKeyword::History)
}

/// Dispatch to the matching Delta statement rule. Caller guarantees [`at_delta_stmt_start`].
pub(in crate::grammar) fn delta_stmt(p: &mut Parser) {
    if at_describe_history(p) {
        describe_history_stmt(p);
    } else if p.nth_contextual(0, ContextualKeyword::Vacuum) {
        vacuum_stmt(p);
    } else if p.nth_contextual(0, ContextualKeyword::Optimize) {
        optimize_stmt(p);
    } else if p.nth_contextual(0, ContextualKeyword::Uncache) {
        uncache_stmt(p);
    } else if p.nth_contextual(0, ContextualKeyword::Cache) {
        cache_stmt(p);
    } else if p.nth_contextual(0, ContextualKeyword::Refresh) {
        refresh_stmt(p);
    } else {
        // Unreachable given the `at_*` gate, but stay total.
        super::expr(p);
    }
}

/// A table-or-path operand: either a (possibly qualified, possibly `delta.`/path/``) name, or a
/// bare string-literal path. Always consumes at least one token when one is present.
fn table_or_path(p: &mut Parser) {
    if p.at(STRING) {
        let m = p.start();
        p.bump(STRING);
        m.complete(p, NAME_REF);
    } else if p.at_name() {
        super::name_ref(p);
    } else {
        p.error("expected a table name or path");
    }
}

/// `VACUUM <table|path> [RETAIN <n> HOURS] [DRY RUN]`. Everything formats on one line.
fn vacuum_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // VACUUM
    table_or_path(p);
    if p.nth_contextual(0, ContextualKeyword::Retain) {
        p.bump_as(CONTEXTUAL_KEYWORD); // RETAIN
        if super::at_expr_start(p) {
            super::expr(p); // the retention count
        } else {
            p.error("expected a retention duration after RETAIN");
        }
        if p.nth_contextual(0, ContextualKeyword::Hours) {
            p.bump_as(CONTEXTUAL_KEYWORD); // HOURS
        } else {
            p.error("expected HOURS after the retention duration");
        }
    }
    if p.nth_contextual(0, ContextualKeyword::Dry) {
        p.bump_as(CONTEXTUAL_KEYWORD); // DRY
        if p.nth_contextual(0, ContextualKeyword::Run) {
            p.bump_as(CONTEXTUAL_KEYWORD); // RUN
        } else {
            p.error("expected RUN after DRY");
        }
    }
    m.complete(p, VACUUM_STMT);
}

/// `OPTIMIZE <table> [WHERE <predicate>] [ZORDER BY ( <col> [, ...] )]`.
fn optimize_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // OPTIMIZE
    table_or_path(p);
    if p.at(WHERE_KW) {
        super::where_clause(p);
    }
    if p.nth_contextual(0, ContextualKeyword::Zorder) {
        zorder_clause(p);
    }
    m.complete(p, OPTIMIZE_STMT);
}

/// `ZORDER BY ( col [, ...] )` — formatted like any other parenthesized column list.
fn zorder_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // ZORDER
    p.expect(BY_KW);
    if p.at(L_PAREN) {
        super::column_list(p);
    } else {
        p.error("expected '(' after ZORDER BY");
    }
    m.complete(p, ZORDER_CLAUSE);
}

/// `CACHE [LAZY] TABLE <t> [OPTIONS (...)] [[AS] <query>]`. The defining query (if any) lands on its
/// own indented line; the `OPTIONS (...)` body is kept as a lossless token run.
fn cache_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // CACHE
    if p.nth_contextual(0, ContextualKeyword::Lazy) {
        p.bump_as(CONTEXTUAL_KEYWORD); // LAZY
    }
    p.expect(TABLE_KW);
    table_or_path(p);
    if p.nth_contextual(0, ContextualKeyword::Options) {
        p.bump_as(CONTEXTUAL_KEYWORD); // OPTIONS
        if p.at(L_PAREN) {
            super::balanced_parens(p);
        } else {
            p.error("expected '(' after OPTIONS");
        }
    }
    // Optional defining query: `[AS] { SELECT … | WITH … | VALUES … | ( … ) }`.
    p.eat(AS_KW);
    if at_cache_query(p) {
        super::query_expr(p);
    }
    m.complete(p, CACHE_STMT);
}

/// At a query that can follow `CACHE … [AS]`.
fn at_cache_query(p: &Parser) -> bool {
    p.at(SELECT_KW)
        || p.at(WITH_KW)
        || p.at(VALUES_KW)
        || p.at(TABLE_KW)
        || p.at(FROM_KW)
        || (p.at(L_PAREN) && (p.nth_at(1, SELECT_KW) || p.nth_at(1, WITH_KW)))
}

/// `UNCACHE TABLE [IF EXISTS] <t>`.
fn uncache_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // UNCACHE
    p.expect(TABLE_KW);
    if p.at(IF_KW) {
        p.bump(IF_KW);
        p.expect(EXISTS_KW);
    }
    table_or_path(p);
    m.complete(p, UNCACHE_STMT);
}

/// `REFRESH [TABLE] <t>` or `REFRESH <path>`.
fn refresh_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // REFRESH
    p.eat(TABLE_KW);
    table_or_path(p);
    m.complete(p, REFRESH_STMT);
}

/// `DESCRIBE HISTORY <table>` / `DESC HISTORY <table>`.
fn describe_history_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump_any(); // DESCRIBE / DESC
    p.bump_as(CONTEXTUAL_KEYWORD); // HISTORY
    if p.at_name() || p.at(STRING) {
        table_or_path(p);
    }
    // Tolerate a trailing limit/options tail leniently so it round-trips.
    while !super::at_stmt_terminator(p) {
        p.bump_any();
    }
    m.complete(p, DESCRIBE_HISTORY_STMT);
}
