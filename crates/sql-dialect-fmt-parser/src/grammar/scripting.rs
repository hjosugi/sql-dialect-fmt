//! Snowflake Scripting blocks (`DECLARE … BEGIN … END`) and their control-flow statements.

use sql_dialect_fmt_syntax::SyntaxKind;
use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{ContextualKeyword, Parser};

use super::{expr, name_ref, stmt};

// ---- Snowflake Scripting blocks (Phase 8) ----

/// A scripting block starts at `DECLARE`, or at a `BEGIN` that is not a transaction start.
pub(super) fn at_block_start(p: &Parser) -> bool {
    p.at(DECLARE_KW) || (p.at(BEGIN_KW) && !at_begin_transaction(p))
}

/// At a transaction-starting `BEGIN` (`BEGIN;`, `BEGIN TRANSACTION …`, `BEGIN WORK`) — as opposed to
/// a Snowflake Scripting block (`BEGIN <stmt>; … END`). Only the transaction form is recognized (so
/// it formats inline); a scripting block is left to pass through verbatim, its inner `;`-separated
/// statements never mis-split. `BEGIN NAME …` is intentionally not matched (rarer, and `name` is a
/// common identifier).
pub(super) fn at_begin_transaction(p: &Parser) -> bool {
    p.at(BEGIN_KW)
        && (p.nth_at(1, SEMICOLON)
            || p.nth_contextual(1, ContextualKeyword::Transaction)
            || p.nth_contextual(1, ContextualKeyword::Work))
}

/// `[DECLARE <decls>] BEGIN <body> [EXCEPTION <handlers>] END [<label>]` — a Snowflake Scripting
/// block. The body and handler bodies are statement sequences (`STMT_LIST`); control-flow statements
/// (IF / loops) are structured and everything else is kept as a lenient inline statement, so the
/// block round-trips losslessly even where a construct is not modeled in detail.
pub(super) fn block_stmt(p: &mut Parser) {
    let m = p.start();
    if p.at(DECLARE_KW) {
        declare_section(p);
    }
    p.expect(BEGIN_KW);
    if p.eat(NOT_KW) {
        if p.nth_contextual(0, ContextualKeyword::Atomic) {
            p.bump_as(CONTEXTUAL_KEYWORD);
        } else {
            p.error("expected ATOMIC after NOT");
        }
    } else if p.nth_contextual(0, ContextualKeyword::Atomic) {
        p.bump_as(CONTEXTUAL_KEYWORD);
    }
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
    } else if p.at(DECLARE_KW) {
        simple_script_stmt(p, DECLARE_ITEM);
    } else if at_block_start(p) {
        block_stmt(p); // nested DECLARE…/BEGIN…END
    } else if p.at(CASE_KW) {
        case_stmt(p);
    } else if p.at(LET_KW) {
        simple_script_stmt(p, LET_STMT);
    } else if p.at(RETURN_KW) {
        simple_script_stmt(p, RETURN_STMT);
    } else if stmt::at_sql_stmt_start(p) {
        stmt::statement(p);
    } else if p.at_name() && p.nth_at(1, ASSIGN) {
        simple_script_stmt(p, ASSIGN_STMT);
    } else {
        simple_script_stmt(p, SCRIPT_STMT);
    }
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

/// A procedural `CASE` statement, in both documented forms: searched
/// `CASE WHEN <cond> THEN <stmts> ... END [CASE]` and simple
/// `CASE <operand> WHEN <value> THEN <stmts> ... END [CASE]`.
fn case_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(CASE_KW);
    if !p.at(WHEN_KW) {
        expr(p); // simple CASE operand
    }
    while p.at(WHEN_KW) {
        let arm = p.start();
        p.bump(WHEN_KW);
        expr(p);
        p.expect(THEN_KW);
        stmt_list(p, |p| p.at(WHEN_KW) || p.at(ELSE_KW) || p.at(END_KW));
        arm.complete(p, CASE_STMT_WHEN);
    }
    if p.eat(ELSE_KW) {
        stmt_list(p, |p| p.at(END_KW));
    }
    p.expect(END_KW);
    p.eat(CASE_KW); // optional trailing CASE in `END CASE`
    m.complete(p, CASE_STMT);
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
