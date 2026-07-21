//! The Pratt expression parser: operators, predicates, literals, calls, and window
//! specifications.

use sql_dialect_fmt_syntax::SyntaxKind;
use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{CompletedMarker, ContextualKeyword, Parser};

use super::{name, name_ref, order_by_clause, query_expr, subquery};

// Binding powers for the Pratt parser. Higher binds tighter; (left, right) for infix.
const BP_OR: (u8, u8) = (1, 2);
const BP_AND: (u8, u8) = (3, 4);
pub(super) const BP_CMP: (u8, u8) = (7, 8);
const BP_CONCAT: (u8, u8) = (9, 10);
const BP_ADD: (u8, u8) = (11, 12);
const BP_MUL: (u8, u8) = (13, 14);
const BP_PREFIX_NOT: u8 = 6; // looser than comparison: `NOT a = b` == `NOT (a = b)`
const BP_PREFIX_NEG: u8 = 15; // unary +/- bind tighter than `*`

const INTERVAL_UNIT_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Year,
    ContextualKeyword::Years,
    ContextualKeyword::Month,
    ContextualKeyword::Months,
    ContextualKeyword::Week,
    ContextualKeyword::Weeks,
    ContextualKeyword::Day,
    ContextualKeyword::Days,
    ContextualKeyword::Hour,
    ContextualKeyword::Hours,
    ContextualKeyword::Minute,
    ContextualKeyword::Minutes,
    ContextualKeyword::Second,
    ContextualKeyword::Seconds,
    ContextualKeyword::Millisecond,
    ContextualKeyword::Milliseconds,
    ContextualKeyword::Microsecond,
    ContextualKeyword::Microseconds,
];

// ---- expressions (Pratt) ----

pub(super) fn at_expr_start(p: &Parser) -> bool {
    p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(STRING)
        || p.at(DOLLAR_STRING)
        || p.at(TRUE_KW)
        || p.at(FALSE_KW)
        || p.at(NULL_KW)
        || p.at(VARIABLE)
        || p.at(PLACEHOLDER)
        || p.at(QUESTION)
        || p.at(COLON)
        || p.at(L_BRACKET)
        || p.at(L_BRACE)
        || p.at(L_PAREN)
        || p.at(MINUS)
        || p.at(PLUS)
        || p.at(NOT_KW)
        || (p.at(EXISTS_KW) && p.nth_at(1, L_PAREN))
        || p.at(CASE_KW)
        || p.at(CAST_KW)
        || p.at(TRY_CAST_KW)
        || p.at(FLATTEN_KW)
        || at_interval_literal_start(p)
        || p.at_name()
        || at_keyword_call_name(p) // a keyword used as a function name: first(x)
}

pub(crate) fn expr(p: &mut Parser) {
    expr_bp(p, 0);
}

pub(super) fn expr_bp(p: &mut Parser, min_bp: u8) -> Option<CompletedMarker> {
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

        // Databricks higher-order-function lambda: `x -> expr` / `(x, y) -> expr`.
        if p.dialect().supports_lambda_expr() && p.at(ARROW) {
            if 0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.bump(ARROW);
            expr_bp(p, 1);
            lhs = m.complete(p, LAMBDA_EXPR);
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
        if at_like_predicate(p) || (neg && at_like_predicate_after_not(p)) {
            if BP_CMP.0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.eat(NOT_KW);
            p.bump_any(); // LIKE / ILIKE / RLIKE / REGEXP
            like_rhs(p);
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

fn at_like_predicate(p: &Parser) -> bool {
    p.at(LIKE_KW) || p.at(ILIKE_KW) || p.at(RLIKE_KW) || p.at(REGEXP_KW)
}

fn at_like_predicate_after_not(p: &Parser) -> bool {
    p.nth_at(1, LIKE_KW) || p.nth_at(1, ILIKE_KW) || p.nth_at(1, RLIKE_KW) || p.nth_at(1, REGEXP_KW)
}

fn like_rhs(p: &mut Parser) {
    if p.at(ANY_KW) || p.at(ALL_KW) {
        p.bump_any();
        p.expect(L_PAREN);
        if !p.at(R_PAREN) {
            expr_list(p);
        }
        p.expect(R_PAREN);
    } else {
        expr_bp(p, BP_CMP.1);
    }
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
    } else if p.at(QUESTION) || p.at(COLON) {
        bind_marker(p)
    } else if at_interval_literal_start(p) {
        interval_literal(p)
    } else if p.at(EXISTS_KW) && p.nth_at(1, L_PAREN) {
        let m = p.start();
        p.bump(EXISTS_KW);
        subquery(p);
        m.complete(p, EXISTS_EXPR)
    } else if p.at(L_PAREN) && (p.nth_at(1, SELECT_KW) || p.nth_at(1, WITH_KW)) {
        subquery(p) // scalar subquery
    } else if p.dialect().supports_lambda_expr() && at_parenthesized_lambda_params(p) {
        lambda_params(p)
    } else if p.at(L_PAREN) {
        let m = p.start();
        p.bump(L_PAREN);
        expr(p);
        expect_closing(p, R_PAREN);
        m.complete(p, PAREN_EXPR)
    } else if p.at(L_BRACKET) {
        array_literal(p)
    } else if p.at(L_BRACE) {
        object_literal(p)
    } else if p.at(CASE_KW) {
        case_expr(p)
    } else if p.at(CAST_KW) || p.at(TRY_CAST_KW) {
        cast_fn_expr(p)
    } else if p.at(FLATTEN_KW) {
        // FLATTEN is a keyword but acts as a table/regular function; treat it as a callable name.
        let m = p.start();
        p.bump(FLATTEN_KW);
        m.complete(p, NAME_REF)
    } else if at_keyword_call_name(p) {
        // A keyword-spelled word used as a function name (`first(x)`, `last(x)`, `left(s, 2)`):
        // tag it as a plain name so the postfix `(` makes it a CALL_EXPR and it formats like any
        // other call (lower-case, hugging its parens).
        let m = p.start();
        p.bump_as(IDENT);
        m.complete(p, NAME_REF)
    } else if p.at_name() {
        name_ref(p)
    } else {
        p.err_and_bump("expected an expression");
        return None;
    };
    Some(cm)
}

fn bind_marker(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    if p.at(QUESTION) {
        p.bump(QUESTION);
    } else {
        p.bump(COLON);
        if p.at_name() {
            name(p);
        } else {
            p.error("expected a bind variable name after ':'");
        }
    }
    m.complete(p, BIND_MARKER)
}

fn interval_literal(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // INTERVAL
    if p.at(STRING) {
        p.bump(STRING);
        interval_unit_range(p);
    } else if at_interval_component_start(p) {
        interval_component(p);
        while at_interval_component_start(p) {
            interval_component(p);
        }
    } else {
        p.error("expected an interval literal value");
    }
    m.complete(p, INTERVAL_LITERAL)
}

fn at_interval_literal_start(p: &Parser) -> bool {
    p.nth_contextual(0, ContextualKeyword::Interval)
        && (p.nth_at(1, STRING)
            || p.nth_at(1, INT_NUMBER)
            || p.nth_at(1, FLOAT_NUMBER)
            || ((p.nth_at(1, PLUS) || p.nth_at(1, MINUS))
                && (p.nth_at(2, INT_NUMBER) || p.nth_at(2, FLOAT_NUMBER) || p.nth_at(2, STRING))))
}

fn at_interval_component_start(p: &Parser) -> bool {
    p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(STRING)
        || ((p.at(PLUS) || p.at(MINUS))
            && (p.nth_at(1, INT_NUMBER) || p.nth_at(1, FLOAT_NUMBER) || p.nth_at(1, STRING)))
}

fn interval_component(p: &mut Parser) {
    if !p.eat(PLUS) {
        p.eat(MINUS);
    }
    if p.at(INT_NUMBER) || p.at(FLOAT_NUMBER) || p.at(STRING) {
        p.bump_any();
    } else {
        p.error("expected an interval literal value");
        return;
    }
    interval_unit_range(p);
}

fn interval_unit_range(p: &mut Parser) {
    if at_interval_unit(p) {
        p.bump_as(CONTEXTUAL_KEYWORD);
        if p.nth_contextual(0, ContextualKeyword::To) {
            p.bump_as(CONTEXTUAL_KEYWORD);
            if at_interval_unit(p) {
                p.bump_as(CONTEXTUAL_KEYWORD);
            } else {
                p.error("expected an interval unit after TO");
            }
        }
    }
}

fn at_interval_unit(p: &Parser) -> bool {
    p.nth_any_contextual(0, INTERVAL_UNIT_CONTEXTUAL_WORDS)
}

fn array_literal(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(L_BRACKET);
    if !p.at(R_BRACKET) {
        expr(p);
        while p.eat(COMMA) {
            if p.at(R_BRACKET) {
                break;
            }
            expr(p);
        }
    }
    expect_closing(p, R_BRACKET);
    m.complete(p, ARRAY_LITERAL)
}

fn object_literal(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(L_BRACE);
    if !p.at(R_BRACE) {
        object_field(p);
        while p.eat(COMMA) {
            if p.at(R_BRACE) {
                break;
            }
            object_field(p);
        }
    }
    expect_closing(p, R_BRACE);
    m.complete(p, OBJECT_LITERAL)
}

fn expect_closing(p: &mut Parser, kind: SyntaxKind) {
    if p.eat(kind) {
        return;
    }
    let msg = format!("expected {}", kind.describe());
    if p.at_eof() || p.at(SEMICOLON) || p.at(R_PAREN) || p.at(R_BRACKET) || p.at(R_BRACE) {
        p.error(msg);
    } else {
        p.err_and_bump(msg);
    }
}

fn object_field(p: &mut Parser) {
    let m = p.start();
    object_key(p);
    p.expect(COLON);
    if at_expr_start(p) {
        expr(p);
    } else {
        p.error("expected an object literal value");
    }
    m.complete(p, OBJECT_FIELD);
}

fn object_key(p: &mut Parser) {
    if p.at(STRING)
        || p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(TRUE_KW)
        || p.at(FALSE_KW)
        || p.at(NULL_KW)
    {
        let m = p.start();
        p.bump_any();
        m.complete(p, LITERAL);
    } else if p.at_name() {
        name_ref(p);
    } else {
        p.error("expected an object literal key");
    }
}

fn at_parenthesized_lambda_params(p: &Parser) -> bool {
    if !p.at(L_PAREN) {
        return false;
    }
    let mut depth = 0u32;
    for i in 0..48 {
        if p.nth_at(i, EOF) {
            return false;
        }
        if p.nth_at(i, L_PAREN) {
            depth += 1;
        } else if p.nth_at(i, R_PAREN) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return p.nth_at(i + 1, ARROW);
            }
        }
    }
    false
}

fn lambda_params(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(L_PAREN);
    if !p.at(R_PAREN) {
        if p.at_name() {
            name(p);
        } else {
            p.error("expected a lambda parameter");
        }
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            name(p);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, LAMBDA_PARAMS)
}

fn at_keyword_call_name(p: &Parser) -> bool {
    (p.at(FIRST_KW) || p.at(LAST_KW) || p.at(LEFT_KW) || p.at(RIGHT_KW)) && p.nth_at(1, L_PAREN)
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

pub(super) fn expr_list(p: &mut Parser) {
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

pub(super) fn arg_list(p: &mut Parser) {
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
    if !p.eat(R_PAREN) {
        p.err_and_bump(format!("expected {}", R_PAREN.describe()));
    }
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

pub(super) fn type_name(p: &mut Parser) {
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

pub(super) fn window_spec(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    if p.at_name() {
        name_ref(p); // base window name in `WINDOW w AS (base ORDER BY ts)`
    }
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

pub(super) fn partition_by_clause(p: &mut Parser) {
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

fn infix_bp(p: &Parser) -> Option<(u8, u8)> {
    let bp = if p.at(OR_KW) {
        BP_OR
    } else if p.at(AND_KW) {
        BP_AND
    } else if p.at(EQ)
        || p.at(NEQ)
        || p.at(NULL_SAFE_EQ)
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
