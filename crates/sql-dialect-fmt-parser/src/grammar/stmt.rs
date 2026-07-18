//! Top-level statement dispatch and Snowflake flow-chain parsing, plus the simple statement
//! forms (`CALL`, session `SET`, `EXECUTE IMMEDIATE`) and the generic lenient statement rule.

use sql_dialect_fmt_syntax::SyntaxKind;
use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::Parser;

/// Parse a statement, then — if it is followed by the flow operator `->>` — the rest of the chain,
/// wrapping the whole pipeline in a [`FLOW_STMT`]. A lone statement abandons the wrapper. Flow
/// chains carry no semicolons between steps; a later step references an earlier one via `$n` in its
/// FROM clause. See <https://docs.snowflake.com/en/sql-reference/operators-flow>.
pub(super) fn statement_or_flow(p: &mut Parser) {
    let m = p.start();
    statement(p);
    if p.dialect().supports_flow_operator() && p.at(FLOW_PIPE) {
        while p.eat(FLOW_PIPE) {
            if at_stmt_start(p) {
                statement(p);
            } else {
                p.error("expected a statement after '->>'");
                break;
            }
        }
        m.complete(p, FLOW_STMT);
    } else {
        m.abandon(p);
    }
}

pub(super) fn at_stmt_start(p: &Parser) -> bool {
    at_sql_stmt_start(p) || super::at_expr_start(p)
}

pub(super) fn at_sql_stmt_start(p: &Parser) -> bool {
    p.at(SELECT_KW)
        || p.at(WITH_KW)
        || p.at(VALUES_KW)
        || p.at(INSERT_KW)
        || p.at(UPDATE_KW)
        || p.at(DELETE_KW)
        || p.at(MERGE_KW)
        || p.at(CREATE_KW)
        || p.at(DROP_KW)
        || p.at(ALTER_KW)
        || p.at(GRANT_KW)
        || p.at(REVOKE_KW)
        || p.at(USE_KW)
        || p.at(SHOW_KW)
        || p.at(DESCRIBE_KW)
        || p.at(DESC_KW)
        || p.at(TRUNCATE_KW)
        || (p.dialect().supports_scripting_blocks() && super::at_block_start(p))
        || p.at(COMMIT_KW)
        || p.at(ROLLBACK_KW)
        || super::at_begin_transaction(p)
        || p.at(UNDROP_KW)
        || super::at_comment_stmt(p)
        || p.at(CALL_KW)
        || p.at(SET_KW)
        || p.at(EXECUTE_KW)
        || (p.dialect().supports_copy_into() && p.at(COPY_KW))
        || super::stage::at_stage_file_stmt(p)
        || (p.dialect().supports_delta_commands() && super::delta::at_delta_stmt_start(p))
}

pub(super) fn statement(p: &mut Parser) {
    if p.at(WITH_KW) {
        super::with_query(p);
    } else if p.at(INSERT_KW) {
        super::dml::insert_stmt(p);
    } else if p.at(UPDATE_KW) {
        super::dml::update_stmt(p);
    } else if p.at(DELETE_KW) {
        super::dml::delete_stmt(p);
    } else if p.at(MERGE_KW) {
        super::dml::merge_stmt(p);
    } else if p.at(CREATE_KW) {
        super::create_stmt(p);
    } else if p.at(DROP_KW) {
        super::drop_stmt(p);
    } else if p.at(ALTER_KW) {
        super::alter_stmt(p);
    } else if p.at(GRANT_KW) {
        super::grant_stmt(p);
    } else if p.at(REVOKE_KW) {
        super::revoke_stmt(p);
    } else if p.at(USE_KW) {
        lenient_stmt(p, USE_STMT);
    } else if p.at(SHOW_KW) {
        lenient_stmt(p, SHOW_STMT);
    } else if p.dialect().supports_delta_commands() && super::delta::at_describe_history(p) {
        super::delta::delta_stmt(p);
    } else if p.at(DESCRIBE_KW) || p.at(DESC_KW) {
        lenient_stmt(p, DESCRIBE_STMT);
    } else if p.at(TRUNCATE_KW) {
        lenient_stmt(p, TRUNCATE_STMT);
    } else if p.dialect().supports_scripting_blocks() && super::at_block_start(p) {
        super::block_stmt(p);
    } else if p.at(COMMIT_KW) || p.at(ROLLBACK_KW) || super::at_begin_transaction(p) {
        lenient_stmt(p, TRANSACTION_STMT);
    } else if p.at(UNDROP_KW) {
        lenient_stmt(p, UNDROP_STMT);
    } else if super::at_comment_stmt(p) {
        super::comment_stmt(p);
    } else if p.at(CALL_KW) {
        call_stmt(p);
    } else if p.at(SET_KW) {
        set_stmt(p);
    } else if p.at(EXECUTE_KW) {
        execute_stmt(p);
    } else if p.dialect().supports_copy_into() && p.at(COPY_KW) {
        super::copy_stmt(p);
    } else if super::stage::at_stage_file_stmt(p) {
        super::stage::stage_file_stmt(p);
    } else if p.dialect().supports_delta_commands() && super::delta::at_delta_stmt_start(p) {
        super::delta::delta_stmt(p);
    } else if p.at(SELECT_KW)
        || p.at(VALUES_KW)
        || (p.at(L_PAREN) && (p.nth_at(1, SELECT_KW) || p.nth_at(1, WITH_KW)))
    {
        super::query_expr(p);
    } else {
        let m = p.start();
        let before = p.pos();
        super::expr(p);
        if p.pos() == before {
            p.err_and_bump("expected an expression");
        }
        m.complete(p, EXPR_STMT);
    }
}

/// Parse the leading keyword and the rest of the statement as a flat token run, completing it as
/// `node`. Used for statements whose surface is large/evolving (GRANT, REVOKE) or simple enough that
/// inline token rendering is all the formatter needs (USE, SHOW, DESCRIBE, TRUNCATE): the result
/// round-trips losslessly and gets inline spacing normalization rather than erroring the file.
fn lenient_stmt(p: &mut Parser, node: SyntaxKind) {
    let m = p.start();
    p.bump_any(); // the leading statement keyword
    while !super::at_stmt_terminator(p) {
        p.bump_any();
    }
    m.complete(p, node);
}

/// `CALL proc(args)` — invoke a stored procedure. The invocation is an ordinary call expression, so
/// its argument list is formatted like any other (one-per-line when it overflows). A trailing tail
/// (e.g. `INTO :result`) is kept leniently as tokens so it round-trips.
pub(super) fn call_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(CALL_KW);
    super::expr(p); // the procedure-call expression: name(args)
    while !super::at_stmt_terminator(p) {
        p.bump_any();
    }
    m.complete(p, CALL_STMT);
}

// ---- session SET / EXECUTE IMMEDIATE ----

/// `SET <name> = <expr>` or `SET (<name>, ...) = (<expr>, ...)` (session variables).
fn set_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(SET_KW);
    if p.at(L_PAREN) {
        super::column_list(p);
    } else {
        super::name_ref(p);
    }
    p.expect(EQ);
    if p.at(L_PAREN) {
        // A tuple / subquery right-hand side.
        p.bump(L_PAREN);
        if p.at(SELECT_KW) || p.at(WITH_KW) {
            super::query_expr(p);
        } else if !p.at(R_PAREN) {
            super::expr_list(p);
        }
        p.expect(R_PAREN);
    } else {
        super::expr(p);
    }
    m.complete(p, SET_STMT);
}

/// `EXECUTE IMMEDIATE <string|$$…$$|:var> [USING (<binds>)]`.
fn execute_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(EXECUTE_KW);
    p.expect(IMMEDIATE_KW);
    super::expr(p);
    if p.eat(USING_KW) {
        if p.at(L_PAREN) {
            p.bump(L_PAREN);
            if !p.at(R_PAREN) {
                super::expr_list(p);
            }
            p.expect(R_PAREN);
        } else {
            super::expr(p);
            while p.eat(COMMA) {
                super::expr(p);
            }
        }
    }
    m.complete(p, EXECUTE_STMT);
}
