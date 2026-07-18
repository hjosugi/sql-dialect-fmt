//! Query grammar: CTEs, set operations, `SELECT` cores and their clauses, and `FROM`/`JOIN`
//! table references.

use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{CompletedMarker, ContextualKeyword, Parser};

use super::{
    arg_list, at_expr_start, balanced_parens, column_list, expr, expr_bp, match_recognize, name,
    name_ref, stage_ref, window_spec, BP_CMP,
};

// ---- queries ----

pub(super) fn with_query(p: &mut Parser) -> CompletedMarker {
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
pub(super) fn query_expr(p: &mut Parser) -> Option<CompletedMarker> {
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

pub(super) fn subquery(p: &mut Parser) -> CompletedMarker {
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
    if p.at(TOP_KW) {
        top_clause(p);
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
    if p.at(WINDOW_KW) {
        window_clause(p);
    }
    while p.dialect().supports_databricks_query_clauses()
        && at_databricks_query_distribution_clause(p)
    {
        databricks_query_distribution_clause(p);
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
    if p.at(FETCH_KW) {
        fetch_clause(p);
    }
    m.complete(p, SELECT_STMT)
}

/// Snowflake `SELECT TOP <n>` header. Keep the count as direct header tokens instead of an
/// expression node so the SELECT-list formatter sees `TOP n` as part of the header, not as a child
/// to ignore. Parenthesized counts are accepted as a balanced token run.
fn top_clause(p: &mut Parser) {
    p.bump(TOP_KW);
    if p.at(L_PAREN) {
        balanced_parens(p);
    } else if p.at(INT_NUMBER) || p.at(FLOAT_NUMBER) || p.at(VARIABLE) || p.at_name() {
        p.bump_any();
    } else {
        p.error("expected a row count after TOP");
    }
}

// ---- SELECT list ----

fn select_list(p: &mut Parser) {
    let m = p.start();
    select_item(p);
    while p.eat(COMMA) {
        if p.at_eof() {
            p.error("expected a select item after ','");
            break;
        }
        if at_clause_end(p) {
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
        || p.at(WINDOW_KW)
        || (p.dialect().supports_databricks_query_clauses()
            && at_databricks_query_distribution_clause(p))
        || p.at(ORDER_KW)
        || p.at(LIMIT_KW)
        || p.at(OFFSET_KW)
        || p.at(FETCH_KW)
}

pub(super) fn select_item(p: &mut Parser) {
    let m = p.start();
    if p.at(STAR) || at_qualified_star(p) {
        star_select_expr(p);
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

fn at_qualified_star(p: &Parser) -> bool {
    if !p.at_name() {
        return false;
    }
    let mut i = 1;
    while i < 32 {
        if !p.nth_at(i, DOT) {
            return false;
        }
        if p.nth_at(i + 1, STAR) {
            return true;
        }
        if !(p.nth_at(i + 1, IDENT) || p.nth_at(i + 1, QUOTED_IDENT)) {
            return false;
        }
        i += 2;
    }
    false
}

fn star_select_expr(p: &mut Parser) {
    let s = p.start();
    if p.at(STAR) {
        p.bump(STAR);
    } else {
        // Qualified star: `t.*`, `"db"."schema".t.*`.
        name_ref(p);
    }
    while at_star_modifier(p) {
        star_modifier(p);
    }
    s.complete(p, STAR_EXPR);
}

fn at_star_modifier(p: &Parser) -> bool {
    p.at(ILIKE_KW)
        || p.at(REPLACE_KW)
        || (p.dialect().supports_delta_commands() && p.at(EXCEPT_KW))
        || (p.dialect().supports_semantic_view() && p.at_name())
}

fn star_modifier(p: &mut Parser) {
    if p.at(ILIKE_KW) {
        p.bump(ILIKE_KW);
        expr_bp(p, BP_CMP.1);
    } else if p.at(REPLACE_KW) {
        p.bump(REPLACE_KW);
        star_modifier_parens(p);
    } else if p.at(EXCEPT_KW) {
        p.bump(EXCEPT_KW);
        star_modifier_parens(p);
    } else if p.at_name() {
        // Snowflake's `EXCLUDE` and `RENAME` are contextual here. The parser does not reserve those
        // words globally, so recognize the modifier by position after `*`.
        p.bump_as(CONTEXTUAL_KEYWORD);
        if p.at(L_PAREN) {
            star_modifier_parens(p);
        } else if p.at_name() {
            name_ref(p);
            if p.eat(AS_KW) && p.at_name() {
                name(p);
            }
        }
    }
}

fn star_modifier_parens(p: &mut Parser) {
    p.expect(L_PAREN);
    if !p.at(R_PAREN) {
        star_modifier_item(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            star_modifier_item(p);
        }
    }
    p.expect(R_PAREN);
}

fn star_modifier_item(p: &mut Parser) {
    if at_expr_start(p) {
        expr(p);
    } else if p.at(STAR) {
        p.bump(STAR);
    } else {
        p.error("expected a star modifier item");
        return;
    }
    if p.eat(AS_KW) && p.at_name() {
        name(p);
    }
}

// ---- FROM / JOIN ----

pub(super) fn from_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(FROM_KW);
    table_ref(p);
    loop {
        if p.dialect().supports_lateral_view() && at_lateral_view(p) {
            lateral_view(p);
        } else if at_join_start(p) {
            join(p);
        } else if p.eat(COMMA) {
            table_ref(p);
        } else {
            break;
        }
    }
    m.complete(p, FROM_CLAUSE);
}

pub(super) fn table_ref(p: &mut Parser) {
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
    } else if p.at(AT) {
        // A staged-file source: `FROM @stage[/path] [( FILE_FORMAT => ... )]` (data-load transform).
        stage_ref(p);
        if p.at(L_PAREN) {
            arg_list(p); // FROM @s ( FILE_FORMAT => my_ff, PATTERN => '...' )
        }
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
    if p.dialect().supports_as_of_travel() && at_databricks_as_of_travel(p) {
        databricks_as_of_travel(p);
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

fn at_lateral_view(p: &Parser) -> bool {
    p.at(LATERAL_KW) && p.nth_at(1, VIEW_KW)
}

/// Databricks/Spark `LATERAL VIEW [OUTER] generator(...) [table_alias] AS col [, ...]`.
fn lateral_view(p: &mut Parser) {
    let m = p.start();
    p.bump(LATERAL_KW);
    p.expect(VIEW_KW);
    p.eat(OUTER_KW);
    if at_expr_start(p) {
        expr(p);
    } else {
        p.error("expected a generator expression after LATERAL VIEW");
    }
    if p.at_name() {
        name(p);
    }
    p.eat(AS_KW);
    if p.at_name() {
        name(p);
        while p.eat(COMMA) {
            if p.at_name() {
                name(p);
            } else {
                p.error("expected a column alias after ','");
                break;
            }
        }
    }
    m.complete(p, LATERAL_VIEW);
}

/// Time-travel: `AT ( ... )` / `BEFORE ( ... )` (`at`/`before` are contextual keywords).
fn at_time_travel(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::At) || p.nth_contextual(0, ContextualKeyword::Before))
        && p.nth_at(1, L_PAREN)
}

/// `<table> {AT|BEFORE} ( TIMESTAMP|OFFSET|STATEMENT => ... )`, captured leniently.
fn time_travel(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // AT / BEFORE (contextual keyword)
    if p.at(L_PAREN) {
        balanced_parens(p);
    }
    m.complete(p, TIME_TRAVEL);
}

fn at_databricks_as_of_travel(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::Version)
        || p.nth_contextual(0, ContextualKeyword::Timestamp))
        && p.nth_at(1, AS_KW)
        && p.nth_contextual(2, ContextualKeyword::Of)
}

/// Databricks table time travel: `VERSION AS OF <expr>` / `TIMESTAMP AS OF <expr>`.
fn databricks_as_of_travel(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // VERSION / TIMESTAMP
    p.expect(AS_KW);
    if p.nth_contextual(0, ContextualKeyword::Of) {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else {
        p.error("expected OF in time travel clause");
    }
    if at_expr_start(p) {
        expr(p);
    } else {
        p.error("expected a time travel value");
    }
    m.complete(p, AS_OF_TRAVEL);
}

/// `<table> {SAMPLE|TABLESAMPLE} [method] ( n [ROWS] ) [REPEATABLE|SEED ( seed )]`. The fraction
/// and any method/seed are captured leniently (balanced parens) for inline formatting.
fn sample_clause(p: &mut Parser) {
    let m = p.start();
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
    m.complete(p, SAMPLE_CLAUSE);
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
        || (p.dialect().supports_as_of_travel() && at_databricks_as_of_travel(p))
        || (p.dialect().supports_lateral_view() && at_lateral_view(p))
        || (p.dialect().supports_databricks_query_clauses()
            && at_databricks_query_distribution_clause(p))
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

pub(super) fn where_clause(p: &mut Parser) {
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

/// SQL named window definitions: `WINDOW w AS (...), w2 AS (w ORDER BY ts)`.
///
/// There is no dedicated `WINDOW_CLAUSE` node yet, so this reuses the generic select-clause
/// formatting path through `QUALIFY_CLAUSE`; the contained definitions still use `WINDOW_SPEC`.
fn window_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(WINDOW_KW);
    window_definition(p);
    while p.eat(COMMA) {
        if at_clause_end(p) {
            break;
        }
        window_definition(p);
    }
    m.complete(p, QUALIFY_CLAUSE);
}

fn window_definition(p: &mut Parser) {
    if p.at_name() {
        name(p);
    } else {
        p.error("expected a window name");
    }
    p.expect(AS_KW);
    if p.at(L_PAREN) {
        window_spec(p);
    } else {
        p.error("expected a window specification");
    }
}

pub(super) fn order_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(ORDER_KW);
    p.expect(BY_KW);
    order_by_item(p);
    while p.eat(COMMA) {
        order_by_item(p);
    }
    m.complete(p, ORDER_BY_CLAUSE);
}

fn at_databricks_query_distribution_clause(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::Distribute)
        || p.nth_contextual(0, ContextualKeyword::Sort)
        || p.nth_contextual(0, ContextualKeyword::Cluster))
        && p.nth_at(1, BY_KW)
}

fn databricks_query_distribution_clause(p: &mut Parser) {
    let m = p.start();
    let kind = if p.nth_contextual(0, ContextualKeyword::Distribute) {
        p.bump_as(CONTEXTUAL_KEYWORD);
        DISTRIBUTE_BY_CLAUSE
    } else if p.nth_contextual(0, ContextualKeyword::Sort) {
        p.bump_as(CONTEXTUAL_KEYWORD);
        SORT_BY_CLAUSE
    } else {
        p.bump_as(CONTEXTUAL_KEYWORD);
        CLUSTER_BY_CLAUSE
    };
    p.expect(BY_KW);
    order_by_item(p);
    while p.eat(COMMA) {
        if at_clause_end(p) {
            break;
        }
        order_by_item(p);
    }
    m.complete(p, kind);
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

fn fetch_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(FETCH_KW);
    p.eat(FIRST_KW);
    expr(p);
    if p.at(ROW_KW) || p.at(ROWS_KW) {
        p.bump_any();
    } else {
        p.error("expected ROW or ROWS after FETCH count");
    }
    // `ONLY` is contextual and intentionally not reserved globally.
    if p.at_name() {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else {
        p.error("expected ONLY after FETCH ... ROWS");
    }
    m.complete(p, LIMIT_CLAUSE);
}

pub(super) fn values_clause(p: &mut Parser) -> CompletedMarker {
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
