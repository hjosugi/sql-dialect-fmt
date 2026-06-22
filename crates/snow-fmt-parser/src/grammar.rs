//! The Snowflake SQL grammar.
//!
//! Phase 1 covered a single `SELECT` plus a Pratt expression parser. Phase 2 grows this toward
//! "parse the most common queries completely": all single-`SELECT` clauses, `JOIN`s, subqueries
//! and derived tables, set operations, CTEs, and the compound predicates (`IS [NOT] NULL`,
//! `[NOT] IN/BETWEEN/LIKE`).
//!
//! Every rule is total: on unexpected input it records a diagnostic and recovers, never panics.

use snow_fmt_syntax::SyntaxKind::*;

use crate::parser::{CompletedMarker, Parser};

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
            statement(p);
        } else {
            p.err_and_bump("expected a statement");
        }
    }
    m.complete(p, SOURCE_FILE);
}

fn at_stmt_start(p: &Parser) -> bool {
    p.at(SELECT_KW) || p.at(WITH_KW) || p.at(VALUES_KW) || at_expr_start(p)
}

fn statement(p: &mut Parser) {
    if p.at(WITH_KW) {
        with_query(p);
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

// ---- queries ----

fn with_query(p: &mut Parser) {
    let m = p.start();
    with_clause(p);
    query_expr(p);
    m.complete(p, WITH_QUERY);
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
    } else if p.at(VALUES_KW) {
        Some(values_clause(p))
    } else {
        p.error("expected a query (SELECT, VALUES, or a parenthesized subquery)");
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
    if p.at(L_PAREN) {
        subquery(p); // derived table
        table_alias(p);
    } else if p.at_name() {
        name_ref(p);
        table_alias(p);
    } else {
        p.error("expected a table reference");
    }
    m.complete(p, TABLE_REF);
}

fn table_alias(p: &mut Parser) {
    let explicit_alias = p.eat(AS_KW);
    if explicit_alias || p.at_name() {
        name(p);
        if p.at(L_PAREN) {
            column_list(p); // derived-table column aliases: (c1, c2, ...)
        }
    }
}

fn at_join_start(p: &Parser) -> bool {
    p.at(JOIN_KW)
        || p.at(INNER_KW)
        || p.at(LEFT_KW)
        || p.at(RIGHT_KW)
        || p.at(FULL_KW)
        || p.at(CROSS_KW)
        || p.at(NATURAL_KW)
}

fn join(p: &mut Parser) {
    let m = p.start();
    p.eat(NATURAL_KW);
    if p.at(INNER_KW) {
        p.bump(INNER_KW);
    } else if p.at(LEFT_KW) || p.at(RIGHT_KW) || p.at(FULL_KW) {
        p.bump_any();
        p.eat(OUTER_KW);
    } else if p.at(CROSS_KW) {
        p.bump(CROSS_KW);
    }
    p.expect(JOIN_KW);
    table_ref(p);
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

fn group_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(GROUP_KW);
    p.expect(BY_KW);
    if p.at(ALL_KW) {
        p.bump(ALL_KW);
    } else {
        expr(p);
        while p.eat(COMMA) {
            expr(p);
        }
    }
    m.complete(p, GROUP_BY_CLAUSE);
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
        || p.at_name()
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
    } else if p.at_name() {
        name_ref(p)
    } else {
        p.error("expected an expression");
        return None;
    };
    Some(cm)
}

fn is_rhs(p: &mut Parser) {
    if p.at(NULL_KW) || p.at(TRUE_KW) || p.at(FALSE_KW) {
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
    if p.at_name() || p.at(STRING) {
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
