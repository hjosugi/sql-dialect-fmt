//! Phase 2 conformance: the common query surface (clauses, JOINs, subqueries, set ops, CTEs,
//! compound predicates, window functions) must parse with no diagnostics and round-trip exactly.

use snow_fmt_parser::SyntaxKind;
use snow_fmt_test_support::parser::{assert_has_node_kind, assert_parse_clean as clean};

#[test]
fn all_single_select_clauses() {
    clean("SELECT a, count(*) FROM t GROUP BY a HAVING count(*) > 1 ORDER BY a DESC NULLS LAST LIMIT 10 OFFSET 5");
    clean("SELECT DISTINCT a, b FROM t WHERE a > 1");
    clean("SELECT a FROM t GROUP BY ALL");
    clean("SELECT a AS x, b alias FROM t");
}

#[test]
fn joins() {
    clean("SELECT a FROM t JOIN u ON t.id = u.id");
    clean("SELECT a FROM t LEFT OUTER JOIN u ON t.id = u.id");
    clean("SELECT a FROM t RIGHT JOIN u ON t.id = u.id");
    clean("SELECT a FROM t FULL OUTER JOIN u ON t.id = u.id");
    clean("SELECT a FROM t INNER JOIN u USING (id)");
    clean("SELECT a FROM t CROSS JOIN u");
    clean("SELECT a FROM t NATURAL JOIN u");
    clean("SELECT a FROM a JOIN b ON a.x = b.x JOIN c ON b.y = c.y");
    clean("SELECT a FROM a, b, c");
    assert_has_node_kind("SELECT a FROM t JOIN u ON t.id = u.id", SyntaxKind::JOIN);
}

#[test]
fn subqueries_and_exists() {
    clean("SELECT x FROM (SELECT a AS x FROM t) sub");
    clean("SELECT x FROM (SELECT a AS x FROM t) AS sub");
    clean("SELECT (SELECT max(x) FROM t) AS m FROM u");
    clean("SELECT 1 FROM t WHERE EXISTS (SELECT 1 FROM u WHERE u.id = t.id)");
    assert_has_node_kind(
        "SELECT x FROM (SELECT a AS x FROM t) sub",
        SyntaxKind::SUBQUERY,
    );
    assert_has_node_kind(
        "SELECT 1 FROM t WHERE EXISTS (SELECT 1 FROM u)",
        SyntaxKind::EXISTS_EXPR,
    );
}

#[test]
fn set_operations() {
    clean("SELECT 1 UNION SELECT 2");
    clean("SELECT 1 UNION ALL SELECT 2");
    clean("SELECT a FROM t EXCEPT SELECT a FROM u");
    clean("SELECT a FROM t INTERSECT SELECT a FROM u");
    clean("(SELECT 1) UNION (SELECT 2)");
    assert_has_node_kind("SELECT 1 UNION ALL SELECT 2", SyntaxKind::SET_OP);
}

#[test]
fn ctes() {
    clean("WITH c AS (SELECT 1 AS x) SELECT x FROM c");
    clean("WITH a AS (SELECT 1 AS x), b AS (SELECT 2 AS y) SELECT x, y FROM a, b");
    clean("WITH RECURSIVE c (n) AS (SELECT 1) SELECT n FROM c");
    assert_has_node_kind(
        "WITH c AS (SELECT 1 AS x) SELECT x FROM c",
        SyntaxKind::WITH_CLAUSE,
    );
    assert_has_node_kind("WITH c AS (SELECT 1 AS x) SELECT x FROM c", SyntaxKind::CTE);
}

#[test]
fn compound_predicates() {
    clean("SELECT a FROM t WHERE a IS NULL AND b IS NOT NULL");
    clean("SELECT a FROM t WHERE a IN (1, 2, 3)");
    clean("SELECT a FROM t WHERE a NOT IN (1, 2, 3)");
    clean("SELECT a FROM t WHERE a IN (SELECT id FROM u)");
    clean("SELECT a FROM t WHERE a BETWEEN 1 AND 10");
    clean("SELECT a FROM t WHERE a NOT BETWEEN 1 AND 10");
    clean("SELECT a FROM t WHERE name LIKE 'A%' AND name NOT LIKE 'B%'");
    assert_has_node_kind("SELECT a FROM t WHERE a IS NOT NULL", SyntaxKind::IS_EXPR);
    assert_has_node_kind("SELECT a FROM t WHERE a IN (1, 2)", SyntaxKind::IN_EXPR);
    assert_has_node_kind(
        "SELECT a FROM t WHERE a BETWEEN 1 AND 10",
        SyntaxKind::BETWEEN_EXPR,
    );
}

#[test]
fn window_functions() {
    clean("SELECT row_number() OVER (PARTITION BY a ORDER BY b) AS rn FROM t");
    clean(
        "SELECT sum(x) OVER (ORDER BY a ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM t",
    );
    clean("SELECT avg(x) OVER (PARTITION BY a) FROM t");
    clean("SELECT a FROM t QUALIFY row_number() OVER (PARTITION BY a ORDER BY b) = 1");
    assert_has_node_kind(
        "SELECT row_number() OVER (ORDER BY a) FROM t",
        SyntaxKind::WINDOW_EXPR,
    );
}

#[test]
fn combined_realistic_query() {
    clean(
        "WITH recent AS (\n  SELECT id, amount FROM orders WHERE created_at > '2026-01-01'\n)\nSELECT r.id, sum(r.amount) AS total\nFROM recent r\nJOIN customers c ON c.id = r.id\nWHERE c.active IS TRUE AND r.amount BETWEEN 10 AND 1000\nGROUP BY r.id\nHAVING sum(r.amount) > 100\nQUALIFY row_number() OVER (PARTITION BY r.id ORDER BY total DESC) = 1\nORDER BY total DESC\nLIMIT 50",
    );
}
