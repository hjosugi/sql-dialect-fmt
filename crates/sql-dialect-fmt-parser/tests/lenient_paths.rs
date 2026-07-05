//! Regression coverage for lenient parser paths.
//!
//! These paths intentionally keep wide SQL surfaces as token runs, but they must still round-trip,
//! tag known contextual words, preserve nested parentheses, and report diagnostics at malformed
//! delimiters.

use sql_dialect_fmt_parser::{parse, SyntaxKind};
use sql_dialect_fmt_test_support::parser::{assert_has_node_kind, assert_parse_clean};

#[test]
fn lenient_statement_families_round_trip_cleanly() {
    for sql in [
        "ALTER TABLE t ADD COLUMN c NUMBER(10, 2) NOT NULL COMMENT 'amount column'",
        "SHOW TABLES LIKE 'ORDERS%' IN SCHEMA analytics",
        "COMMENT ON TABLE analytics.orders IS 'contains a semicolon; inside the string'",
        "CREATE MASKING POLICY redact_email AS (val STRING) RETURNS STRING -> CASE WHEN CURRENT_ROLE() = 'ADMIN' THEN val ELSE 'x' END",
        "CREATE TAG classification ALLOWED_VALUES 'public', 'internal', 'restricted' PROPAGATE = ON_DEPENDENCY",
    ] {
        assert_parse_clean(sql);
    }
}

#[test]
fn lenient_balanced_runs_preserve_nested_commas() {
    let sql = "CREATE TABLE t (id NUMBER(38, 0) PRIMARY KEY, amount NUMBER(18, 4) DEFAULT round(1, 2), CONSTRAINT pk PRIMARY KEY (id, amount))";
    let parsed = assert_parse_clean(sql);
    let column_defs = parsed
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::COLUMN_DEF)
        .count();
    assert_eq!(
        column_defs, 3,
        "nested parens must not split column definitions"
    );
    assert_has_node_kind(sql, SyntaxKind::COLUMN_DEF_LIST);
}

#[test]
fn semantic_view_items_preserve_nested_commas() {
    let sql = "CREATE SEMANTIC VIEW sv TABLES(orders AS mart.orders PRIMARY KEY(order_id, line_id), customers AS mart.customers PRIMARY KEY(customer_id)) METRICS(PUBLIC orders.revenue AS SUM(orders.net_amount))";
    let parsed = assert_parse_clean(sql);
    let items = parsed
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::SEMANTIC_VIEW_ITEM)
        .count();
    assert_eq!(
        items, 3,
        "nested key/function parens must not split semantic-view items"
    );
}

#[test]
fn malformed_lenient_regions_round_trip_with_diagnostics() {
    for sql in [
        "CREATE TABLE t (a NUMBER(10, 2",
        "CREATE SEMANTIC VIEW sv TABLES(orders AS mart.orders PRIMARY KEY(order_id)",
        "COPY INTO @stage FROM (SELECT * FROM",
    ] {
        let parsed = parse(sql);
        assert_eq!(parsed.syntax().to_string(), sql, "round-trip for {sql:?}");
        assert!(
            !parsed.errors().is_empty(),
            "expected diagnostics for malformed lenient input {sql:?}"
        );
    }
}
