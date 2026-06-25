//! The Snowflake SQL grammar.
//!
//! Phase 1 covered a single `SELECT` plus a Pratt expression parser. Phase 2 grows this toward
//! "parse the most common queries completely": all single-`SELECT` clauses, `JOIN`s, subqueries
//! and derived tables, set operations, CTEs, and the compound predicates (`IS [NOT] NULL`,
//! `[NOT] IN/BETWEEN/LIKE`).
//!
//! Every rule is total: on unexpected input it records a diagnostic and recovers, never panics.

use snow_fmt_syntax::SyntaxKind;
use snow_fmt_syntax::SyntaxKind::*;

use crate::parser::{CompletedMarker, ContextualKeyword, Parser};

// Binding powers for the Pratt parser. Higher binds tighter; (left, right) for infix.
const BP_OR: (u8, u8) = (1, 2);
const BP_AND: (u8, u8) = (3, 4);
const BP_CMP: (u8, u8) = (7, 8);
const BP_CONCAT: (u8, u8) = (9, 10);
const BP_ADD: (u8, u8) = (11, 12);
const BP_MUL: (u8, u8) = (13, 14);
const BP_PREFIX_NOT: u8 = 6; // looser than comparison: `NOT a = b` == `NOT (a = b)`
const BP_PREFIX_NEG: u8 = 15; // unary +/- bind tighter than `*`

// ---- top level ----

pub(crate) fn source_file(p: &mut Parser) {
    let m = p.start();
    while !p.at_eof() {
        if p.at(SEMICOLON) {
            p.bump(SEMICOLON); // statement separator / empty statement
        } else if at_stmt_start(p) {
            statement_or_flow(p);
        } else {
            p.err_and_bump("expected a statement");
        }
    }
    m.complete(p, SOURCE_FILE);
}

/// Parse a statement, then — if it is followed by the flow operator `->>` — the rest of the chain,
/// wrapping the whole pipeline in a [`FLOW_STMT`]. A lone statement abandons the wrapper. Flow
/// chains carry no semicolons between steps; a later step references an earlier one via `$n` in its
/// FROM clause. See <https://docs.snowflake.com/en/sql-reference/operators-flow>.
fn statement_or_flow(p: &mut Parser) {
    let m = p.start();
    statement(p);
    if p.at(FLOW_PIPE) {
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

fn at_stmt_start(p: &Parser) -> bool {
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
        || at_block_start(p)
        || p.at(COMMIT_KW)
        || p.at(ROLLBACK_KW)
        || at_begin_transaction(p)
        || p.at(UNDROP_KW)
        || at_comment_stmt(p)
        || p.at(CALL_KW)
        || p.at(SET_KW)
        || p.at(EXECUTE_KW)
        || p.at(COPY_KW)
        || at_expr_start(p)
}

fn statement(p: &mut Parser) {
    if p.at(WITH_KW) {
        with_query(p);
    } else if p.at(INSERT_KW) {
        insert_stmt(p);
    } else if p.at(UPDATE_KW) {
        update_stmt(p);
    } else if p.at(DELETE_KW) {
        delete_stmt(p);
    } else if p.at(MERGE_KW) {
        merge_stmt(p);
    } else if p.at(CREATE_KW) {
        create_stmt(p);
    } else if p.at(DROP_KW) {
        drop_stmt(p);
    } else if p.at(ALTER_KW) {
        alter_stmt(p);
    } else if p.at(GRANT_KW) {
        lenient_stmt(p, GRANT_STMT);
    } else if p.at(REVOKE_KW) {
        lenient_stmt(p, REVOKE_STMT);
    } else if p.at(USE_KW) {
        lenient_stmt(p, USE_STMT);
    } else if p.at(SHOW_KW) {
        lenient_stmt(p, SHOW_STMT);
    } else if p.at(DESCRIBE_KW) || p.at(DESC_KW) {
        lenient_stmt(p, DESCRIBE_STMT);
    } else if p.at(TRUNCATE_KW) {
        lenient_stmt(p, TRUNCATE_STMT);
    } else if at_block_start(p) {
        block_stmt(p);
    } else if p.at(COMMIT_KW) || p.at(ROLLBACK_KW) || at_begin_transaction(p) {
        lenient_stmt(p, TRANSACTION_STMT);
    } else if p.at(UNDROP_KW) {
        lenient_stmt(p, UNDROP_STMT);
    } else if at_comment_stmt(p) {
        comment_stmt(p);
    } else if p.at(CALL_KW) {
        call_stmt(p);
    } else if p.at(SET_KW) {
        set_stmt(p);
    } else if p.at(EXECUTE_KW) {
        execute_stmt(p);
    } else if p.at(COPY_KW) {
        copy_stmt(p);
    } else if p.at(SELECT_KW)
        || p.at(VALUES_KW)
        || (p.at(L_PAREN) && (p.nth_at(1, SELECT_KW) || p.nth_at(1, WITH_KW)))
    {
        query_expr(p);
    } else {
        let m = p.start();
        expr(p);
        m.complete(p, EXPR_STMT);
    }
}

// ---- DML (Phase 6) ----

fn insert_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(INSERT_KW);
    p.eat(OVERWRITE_KW);
    if p.at(ALL_KW) || p.at(FIRST_KW) {
        multi_table_insert(p);
    } else {
        // Single-table: INSERT [OVERWRITE] INTO t [(cols)] VALUES/<query>.
        p.expect(INTO_KW);
        name_ref(p);
        if p.at(L_PAREN) {
            column_list(p);
        }
        if p.at(VALUES_KW) {
            values_clause(p);
        } else {
            query_expr(p);
        }
    }
    m.complete(p, INSERT_STMT);
}

/// `INSERT [OVERWRITE] ALL <into>+ <query>` (unconditional) or
/// `INSERT [OVERWRITE] {ALL|FIRST} (WHEN <cond> THEN <into>+)+ [ELSE <into>+] <query>`.
fn multi_table_insert(p: &mut Parser) {
    p.bump_any(); // ALL or FIRST
    if p.at(WHEN_KW) {
        while p.at(WHEN_KW) {
            insert_when(p);
        }
        if p.eat(ELSE_KW) {
            while p.at(INTO_KW) {
                into_clause(p);
            }
        }
    } else {
        while p.at(INTO_KW) {
            into_clause(p);
        }
    }
    query_expr(p); // the source rows
}

fn insert_when(p: &mut Parser) {
    let m = p.start();
    p.bump(WHEN_KW);
    expr(p);
    p.expect(THEN_KW);
    while p.at(INTO_KW) {
        into_clause(p);
    }
    m.complete(p, INSERT_WHEN);
}

fn into_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(INTO_KW);
    name_ref(p);
    if p.at(L_PAREN) {
        column_list(p);
    }
    if p.at(VALUES_KW) {
        values_clause(p);
    }
    m.complete(p, INTO_CLAUSE);
}

fn update_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(UPDATE_KW);
    table_ref(p);
    set_clause(p);
    if p.at(FROM_KW) {
        from_clause(p);
    }
    if p.at(WHERE_KW) {
        where_clause(p);
    }
    m.complete(p, UPDATE_STMT);
}

fn delete_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(DELETE_KW);
    p.expect(FROM_KW);
    table_ref(p);
    if p.eat(USING_KW) {
        table_ref(p);
        while p.eat(COMMA) {
            table_ref(p);
        }
    }
    if p.at(WHERE_KW) {
        where_clause(p);
    }
    m.complete(p, DELETE_STMT);
}

fn set_clause(p: &mut Parser) {
    let m = p.start();
    p.expect(SET_KW);
    assignment(p);
    while p.eat(COMMA) {
        assignment(p);
    }
    m.complete(p, SET_CLAUSE);
}

fn assignment(p: &mut Parser) {
    let m = p.start();
    name_ref(p);
    p.expect(EQ);
    expr(p);
    m.complete(p, ASSIGNMENT);
}

fn merge_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(MERGE_KW);
    p.expect(INTO_KW);
    table_ref(p);
    p.expect(USING_KW);
    table_ref(p);
    p.expect(ON_KW);
    expr(p);
    while p.at(WHEN_KW) {
        merge_when(p);
    }
    m.complete(p, MERGE_STMT);
}

fn merge_when(p: &mut Parser) {
    let m = p.start();
    p.bump(WHEN_KW);
    p.eat(NOT_KW);
    p.expect(MATCHED_KW);
    if p.eat(AND_KW) {
        expr(p); // WHEN MATCHED AND <cond>
    }
    p.expect(THEN_KW);
    if p.at(UPDATE_KW) {
        p.bump(UPDATE_KW);
        set_clause(p);
    } else if p.at(DELETE_KW) {
        p.bump(DELETE_KW);
    } else if p.at(INSERT_KW) {
        p.bump(INSERT_KW);
        if p.at(L_PAREN) {
            column_list(p);
        }
        if p.at(VALUES_KW) {
            values_clause(p);
        }
    } else {
        p.error("expected UPDATE, DELETE, or INSERT after THEN");
    }
    m.complete(p, MERGE_WHEN);
}

// ---- DDL (Phase 7) ----

/// `IF [NOT] EXISTS`, tolerated wherever Snowflake allows it.
fn if_exists_clause(p: &mut Parser) {
    if p.at(IF_KW) {
        p.bump(IF_KW);
        p.eat(NOT_KW);
        p.eat(EXISTS_KW);
    }
}

fn create_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(CREATE_KW);
    if p.at(OR_KW) {
        p.bump(OR_KW);
        p.expect(REPLACE_KW);
    }
    // Modifiers before the object kind (SECURE / TEMPORARY / TRANSIENT / MATERIALIZED / ...). Stop
    // at a query/body `AS` so `create_other` can leave body-bearing creates (e.g. CREATE TASK)
    // verbatim instead of swallowing the body into this flat run.
    while !p.at(TABLE_KW)
        && !p.at(VIEW_KW)
        && !p.at(PROCEDURE_KW)
        && !p.at(FUNCTION_KW)
        && !p.at(SEMICOLON)
        && !at_create_body(p)
        && !p.at_eof()
    {
        p.bump_any();
    }
    if p.at(VIEW_KW) {
        create_view(p);
    } else if p.at(TABLE_KW) {
        create_table(p);
    } else if p.at(PROCEDURE_KW) || p.at(FUNCTION_KW) {
        create_routine(p);
    } else {
        create_other(p);
    }
    m.complete(p, CREATE_STMT);
}

/// Object kinds without a query body — `CREATE SCHEMA/DATABASE/WAREHOUSE/SEQUENCE/STAGE/FILE
/// FORMAT/ROLE/…` — parsed leniently as a flat token run so they round-trip and get inline spacing
/// (like [`alter_stmt`]). If the statement carries an `AS <body>` (e.g. `CREATE TASK … AS <dml>`),
/// bail to an error so it passes through verbatim rather than being flattened into one line — the
/// formatter cannot lay out a body it has not parsed structurally.
fn create_other(p: &mut Parser) {
    while !p.at(SEMICOLON) && !p.at_eof() {
        if at_create_body(p) {
            p.error("CREATE ... AS <body> is not yet formatted; left verbatim");
            while !p.at(SEMICOLON) && !p.at_eof() {
                p.bump_any();
            }
            return;
        }
        p.bump_any();
    }
}

/// At an `AS` that introduces a statement/query body (a task's DML, a dynamic-table query, a
/// procedural block) rather than an inline option like `CREATE DATABASE d AS REPLICA OF …`.
fn at_create_body(p: &Parser) -> bool {
    p.at(AS_KW)
        && (p.nth_at(1, SELECT_KW)
            || p.nth_at(1, WITH_KW)
            || p.nth_at(1, VALUES_KW)
            || p.nth_at(1, INSERT_KW)
            || p.nth_at(1, UPDATE_KW)
            || p.nth_at(1, DELETE_KW)
            || p.nth_at(1, MERGE_KW)
            || p.nth_at(1, CALL_KW)
            || p.nth_at(1, BEGIN_KW)
            || p.nth_at(1, L_PAREN))
}

/// `CREATE ... PROCEDURE/FUNCTION name (params) RETURNS ... <options> AS <body>`.
///
/// Skeleton support (Phase 8): the signature/options are kept leniently as tokens and the body is
/// the lexer's single delimited token (`$$ … $$` or a quoted string), preserved verbatim. Only the
/// delimited-body form parses cleanly; an unquoted scripting body (`AS BEGIN …; … END`) is left to
/// error so the statement passes through untouched rather than being mis-split on its inner `;`.
fn create_routine(p: &mut Parser) {
    p.bump_any(); // PROCEDURE or FUNCTION
    name_ref(p);
    if p.at(L_PAREN) {
        column_def_list(p); // parameter list, parsed leniently like column defs
    }
    // RETURNS / LANGUAGE / RUNTIME_VERSION / PACKAGES / HANDLER / EXECUTE AS / ... up to `AS <body>`.
    while !at_routine_body(p) && !p.at(SEMICOLON) && !p.at_eof() {
        p.bump_any();
    }
    if at_routine_body(p) {
        p.bump(AS_KW);
        p.bump_any(); // the delimited body token
    } else {
        p.error("expected a delimited routine body (AS $$ … $$ or AS '…')");
    }
}

/// At `AS` immediately followed by a delimited body token (so we don't stop on `EXECUTE AS`).
fn at_routine_body(p: &Parser) -> bool {
    p.at(AS_KW) && (p.nth_at(1, DOLLAR_STRING) || p.nth_at(1, STRING))
}

// ---- COPY INTO (Phase 6) ----

/// `COPY INTO <target> FROM <source> <option>*` (both the load and unload shapes). The location
/// operands (`@stage/path`, table names) are captured verbatim — stage paths use `/` which would be
/// mangled by operator spacing — while options are parsed as `name = value` (or `PARTITION BY (...)`)
/// so each can sit on its own line.
fn copy_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(COPY_KW);
    p.expect(INTO_KW);
    copy_operand(p);
    if p.eat(FROM_KW) {
        copy_operand(p);
    }
    while !p.at(SEMICOLON) && !p.at_eof() {
        copy_option(p);
    }
    m.complete(p, COPY_STMT);
}

/// A COPY target/source: a parenthesized query, or a location captured as a verbatim token run up
/// to `FROM`, the first option, or the statement end.
fn copy_operand(p: &mut Parser) {
    if p.at(L_PAREN) {
        subquery(p);
        return;
    }
    let m = p.start();
    while !p.at(FROM_KW) && !p.at(SEMICOLON) && !p.at_eof() && !at_copy_option_start(p) {
        p.bump_any();
    }
    m.complete(p, COPY_LOCATION);
}

/// A COPY option starts at `PARTITION BY` or any word immediately followed by `=`.
fn at_copy_option_start(p: &Parser) -> bool {
    p.at(PARTITION_KW) || p.nth_at(1, EQ)
}

fn copy_option(p: &mut Parser) {
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
            } else {
                p.bump_any(); // a single literal / bare word value
            }
        }
    }
    m.complete(p, COPY_OPTION);
}

// ---- session SET / EXECUTE IMMEDIATE ----

/// `SET <name> = <expr>` or `SET (<name>, ...) = (<expr>, ...)` (session variables).
fn set_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(SET_KW);
    if p.at(L_PAREN) {
        column_list(p);
    } else {
        name_ref(p);
    }
    p.expect(EQ);
    if p.at(L_PAREN) {
        // A tuple / subquery right-hand side.
        p.bump(L_PAREN);
        if p.at(SELECT_KW) || p.at(WITH_KW) {
            query_expr(p);
        } else if !p.at(R_PAREN) {
            expr_list(p);
        }
        p.expect(R_PAREN);
    } else {
        expr(p);
    }
    m.complete(p, SET_STMT);
}

/// `EXECUTE IMMEDIATE <string|$$…$$|:var> [USING (<binds>)]`.
fn execute_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(EXECUTE_KW);
    p.expect(IMMEDIATE_KW);
    expr(p);
    if p.eat(USING_KW) {
        if p.at(L_PAREN) {
            p.bump(L_PAREN);
            if !p.at(R_PAREN) {
                expr_list(p);
            }
            p.expect(R_PAREN);
        } else {
            expr(p);
            while p.eat(COMMA) {
                expr(p);
            }
        }
    }
    m.complete(p, EXECUTE_STMT);
}

fn create_view(p: &mut Parser) {
    p.bump(VIEW_KW);
    if_exists_clause(p);
    name_ref(p);
    if p.at(L_PAREN) {
        column_list(p);
    }
    // Tolerate view options (COMMENT = '...', masking policies, ...) up to the defining query.
    while !p.at(AS_KW) && !p.at(SELECT_KW) && !p.at(WITH_KW) && !p.at(SEMICOLON) && !p.at_eof() {
        p.bump_any();
    }
    p.eat(AS_KW);
    if p.at(SELECT_KW) || p.at(WITH_KW) || p.at(VALUES_KW) || p.at(L_PAREN) {
        query_expr(p);
    }
}

fn create_table(p: &mut Parser) {
    p.bump(TABLE_KW);
    if_exists_clause(p);
    name_ref(p);
    if p.at(L_PAREN) {
        column_def_list(p);
    }
    // Tolerate table options (CLUSTER BY (...), COMMENT = '...', ...) up to an optional CTAS query.
    while !p.at(AS_KW) && !p.at(SEMICOLON) && !p.at_eof() {
        p.bump_any();
    }
    if p.eat(AS_KW) {
        query_expr(p);
    }
}

fn column_def_list(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    if !p.at(R_PAREN) {
        column_def(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            column_def(p);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, COLUMN_DEF_LIST);
}

/// A column definition or table constraint, captured leniently as `name type constraints...` up to
/// the next top-level comma or the closing paren (balanced inner parens for `NUMBER(10,2)` etc.).
fn column_def(p: &mut Parser) {
    let m = p.start();
    let mut depth = 0u32;
    while !p.at_eof() {
        if depth == 0 && (p.at(COMMA) || p.at(R_PAREN)) {
            break;
        }
        if p.at(L_PAREN) {
            depth += 1;
        } else if p.at(R_PAREN) {
            depth -= 1;
        }
        p.bump_any();
    }
    m.complete(p, COLUMN_DEF);
}

fn drop_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(DROP_KW);
    // Object kind (TABLE / VIEW / SCHEMA / ...).
    if !p.at(SEMICOLON) && !p.at_eof() {
        p.bump_any();
    }
    if_exists_clause(p);
    if p.at_name() {
        name_ref(p);
    }
    // Tolerate trailing options (CASCADE / RESTRICT / ...).
    while !p.at(SEMICOLON) && !p.at_eof() {
        p.bump_any();
    }
    m.complete(p, DROP_STMT);
}

/// `ALTER` has enormous surface; parse it leniently as a flat token run so it round-trips and gets
/// inline formatting rather than erroring the whole file.
fn alter_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(ALTER_KW);
    while !p.at(SEMICOLON) && !p.at_eof() {
        p.bump_any();
    }
    m.complete(p, ALTER_STMT);
}

/// Parse the leading keyword and the rest of the statement as a flat token run, completing it as
/// `node`. Used for statements whose surface is large/evolving (GRANT, REVOKE) or simple enough that
/// inline token rendering is all the formatter needs (USE, SHOW, DESCRIBE, TRUNCATE): the result
/// round-trips losslessly and gets inline spacing normalization rather than erroring the file.
fn lenient_stmt(p: &mut Parser, node: SyntaxKind) {
    let m = p.start();
    p.bump_any(); // the leading statement keyword
    while !p.at(SEMICOLON) && !p.at_eof() {
        p.bump_any();
    }
    m.complete(p, node);
}

/// At a `COMMENT ON …` statement. `comment` is a contextual keyword recognized only before `ON`, so
/// the very common `comment` column/identifier is never mistaken for this statement.
fn at_comment_stmt(p: &Parser) -> bool {
    p.nth_contextual(0, ContextualKeyword::Comment) && p.nth_at(1, ON_KW)
}

/// At a transaction-starting `BEGIN` (`BEGIN;`, `BEGIN TRANSACTION …`, `BEGIN WORK`) — as opposed to
/// a Snowflake Scripting block (`BEGIN <stmt>; … END`). Only the transaction form is recognized (so
/// it formats inline); a scripting block is left to pass through verbatim, its inner `;`-separated
/// statements never mis-split. `BEGIN NAME …` is intentionally not matched (rarer, and `name` is a
/// common identifier).
fn at_begin_transaction(p: &Parser) -> bool {
    p.at(BEGIN_KW)
        && (p.nth_at(1, SEMICOLON)
            || p.nth_contextual(1, ContextualKeyword::Transaction)
            || p.nth_contextual(1, ContextualKeyword::Work))
}

/// `COMMENT ON <object> IS '<text>'` (or `COMMENT IF EXISTS …`). Parsed leniently as a flat token
/// run after up-casing the contextual `COMMENT`, so it round-trips and formats inline.
fn comment_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // COMMENT
    while !p.at(SEMICOLON) && !p.at_eof() {
        p.bump_any();
    }
    m.complete(p, COMMENT_STMT);
}

/// `CALL proc(args)` — invoke a stored procedure. The invocation is an ordinary call expression, so
/// its argument list is formatted like any other (one-per-line when it overflows). A trailing tail
/// (e.g. `INTO :result`) is kept leniently as tokens so it round-trips.
fn call_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(CALL_KW);
    expr(p); // the procedure-call expression: name(args)
    while !p.at(SEMICOLON) && !p.at_eof() {
        p.bump_any();
    }
    m.complete(p, CALL_STMT);
}

// ---- Snowflake Scripting blocks (Phase 8) ----

/// A scripting block starts at `DECLARE`, or at a `BEGIN` that is not a transaction start.
fn at_block_start(p: &Parser) -> bool {
    p.at(DECLARE_KW) || (p.at(BEGIN_KW) && !at_begin_transaction(p))
}

/// `[DECLARE <decls>] BEGIN <body> [EXCEPTION <handlers>] END [<label>]` — a Snowflake Scripting
/// block. The body and handler bodies are statement sequences (`STMT_LIST`); control-flow statements
/// (IF / loops) are structured and everything else is kept as a lenient inline statement, so the
/// block round-trips losslessly even where a construct is not modeled in detail.
fn block_stmt(p: &mut Parser) {
    let m = p.start();
    if p.at(DECLARE_KW) {
        declare_section(p);
    }
    p.expect(BEGIN_KW);
    stmt_list(p, |p| p.at(END_KW) || p.at(EXCEPTION_KW));
    if p.at(EXCEPTION_KW) {
        exception_section(p);
    }
    p.expect(END_KW);
    if p.at_name() {
        name_ref(p); // optional label after END
    }
    m.complete(p, BLOCK_STMT);
}

/// `DECLARE <decl>; <decl>; …` — each declaration kept leniently as a token run up to its `;` (a
/// cursor/resultset declaration's inner query has no top-level `;`, so this is safe).
fn declare_section(p: &mut Parser) {
    let m = p.start();
    p.bump(DECLARE_KW);
    while !p.at(BEGIN_KW) && !p.at_eof() {
        if p.eat(SEMICOLON) {
            continue;
        }
        declare_item(p);
        p.eat(SEMICOLON);
    }
    m.complete(p, DECLARE_SECTION);
}

fn declare_item(p: &mut Parser) {
    let m = p.start();
    let mut first = true;
    while !p.at(SEMICOLON) && !p.at(BEGIN_KW) && !p.at_eof() {
        // `<name> [<type>] DEFAULT <expr>` / cursor `… FOR <query>`: up-case the `DEFAULT` value
        // word (not reserved — `default` is a common identifier, so never the declared name in the
        // first position) and keep the rest verbatim.
        if !first && p.nth_contextual(0, ContextualKeyword::Default) {
            p.bump_as(CONTEXTUAL_KEYWORD);
        } else {
            p.bump_any();
        }
        first = false;
    }
    m.complete(p, DECLARE_ITEM);
}

/// A sequence of scripting statements, each terminated by `;`, until `is_end` holds. Wrapped in a
/// `STMT_LIST` so the formatter can indent the whole body as one unit.
fn stmt_list(p: &mut Parser, is_end: impl Fn(&Parser) -> bool) {
    let m = p.start();
    while !is_end(p) && !p.at_eof() {
        if p.eat(SEMICOLON) {
            continue; // a stray/empty `;`
        }
        block_statement(p);
        p.eat(SEMICOLON);
    }
    m.complete(p, STMT_LIST);
}

/// One statement inside a scripting block: a structured control-flow construct, a nested block, or a
/// lenient inline statement (LET / RETURN / assignment / a SQL statement / anything else up to `;`).
fn block_statement(p: &mut Parser) {
    if p.at(IF_KW) {
        if_stmt(p);
    } else if p.at(FOR_KW) || p.at(WHILE_KW) || p.at(LOOP_KW) || p.at(REPEAT_KW) {
        loop_stmt(p);
    } else if at_block_start(p) {
        block_stmt(p); // nested DECLARE…/BEGIN…END
    } else if p.at(CASE_KW) {
        // CASE statement: kept as one balanced token run (rendered inline) — not yet pretty-printed.
        balanced_construct(p, SCRIPT_STMT);
    } else if p.at(LET_KW) {
        simple_script_stmt(p, LET_STMT);
    } else if p.at(RETURN_KW) {
        simple_script_stmt(p, RETURN_STMT);
    } else if at_sql_statement_start(p) {
        statement(p);
    } else if p.at_name() && p.nth_at(1, ASSIGN) {
        simple_script_stmt(p, ASSIGN_STMT);
    } else {
        simple_script_stmt(p, SCRIPT_STMT);
    }
}

/// The SQL statements that the top-level [`statement`] dispatcher handles well, recognized so a
/// scripting block can delegate to it (and get full structural formatting of nested SQL).
fn at_sql_statement_start(p: &Parser) -> bool {
    p.at(WITH_KW)
        || p.at(SELECT_KW)
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
        || p.at(COMMIT_KW)
        || p.at(ROLLBACK_KW)
        || p.at(UNDROP_KW)
        || at_comment_stmt(p)
        || p.at(CALL_KW)
        || p.at(SET_KW)
        || p.at(EXECUTE_KW)
        || p.at(COPY_KW)
        || at_begin_transaction(p)
}

/// A lenient scripting statement: consume tokens up to (but not including) the terminating `;`. Every
/// Snowflake Scripting statement ends with `;`, so this captures the whole statement — including an
/// expression `CASE … END` on the right of a `LET`/assignment — without mis-splitting.
fn simple_script_stmt(p: &mut Parser, node: SyntaxKind) {
    let m = p.start();
    let mut first = true;
    while !p.at(SEMICOLON) && !p.at_eof() {
        // Up-case the scripting structural words that are not reserved, but only in a position where
        // the word cannot be an identifier — so a variable literally named `default`/`break` is left
        // alone. `DEFAULT` introduces a value and never starts a statement; `BREAK`/`CONTINUE` are
        // whole statements (the only/first token). Everything else is kept verbatim.
        let up = if first {
            p.nth_contextual(0, ContextualKeyword::Break)
                || p.nth_contextual(0, ContextualKeyword::Continue)
        } else {
            p.nth_contextual(0, ContextualKeyword::Default) && !p.nth_at(1, ASSIGN)
        };
        if up {
            p.bump_as(CONTEXTUAL_KEYWORD);
        } else {
            p.bump_any();
        }
        first = false;
    }
    m.complete(p, node);
}

/// `IF <cond> THEN <body> [ELSEIF <cond> THEN <body>]… [ELSE <body>] END IF`.
fn if_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(IF_KW);
    expr(p); // condition (parenthesized or bare)
    p.expect(THEN_KW);
    stmt_list(p, |p| p.at(ELSEIF_KW) || p.at(ELSE_KW) || p.at(END_KW));
    while p.at(ELSEIF_KW) {
        p.bump(ELSEIF_KW);
        expr(p);
        p.expect(THEN_KW);
        stmt_list(p, |p| p.at(ELSEIF_KW) || p.at(ELSE_KW) || p.at(END_KW));
    }
    if p.eat(ELSE_KW) {
        stmt_list(p, |p| p.at(END_KW));
    }
    p.expect(END_KW);
    p.expect(IF_KW);
    m.complete(p, IF_STMT);
}

/// `FOR …/WHILE … DO <body> END FOR/WHILE`, `LOOP <body> END LOOP`, and
/// `REPEAT <body> UNTIL <cond> END REPEAT` — unified as one loop node.
fn loop_stmt(p: &mut Parser) {
    let m = p.start();
    if p.at(FOR_KW) || p.at(WHILE_KW) {
        p.bump_any(); // FOR / WHILE
        while !p.at(DO_KW) && !p.at(SEMICOLON) && !p.at(END_KW) && !p.at_eof() {
            // The counter-loop range words `REVERSE`/`TO` up-case in this position; everything else
            // (the counter name, bounds, cursor name, `USING (…)`, the condition) is kept verbatim.
            if p.nth_contextual(0, ContextualKeyword::Reverse)
                || p.nth_contextual(0, ContextualKeyword::To)
            {
                p.bump_as(CONTEXTUAL_KEYWORD);
            } else {
                p.bump_any(); // loop header (counter/range or cursor; condition)
            }
        }
        p.expect(DO_KW);
        stmt_list(p, |p| p.at(END_KW));
    } else if p.at(LOOP_KW) {
        p.bump(LOOP_KW);
        stmt_list(p, |p| p.at(END_KW));
    } else {
        p.bump(REPEAT_KW);
        stmt_list(p, |p| p.at(UNTIL_KW) || p.at(END_KW));
        if p.eat(UNTIL_KW) {
            expr(p); // loop condition
            while !p.at(END_KW) && !p.at(SEMICOLON) && !p.at_eof() {
                p.bump_any();
            }
        }
    }
    p.expect(END_KW);
    // The matching trailer keyword: END FOR / WHILE / LOOP / REPEAT.
    if p.at(FOR_KW) || p.at(WHILE_KW) || p.at(LOOP_KW) || p.at(REPEAT_KW) {
        p.bump_any();
    }
    m.complete(p, LOOP_STMT);
}

/// `EXCEPTION WHEN <exc> [OR <exc>]… THEN <body> …` inside a block.
fn exception_section(p: &mut Parser) {
    let m = p.start();
    p.bump(EXCEPTION_KW);
    while p.at(WHEN_KW) {
        exception_when(p);
    }
    m.complete(p, EXCEPTION_SECTION);
}

fn exception_when(p: &mut Parser) {
    let m = p.start();
    p.bump(WHEN_KW);
    while !p.at(THEN_KW) && !p.at(WHEN_KW) && !p.at(END_KW) && !p.at_eof() {
        p.bump_any(); // exception name(s) / OTHER
    }
    p.expect(THEN_KW);
    stmt_list(p, |p| p.at(WHEN_KW) || p.at(END_KW));
    m.complete(p, EXCEPTION_WHEN);
}

/// Consume a construct that opens with a block keyword (e.g. a `CASE` statement) up to its matching
/// `END [trailer]`, tracking nesting so an inner construct's `END` does not close it early. Each
/// opener (`IF`/`CASE`/`FOR`/`WHILE`/`LOOP`/`REPEAT`/`BEGIN`) is balanced by exactly one `END`.
fn balanced_construct(p: &mut Parser, node: SyntaxKind) {
    let m = p.start();
    let mut depth: u32 = 0;
    while !p.at_eof() {
        if is_block_opener(p) {
            depth += 1;
            p.bump_any();
        } else if p.at(END_KW) {
            p.bump_any();
            // Consume the optional trailer keyword so it is not re-read as an opener.
            if is_construct_trailer(p) {
                p.bump_any();
            }
            depth -= 1;
            if depth == 0 {
                break;
            }
        } else {
            p.bump_any();
        }
    }
    m.complete(p, node);
}

fn is_block_opener(p: &Parser) -> bool {
    p.at(IF_KW)
        || p.at(CASE_KW)
        || p.at(FOR_KW)
        || p.at(WHILE_KW)
        || p.at(LOOP_KW)
        || p.at(REPEAT_KW)
        || p.at(BEGIN_KW)
}

fn is_construct_trailer(p: &Parser) -> bool {
    p.at(IF_KW)
        || p.at(CASE_KW)
        || p.at(FOR_KW)
        || p.at(WHILE_KW)
        || p.at(LOOP_KW)
        || p.at(REPEAT_KW)
}

// ---- queries ----

fn with_query(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    with_clause(p);
    query_expr(p);
    m.complete(p, WITH_QUERY)
}

fn with_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(WITH_KW);
    p.eat(RECURSIVE_KW);
    cte(p);
    while p.eat(COMMA) {
        cte(p);
    }
    m.complete(p, WITH_CLAUSE);
}

fn cte(p: &mut Parser) {
    let m = p.start();
    name(p);
    if p.at(L_PAREN) {
        column_list(p);
    }
    p.expect(AS_KW);
    subquery(p);
    m.complete(p, CTE);
}

/// A query expression: query primaries combined by left-associative set operations.
fn query_expr(p: &mut Parser) -> Option<CompletedMarker> {
    let mut lhs = query_primary(p)?;
    while p.at(UNION_KW) || p.at(EXCEPT_KW) || p.at(INTERSECT_KW) || p.at(MINUS_KW) {
        let m = lhs.precede(p);
        p.bump_any(); // the set operator
        if p.at(ALL_KW) {
            p.bump(ALL_KW);
        } else if p.at(DISTINCT_KW) {
            p.bump(DISTINCT_KW);
        }
        query_primary(p);
        lhs = m.complete(p, SET_OP);
    }
    Some(lhs)
}

fn query_primary(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at(L_PAREN) {
        Some(subquery(p))
    } else if p.at(SELECT_KW) {
        Some(select_core(p))
    } else if p.at(WITH_KW) {
        Some(with_query(p)) // a CTE query is a valid (sub)query: `(WITH ... SELECT ...)`, `AS WITH ...`
    } else if p.at(VALUES_KW) {
        Some(values_clause(p))
    } else {
        p.error("expected a query (SELECT, VALUES, WITH, or a parenthesized subquery)");
        None
    }
}

fn subquery(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect(L_PAREN);
    query_expr(p);
    p.expect(R_PAREN);
    m.complete(p, SUBQUERY)
}

fn select_core(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    if p.at(SELECT_KW) {
        p.bump(SELECT_KW);
    } else {
        p.error("expected SELECT");
    }
    if p.at(DISTINCT_KW) {
        p.bump(DISTINCT_KW);
    } else if p.at(ALL_KW) {
        p.bump(ALL_KW);
    }
    select_list(p);
    if p.at(FROM_KW) {
        from_clause(p);
    }
    if p.at(WHERE_KW) {
        where_clause(p);
    }
    // Hierarchical queries: `START WITH` / `CONNECT BY` in either order.
    while p.at(START_KW) || p.at(CONNECT_KW) {
        if p.at(START_KW) {
            start_with_clause(p);
        } else {
            connect_by_clause(p);
        }
    }
    if p.at(GROUP_KW) {
        group_by_clause(p);
    }
    if p.at(HAVING_KW) {
        having_clause(p);
    }
    if p.at(QUALIFY_KW) {
        qualify_clause(p);
    }
    if p.at(ORDER_KW) {
        order_by_clause(p);
    }
    if p.at(LIMIT_KW) {
        limit_clause(p);
    }
    if p.at(OFFSET_KW) {
        offset_clause(p);
    }
    m.complete(p, SELECT_STMT)
}

// ---- SELECT list ----

fn select_list(p: &mut Parser) {
    let m = p.start();
    select_item(p);
    while p.eat(COMMA) {
        if p.at_eof() || at_clause_end(p) {
            break; // tolerate a trailing comma
        }
        select_item(p);
    }
    m.complete(p, SELECT_LIST);
}

/// Keywords that end the select list (so a stray trailing comma doesn't eat them).
fn at_clause_end(p: &Parser) -> bool {
    p.at(FROM_KW)
        || p.at(WHERE_KW)
        || p.at(GROUP_KW)
        || p.at(HAVING_KW)
        || p.at(QUALIFY_KW)
        || p.at(ORDER_KW)
        || p.at(LIMIT_KW)
        || p.at(OFFSET_KW)
}

fn select_item(p: &mut Parser) {
    let m = p.start();
    if p.at(STAR) {
        let s = p.start();
        p.bump(STAR);
        s.complete(p, STAR_EXPR);
    } else if at_expr_start(p) {
        expr(p);
        let explicit_alias = p.eat(AS_KW);
        if explicit_alias || p.at_name() {
            name(p); // implicit alias: SELECT a alias
        }
    } else {
        p.error("expected a select item");
    }
    m.complete(p, SELECT_ITEM);
}

// ---- FROM / JOIN ----

fn from_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(FROM_KW);
    table_ref(p);
    loop {
        if at_join_start(p) {
            join(p);
        } else if p.eat(COMMA) {
            table_ref(p);
        } else {
            break;
        }
    }
    m.complete(p, FROM_CLAUSE);
}

fn table_ref(p: &mut Parser) {
    let m = p.start();
    p.eat(LATERAL_KW); // LATERAL FLATTEN(...) / LATERAL (subquery)
    if p.at(L_PAREN) {
        subquery(p); // derived table
    } else if p.at(VALUES_KW) {
        values_clause(p); // FROM VALUES (...), (...) [AS t(c1, c2)]
    } else if p.at(FLATTEN_KW) || p.at(TABLE_KW) {
        // Keyword-named table function: FLATTEN(...) / TABLE(...).
        p.bump_any();
        if p.at(L_PAREN) {
            arg_list(p);
        } else {
            p.error("expected '(' after table function");
        }
    } else if p.at(VARIABLE) {
        // Flow-operator back-reference: `FROM $1` points at a previous statement in the chain.
        let r = p.start();
        p.bump(VARIABLE);
        r.complete(p, NAME_REF);
    } else if p.at_name() {
        name_ref(p);
        if p.at(L_PAREN) {
            arg_list(p); // table function: my_udtf(args)
        }
    } else {
        p.error("expected a table reference");
    }
    // Change-tracking, time travel, SAMPLE / TABLESAMPLE, MATCH_RECOGNIZE, and PIVOT / UNPIVOT all
    // attach to the table before its alias.
    if p.nth_contextual(0, ContextualKeyword::Changes) {
        // `CHANGES ( INFORMATION => ... )` then AT|BEFORE (below) and an optional END ( ... ).
        p.bump_as(CONTEXTUAL_KEYWORD); // CHANGES
        if p.at(L_PAREN) {
            balanced_parens(p);
        }
    }
    if at_time_travel(p) {
        time_travel(p);
    }
    if p.at(END_KW) && p.nth_at(1, L_PAREN) {
        p.bump(END_KW); // CHANGES ... END ( TIMESTAMP => ... )
        balanced_parens(p);
    }
    if p.at(SAMPLE_KW) || p.at(TABLESAMPLE_KW) {
        sample_clause(p);
    }
    if p.nth_contextual(0, ContextualKeyword::MatchRecognize) {
        match_recognize(p);
    }
    while p.at(PIVOT_KW) || p.at(UNPIVOT_KW) {
        pivot_clause(p);
    }
    table_alias(p);
    m.complete(p, TABLE_REF);
}

/// Time-travel: `AT ( ... )` / `BEFORE ( ... )` (`at`/`before` are contextual keywords).
fn at_time_travel(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::At) || p.nth_contextual(0, ContextualKeyword::Before))
        && p.nth_at(1, L_PAREN)
}

/// `<table> {AT|BEFORE} ( TIMESTAMP|OFFSET|STATEMENT => ... )`, captured leniently.
fn time_travel(p: &mut Parser) {
    p.bump_as(CONTEXTUAL_KEYWORD); // AT / BEFORE (contextual keyword)
    if p.at(L_PAREN) {
        balanced_parens(p);
    }
}

/// `<table> {SAMPLE|TABLESAMPLE} [method] ( n [ROWS] ) [REPEATABLE|SEED ( seed )]`. The fraction
/// and any method/seed are captured leniently (balanced parens) for inline formatting.
fn sample_clause(p: &mut Parser) {
    p.bump_any(); // SAMPLE / TABLESAMPLE
                  // Sampling method: BERNOULLI / SYSTEM / BLOCK are plain words; ROW is the reserved keyword
                  // `ROW_KW` (so `p.at_name()` is false for it) — accept it explicitly. Guard the `(` so a bare
                  // `SAMPLE (10)` (no method) and a `ROW`-without-parens both stay total.
    if p.at(ROW_KW) && p.nth_at(1, L_PAREN) {
        p.bump(ROW_KW);
    } else if p.at_name() {
        name_ref(p);
    }
    if p.at(L_PAREN) {
        balanced_parens(p);
    }
    // Optional REPEATABLE(seed) / SEED(seed).
    if p.at_name() && p.nth_at(1, L_PAREN) {
        name_ref(p);
        balanced_parens(p);
    }
}

/// `<table> MATCH_RECOGNIZE ( <body> )`. The body's clauses appear in a fixed order
/// (PARTITION BY / ORDER BY / MEASURES / {ONE ROW|ALL ROWS} PER MATCH / AFTER MATCH SKIP /
/// PATTERN / SUBSET / DEFINE) but are parsed resiliently: dispatch on the clause-introducing word
/// and, for anything unrecognized, consume one token so the rule stays total and lossless. The
/// `MATCH_RECOGNIZE` word and the body keywords (MEASURES/PATTERN/DEFINE/…) are contextual.
fn match_recognize(p: &mut Parser) {
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

/// `MEASURES <expr> [AS] <alias> [, ...]` (reusing the select-item shape: expression + optional
/// alias). `FINAL`/`RUNNING` measure prefixes are not modelled yet; they parse leniently.
fn measures_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // MEASURES
    select_item(p);
    while p.eat(COMMA) {
        select_item(p);
    }
    m.complete(p, MEASURES_CLAUSE);
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

fn pivot_clause(p: &mut Parser) {
    let m = p.start();
    let is_pivot = p.at(PIVOT_KW);
    p.bump_any(); // PIVOT or UNPIVOT
    p.expect(L_PAREN);
    if is_pivot {
        // PIVOT ( <agg>(col) FOR col IN ( value, ... ) )
        expr(p);
    } else {
        // UNPIVOT ( value_col FOR name_col IN ( col, ... ) )
        name_ref(p);
    }
    p.expect(FOR_KW);
    name_ref(p);
    p.expect(IN_KW);
    p.expect(L_PAREN);
    if !p.at(R_PAREN) {
        pivot_value(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            pivot_value(p);
        }
    }
    p.expect(R_PAREN);
    p.expect(R_PAREN);
    m.complete(p, PIVOT_CLAUSE);
}

/// A PIVOT `IN` value: an expression with an optional alias (`1 AS JAN`, `'jan' AS January`).
fn pivot_value(p: &mut Parser) {
    expr(p);
    if p.eat(AS_KW) || p.at_name() {
        name(p);
    }
}

fn table_alias(p: &mut Parser) {
    let explicit_alias = p.eat(AS_KW);
    if explicit_alias || (p.at_name() && !at_alias_blocker(p)) {
        name(p);
        if p.at(L_PAREN) {
            column_list(p); // derived-table column aliases: (c1, c2, ...)
        }
    }
}

/// A contextual word that follows a table but introduces a clause rather than being its alias:
/// `ASOF JOIN`, `MATCH_CONDITION (...)`. (`MATCH_RECOGNIZE` is consumed before the alias already.)
fn at_alias_blocker(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::Asof) && p.nth_at(1, JOIN_KW))
        || (p.nth_contextual(0, ContextualKeyword::MatchCondition) && p.nth_at(1, L_PAREN))
        || at_time_travel(p)
}

fn at_join_start(p: &Parser) -> bool {
    p.at(JOIN_KW)
        || p.at(INNER_KW)
        || p.at(LEFT_KW)
        || p.at(RIGHT_KW)
        || p.at(FULL_KW)
        || p.at(CROSS_KW)
        || p.at(NATURAL_KW)
        || (p.nth_contextual(0, ContextualKeyword::Asof) && p.nth_at(1, JOIN_KW))
}

fn join(p: &mut Parser) {
    let m = p.start();
    p.eat(NATURAL_KW);
    if p.nth_contextual(0, ContextualKeyword::Asof) {
        p.bump_as(CONTEXTUAL_KEYWORD); // ASOF (contextual keyword)
    } else if p.at(INNER_KW) {
        p.bump(INNER_KW);
    } else if p.at(LEFT_KW) || p.at(RIGHT_KW) || p.at(FULL_KW) {
        p.bump_any();
        p.eat(OUTER_KW);
    } else if p.at(CROSS_KW) {
        p.bump(CROSS_KW);
    }
    p.expect(JOIN_KW);
    table_ref(p);
    // ASOF joins carry a MATCH_CONDITION ( <predicate> ) before any ON.
    if p.nth_contextual(0, ContextualKeyword::MatchCondition) {
        p.bump_as(CONTEXTUAL_KEYWORD); // MATCH_CONDITION (contextual keyword)
        p.expect(L_PAREN);
        expr(p);
        p.expect(R_PAREN);
    }
    if p.eat(ON_KW) {
        expr(p);
    } else if p.eat(USING_KW) {
        column_list(p);
    }
    m.complete(p, JOIN);
}

// ---- other clauses ----

fn where_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(WHERE_KW);
    expr(p);
    m.complete(p, WHERE_CLAUSE);
}

/// `START WITH <predicate>` — the seed of a hierarchical (`CONNECT BY`) query.
fn start_with_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(START_KW);
    p.expect(WITH_KW);
    expr(p);
    m.complete(p, START_WITH_CLAUSE);
}

/// `CONNECT BY [NOCYCLE] <predicate>` (the predicate uses the `PRIOR` prefix to refer to the parent
/// row).
fn connect_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(CONNECT_KW);
    p.expect(BY_KW);
    if p.nth_contextual(0, ContextualKeyword::NoCycle) {
        p.bump_as(CONTEXTUAL_KEYWORD); // NOCYCLE
    }
    expr(p);
    m.complete(p, CONNECT_BY_CLAUSE);
}

fn group_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(GROUP_KW);
    p.expect(BY_KW);
    if p.at(ALL_KW) {
        p.bump(ALL_KW);
    } else {
        grouping_element(p);
        while p.eat(COMMA) {
            grouping_element(p);
        }
    }
    m.complete(p, GROUP_BY_CLAUSE);
}

/// A `GROUP BY` element: `GROUPING SETS (...)`, or an ordinary expression (which already covers
/// `CUBE(...)` / `ROLLUP(...)`, parsed as function calls).
fn grouping_element(p: &mut Parser) {
    if p.nth_contextual(0, ContextualKeyword::Grouping)
        && p.nth_contextual(1, ContextualKeyword::Sets)
    {
        let m = p.start();
        p.bump_as(CONTEXTUAL_KEYWORD); // GROUPING (contextual keyword)
        p.bump_as(CONTEXTUAL_KEYWORD); // SETS (contextual keyword)
        p.expect(L_PAREN);
        if !p.at(R_PAREN) {
            grouping_set(p);
            while p.eat(COMMA) {
                if p.at(R_PAREN) {
                    break;
                }
                grouping_set(p);
            }
        }
        p.expect(R_PAREN);
        m.complete(p, GROUPING_SETS);
    } else {
        expr(p);
    }
}

/// One set inside `GROUPING SETS`: a parenthesized (possibly empty) tuple of expressions, or a
/// single bare expression.
fn grouping_set(p: &mut Parser) {
    if p.at(L_PAREN) {
        p.bump(L_PAREN);
        if !p.at(R_PAREN) {
            expr(p);
            while p.eat(COMMA) {
                if p.at(R_PAREN) {
                    break;
                }
                expr(p);
            }
        }
        p.expect(R_PAREN);
    } else {
        expr(p);
    }
}

fn having_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(HAVING_KW);
    expr(p);
    m.complete(p, HAVING_CLAUSE);
}

fn qualify_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(QUALIFY_KW);
    expr(p);
    m.complete(p, QUALIFY_CLAUSE);
}

fn order_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(ORDER_KW);
    p.expect(BY_KW);
    order_by_item(p);
    while p.eat(COMMA) {
        order_by_item(p);
    }
    m.complete(p, ORDER_BY_CLAUSE);
}

fn order_by_item(p: &mut Parser) {
    let m = p.start();
    expr(p);
    if p.at(ASC_KW) || p.at(DESC_KW) {
        p.bump_any();
    }
    if p.at(NULLS_KW) {
        p.bump(NULLS_KW);
        if p.at(FIRST_KW) || p.at(LAST_KW) {
            p.bump_any();
        } else {
            p.error("expected FIRST or LAST after NULLS");
        }
    }
    m.complete(p, ORDER_BY_ITEM);
}

fn limit_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(LIMIT_KW);
    expr(p);
    m.complete(p, LIMIT_CLAUSE);
}

fn offset_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(OFFSET_KW);
    expr(p);
    m.complete(p, OFFSET_CLAUSE);
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

// ---- expressions (Pratt) ----

fn at_expr_start(p: &Parser) -> bool {
    p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(STRING)
        || p.at(DOLLAR_STRING)
        || p.at(TRUE_KW)
        || p.at(FALSE_KW)
        || p.at(NULL_KW)
        || p.at(VARIABLE)
        || p.at(L_PAREN)
        || p.at(MINUS)
        || p.at(PLUS)
        || p.at(NOT_KW)
        || p.at(EXISTS_KW)
        || p.at(CASE_KW)
        || p.at(CAST_KW)
        || p.at(TRY_CAST_KW)
        || p.at(FLATTEN_KW)
        || p.at_name()
        || (p.at_keyword() && p.nth_at(1, L_PAREN)) // a keyword used as a function name: first(x)
}

pub(crate) fn expr(p: &mut Parser) {
    expr_bp(p, 0);
}

fn expr_bp(p: &mut Parser, min_bp: u8) -> Option<CompletedMarker> {
    let mut lhs = lhs(p)?;
    loop {
        // Postfix operators bind tightest and always apply when present.
        if p.at(L_PAREN) {
            let m = lhs.precede(p);
            arg_list(p);
            lhs = m.complete(p, CALL_EXPR);
            continue;
        }
        if p.at(L_BRACKET) {
            let m = lhs.precede(p);
            p.bump(L_BRACKET);
            expr(p);
            p.expect(R_BRACKET);
            lhs = m.complete(p, INDEX_EXPR);
            continue;
        }
        if p.at(COLON2) {
            let m = lhs.precede(p);
            p.bump(COLON2);
            type_name(p);
            lhs = m.complete(p, CAST_EXPR);
            continue;
        }
        // Semi-structured / VARIANT path access: `col:path.to.field`, `col:a[0]:b`.
        if p.at(COLON) {
            let m = lhs.precede(p);
            json_path(p);
            lhs = m.complete(p, JSON_ACCESS);
            continue;
        }
        // Window functions: `f(...) OVER (...)` or `f(...) OVER window_name`.
        if p.at(OVER_KW) {
            let m = lhs.precede(p);
            p.bump(OVER_KW);
            if p.at(L_PAREN) {
                window_spec(p);
            } else if p.at_name() {
                name_ref(p);
            } else {
                p.error("expected a window specification");
            }
            lhs = m.complete(p, WINDOW_EXPR);
            continue;
        }
        // Ordered-set aggregates: `LISTAGG(x, ',') WITHIN GROUP (ORDER BY x)`.
        if p.at(WITHIN_KW) {
            let m = lhs.precede(p);
            p.bump(WITHIN_KW);
            p.expect(GROUP_KW);
            p.expect(L_PAREN);
            if p.at(ORDER_KW) {
                order_by_clause(p);
            }
            p.expect(R_PAREN);
            lhs = m.complete(p, WITHIN_GROUP);
            continue;
        }

        // Compound predicates, all at comparison precedence.
        if p.at(IS_KW) {
            if BP_CMP.0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.bump(IS_KW);
            p.eat(NOT_KW);
            is_rhs(p);
            lhs = m.complete(p, IS_EXPR);
            continue;
        }
        let neg = p.at(NOT_KW);
        if p.at(BETWEEN_KW) || (neg && p.nth_at(1, BETWEEN_KW)) {
            if BP_CMP.0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.eat(NOT_KW);
            p.bump(BETWEEN_KW);
            expr_bp(p, BP_CMP.1); // first bound (won't swallow the AND)
            p.expect(AND_KW);
            expr_bp(p, BP_CMP.1); // second bound
            lhs = m.complete(p, BETWEEN_EXPR);
            continue;
        }
        if p.at(IN_KW) || (neg && p.nth_at(1, IN_KW)) {
            if BP_CMP.0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.eat(NOT_KW);
            p.bump(IN_KW);
            in_rhs(p);
            lhs = m.complete(p, IN_EXPR);
            continue;
        }
        if neg && (p.nth_at(1, LIKE_KW) || p.nth_at(1, ILIKE_KW)) {
            if BP_CMP.0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.bump(NOT_KW);
            p.bump_any(); // LIKE / ILIKE
            expr_bp(p, BP_CMP.1);
            lhs = m.complete(p, BIN_EXPR);
            continue;
        }

        // Generic infix binary operators.
        let (lbp, rbp) = match infix_bp(p) {
            Some(bp) => bp,
            None => break,
        };
        if lbp < min_bp {
            break;
        }
        let m = lhs.precede(p);
        p.bump_any(); // the operator
        expr_bp(p, rbp);
        lhs = m.complete(p, BIN_EXPR);
    }
    Some(lhs)
}

fn lhs(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at(NOT_KW) {
        let m = p.start();
        p.bump(NOT_KW);
        expr_bp(p, BP_PREFIX_NOT);
        return Some(m.complete(p, PREFIX_EXPR));
    }
    if p.at(PRIOR_KW) {
        // `CONNECT BY PRIOR <col> = <col>`: PRIOR is a tight unary prefix on a value.
        let m = p.start();
        p.bump(PRIOR_KW);
        expr_bp(p, BP_PREFIX_NEG);
        return Some(m.complete(p, PREFIX_EXPR));
    }
    if p.at(MINUS) || p.at(PLUS) {
        let m = p.start();
        p.bump_any();
        expr_bp(p, BP_PREFIX_NEG);
        return Some(m.complete(p, PREFIX_EXPR));
    }
    primary(p)
}

fn primary(p: &mut Parser) -> Option<CompletedMarker> {
    let cm = if p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(STRING)
        || p.at(DOLLAR_STRING)
        || p.at(TRUE_KW)
        || p.at(FALSE_KW)
        || p.at(NULL_KW)
        || p.at(VARIABLE)
    {
        let m = p.start();
        p.bump_any();
        m.complete(p, LITERAL)
    } else if p.at(EXISTS_KW) {
        let m = p.start();
        p.bump(EXISTS_KW);
        subquery(p);
        m.complete(p, EXISTS_EXPR)
    } else if p.at(L_PAREN) && (p.nth_at(1, SELECT_KW) || p.nth_at(1, WITH_KW)) {
        subquery(p) // scalar subquery
    } else if p.at(L_PAREN) {
        let m = p.start();
        p.bump(L_PAREN);
        expr(p);
        p.expect(R_PAREN);
        m.complete(p, PAREN_EXPR)
    } else if p.at(CASE_KW) {
        case_expr(p)
    } else if p.at(CAST_KW) || p.at(TRY_CAST_KW) {
        cast_fn_expr(p)
    } else if p.at(FLATTEN_KW) {
        // FLATTEN is a keyword but acts as a table/regular function; treat it as a callable name.
        let m = p.start();
        p.bump(FLATTEN_KW);
        m.complete(p, NAME_REF)
    } else if p.at_keyword() && p.nth_at(1, L_PAREN) {
        // A keyword-spelled word used as a function name (`first(x)`, `last(x)`, `left(s, 2)`):
        // tag it as a plain name so the postfix `(` makes it a CALL_EXPR and it formats like any
        // other call (lower-case, hugging its parens).
        let m = p.start();
        p.bump_as(IDENT);
        m.complete(p, NAME_REF)
    } else if p.at_name() {
        name_ref(p)
    } else {
        p.error("expected an expression");
        return None;
    };
    Some(cm)
}

fn is_rhs(p: &mut Parser) {
    if p.at(DISTINCT_KW) {
        // `a IS [NOT] DISTINCT FROM b`
        p.bump(DISTINCT_KW);
        p.expect(FROM_KW);
        expr_bp(p, BP_CMP.1);
    } else if p.at(NULL_KW) || p.at(TRUE_KW) || p.at(FALSE_KW) {
        let m = p.start();
        p.bump_any();
        m.complete(p, LITERAL);
    } else {
        expr_bp(p, BP_CMP.1);
    }
}

fn in_rhs(p: &mut Parser) {
    if !p.eat(L_PAREN) {
        p.error("expected '(' after IN");
        return;
    }
    if p.at(SELECT_KW) || p.at(WITH_KW) {
        query_expr(p);
    } else if !p.at(R_PAREN) {
        expr_list(p);
    }
    p.expect(R_PAREN);
}

fn expr_list(p: &mut Parser) {
    let m = p.start();
    expr(p);
    while p.eat(COMMA) {
        if p.at(R_PAREN) {
            break;
        }
        expr(p);
    }
    m.complete(p, EXPR_LIST);
}

fn arg_list(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    // Aggregate quantifier applying to the whole argument list: COUNT(DISTINCT x), ARRAY_AGG(ALL x).
    if p.at(DISTINCT_KW) || p.at(ALL_KW) {
        p.bump_any();
    }
    if !p.at(R_PAREN) {
        arg(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            arg(p);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, ARG_LIST);
}

fn arg(p: &mut Parser) {
    if p.at(STAR) {
        let m = p.start();
        p.bump(STAR);
        m.complete(p, STAR_EXPR);
    } else if p.at_ident_like() && p.nth_at(1, FAT_ARROW) {
        // Named argument: `name => value` (e.g. FLATTEN(INPUT => col, OUTER => TRUE)).
        let m = p.start();
        p.bump_any(); // the argument name
        p.bump(FAT_ARROW);
        expr(p);
        m.complete(p, NAMED_ARG);
    } else if at_expr_start(p) {
        expr(p);
    } else {
        p.error("expected an argument");
    }
}

fn type_name(p: &mut Parser) {
    let m = p.start();
    if p.at_name() {
        p.bump_any();
        if p.eat(L_PAREN) {
            while !p.at(R_PAREN) && !p.at_eof() {
                p.bump_any();
            }
            p.expect(R_PAREN);
        }
    } else {
        p.error("expected a type name");
    }
    m.complete(p, TYPE_NAME);
}

fn window_spec(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    if p.at(PARTITION_KW) {
        partition_by_clause(p);
    }
    if p.at(ORDER_KW) {
        order_by_clause(p);
    }
    if p.at(ROWS_KW) || p.at(RANGE_KW) {
        window_frame(p);
    }
    p.expect(R_PAREN);
    m.complete(p, WINDOW_SPEC);
}

fn partition_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(PARTITION_KW);
    p.expect(BY_KW);
    expr(p);
    while p.eat(COMMA) {
        expr(p);
    }
    m.complete(p, PARTITION_BY_CLAUSE);
}

fn window_frame(p: &mut Parser) {
    let m = p.start();
    p.bump_any(); // ROWS or RANGE
    if p.eat(BETWEEN_KW) {
        frame_bound(p);
        p.expect(AND_KW);
        frame_bound(p);
    } else {
        frame_bound(p);
    }
    m.complete(p, WINDOW_FRAME);
}

fn frame_bound(p: &mut Parser) {
    if p.at(UNBOUNDED_KW) {
        p.bump(UNBOUNDED_KW);
        if p.at(PRECEDING_KW) || p.at(FOLLOWING_KW) {
            p.bump_any();
        } else {
            p.error("expected PRECEDING or FOLLOWING");
        }
    } else if p.at(CURRENT_KW) {
        p.bump(CURRENT_KW);
        p.expect(ROW_KW);
    } else {
        expr(p);
        if p.at(PRECEDING_KW) || p.at(FOLLOWING_KW) {
            p.bump_any();
        } else {
            p.error("expected PRECEDING or FOLLOWING");
        }
    }
}

fn case_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(CASE_KW);
    if !p.at(WHEN_KW) {
        expr(p); // simple CASE: the operand before the first WHEN
    }
    while p.at(WHEN_KW) {
        let arm = p.start();
        p.bump(WHEN_KW);
        expr(p);
        p.expect(THEN_KW);
        expr(p);
        arm.complete(p, CASE_WHEN);
    }
    if p.eat(ELSE_KW) {
        expr(p);
    }
    p.expect(END_KW);
    m.complete(p, CASE_EXPR)
}

fn cast_fn_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump_any(); // CAST or TRY_CAST
    p.expect(L_PAREN);
    expr(p);
    p.expect(AS_KW);
    type_name(p);
    p.expect(R_PAREN);
    m.complete(p, CAST_EXPR)
}

/// Semi-structured path tail after a leading `:` (already on the `:` token).
fn json_path(p: &mut Parser) {
    p.bump(COLON);
    json_path_segment(p);
    loop {
        if p.at(DOT) {
            p.bump(DOT);
            json_path_segment(p);
        } else if p.at(COLON) {
            p.bump(COLON);
            json_path_segment(p);
        } else if p.at(L_BRACKET) {
            p.bump(L_BRACKET);
            expr(p);
            p.expect(R_BRACKET);
        } else {
            break;
        }
    }
}

fn json_path_segment(p: &mut Parser) {
    // A path key may be any bare word, including one that spells a keyword (`payload:order`). Tag it
    // as a plain IDENT so its case is preserved — semi-structured keys are case-sensitive.
    if p.at_ident_like() {
        p.bump_as(IDENT);
    } else if p.at(STRING) {
        p.bump_any();
    } else {
        p.error("expected a path segment after ':'");
    }
}

fn values_clause(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(VALUES_KW);
    values_row(p);
    while p.eat(COMMA) {
        values_row(p);
    }
    m.complete(p, VALUES_CLAUSE)
}

fn values_row(p: &mut Parser) {
    let m = p.start();
    p.expect(L_PAREN);
    if !p.at(R_PAREN) {
        expr(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            expr(p);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, VALUES_ROW);
}

fn infix_bp(p: &Parser) -> Option<(u8, u8)> {
    let bp = if p.at(OR_KW) {
        BP_OR
    } else if p.at(AND_KW) {
        BP_AND
    } else if p.at(EQ)
        || p.at(NEQ)
        || p.at(LT)
        || p.at(LTE)
        || p.at(GT)
        || p.at(GTE)
        || p.at(LIKE_KW)
        || p.at(ILIKE_KW)
    {
        BP_CMP
    } else if p.at(CONCAT) {
        BP_CONCAT
    } else if p.at(PLUS) || p.at(MINUS) {
        BP_ADD
    } else if p.at(STAR) || p.at(SLASH) || p.at(PERCENT) {
        BP_MUL
    } else {
        return None;
    };
    Some(bp)
}
