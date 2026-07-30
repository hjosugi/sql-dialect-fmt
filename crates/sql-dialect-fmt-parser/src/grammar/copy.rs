//! `COPY INTO` statements and staged-file references (`@stage/path`).

use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::Parser;

use super::{at_stmt_terminator, balanced_parens, column_list, name_ref, option_value, subquery};

// ---- COPY INTO (Phase 6) ----

/// `COPY INTO <target> FROM <source> <option>*` (both the load and unload shapes). The location
/// operands (`@stage/path`, table names) are captured verbatim — stage paths use `/` which would be
/// mangled by operator spacing — while options are parsed as `name = value` (or `PARTITION BY (...)`)
/// so each can sit on its own line.
pub(super) fn copy_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(COPY_KW);
    p.expect(INTO_KW);
    copy_operand(p);
    if p.eat(FROM_KW) {
        copy_operand(p);
    }
    // `CREATE PIPE ... AS (COPY INTO ...)` wraps the COPY statement in parentheses. The closing
    // paren belongs to that outer body node, not to a COPY option.
    while !at_stmt_terminator(p) && !p.at(R_PAREN) {
        copy_option(p);
    }
    m.complete(p, COPY_STMT);
}

/// A staged-file reference used where a table can appear: `@[~|%][namespace.]stage[/path...]`.
///
/// The lexer splits this into many tokens (`@`, names, `/`, `.`, `~`, `%`, numbers); we gather a
/// contiguous run into one `STAGE_REF` node. To avoid swallowing a following clause keyword (e.g.
/// `FROM @s WHERE`), additional path segments are only consumed when joined by a `/` or `.`
/// connector — a bare word after whitespace ends the reference. The rule is total and never panics.
pub(super) fn stage_ref(p: &mut Parser) {
    let m = p.start();
    p.bump(AT); // @
    p.eat(TILDE); // @~ (the user's home stage)
    p.eat(PERCENT); // @%table (a table's internal stage)
    eat_stage_atom(p); // stage / table / namespace name (possibly a quoted identifier)
    while p.at(DOT) || p.at(SLASH) {
        // Further `.namespace` / `/path` segments, only when introduced by a connector.
        p.bump_any(); // . or /
        while eat_stage_atom(p) {}
    }
    m.complete(p, STAGE_REF);
}

/// Consume one atom of a stage path (a name, number, or `~`/`%`), returning whether one was eaten.
/// Anything else (paren, comma, `=`, EOF, a clause keyword) ends the path.
fn eat_stage_atom(p: &mut Parser) -> bool {
    if at_stage_ref_boundary(p) {
        return false;
    }
    if p.at(IDENT)
        || p.at(QUOTED_IDENT)
        || p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(TILDE)
        || p.at(PERCENT)
    {
        p.bump_any();
        true
    } else {
        false
    }
}

fn at_stage_ref_boundary(p: &Parser) -> bool {
    let keyword_boundary = p.at(FROM_KW)
        || p.at(WHERE_KW)
        || p.at(GROUP_KW)
        || p.at(HAVING_KW)
        || p.at(QUALIFY_KW)
        || p.at(WINDOW_KW)
        || p.at(ORDER_KW)
        || p.at(LIMIT_KW)
        || p.at(OFFSET_KW)
        || p.at(FETCH_KW)
        || p.at(JOIN_KW)
        || p.at(INNER_KW)
        || p.at(LEFT_KW)
        || p.at(RIGHT_KW)
        || p.at(FULL_KW)
        || p.at(CROSS_KW)
        || p.at(NATURAL_KW)
        || at_copy_option_start(p);
    keyword_boundary && !p.nth_at(1, SLASH) && !p.nth_at(1, DOT)
}

/// A COPY target/source: a parenthesized query, a `@stage/path` reference, a table name with an
/// optional column list, or — for anything else (an external `'s3://…'` location, a malformed
/// operand) — a verbatim token run up to `FROM`, the first option, or the statement end.
///
/// The table form is structured (a [`NAME_REF`] plus a [`COLUMN_LIST`], like `INSERT INTO t (…)`)
/// so a wide load list wraps to the line width instead of being replayed at whatever indentation
/// the input happened to use. Stage paths stay verbatim because their `/` would be re-spaced as a
/// division operator.
fn copy_operand(p: &mut Parser) {
    if p.at(L_PAREN) {
        subquery(p);
        return;
    }
    let m = p.start();
    if p.at(AT) {
        stage_ref(p);
        m.complete(p, COPY_LOCATION);
        return;
    }
    if p.at_name() && !at_copy_option_start(p) {
        name_ref(p);
        if p.at(L_PAREN) {
            column_list(p);
        }
    }
    // Any residue (an external location literal, or an unmodeled tail after the table name) stays a
    // lossless flat run.
    while !p.at(FROM_KW) && !at_stmt_terminator(p) && !at_copy_option_start(p) {
        p.bump_any();
    }
    m.complete(p, COPY_LOCATION);
}

/// A COPY option starts at `PARTITION BY` or any word immediately followed by `=`.
fn at_copy_option_start(p: &Parser) -> bool {
    p.at(PARTITION_KW) || p.nth_at(1, EQ)
}

pub(super) fn copy_option(p: &mut Parser) {
    let m = p.start();
    if p.at(PARTITION_KW) {
        p.bump(PARTITION_KW);
        p.expect(BY_KW);
        if p.at(L_PAREN) {
            balanced_parens(p);
        }
    } else {
        p.bump_any(); // option name
        if p.eat(EQ) {
            if p.at(L_PAREN) {
                balanced_parens(p);
            } else if !at_stmt_terminator(p) {
                option_value(p); // a literal / bare-word / qualified-name value
            }
        }
    }
    m.complete(p, COPY_OPTION);
}
