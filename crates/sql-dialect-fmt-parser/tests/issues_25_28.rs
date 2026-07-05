//! Focused parser coverage for issues #25-#28.

use sql_dialect_fmt_parser::{parse_with_dialect, Dialect, SyntaxKind};
use sql_dialect_fmt_test_support::parser::assert_parse_clean;

fn assert_databricks_clean(sql: &str) {
    let parsed = parse_with_dialect(sql, Dialect::Databricks);
    assert_eq!(parsed.syntax().to_string(), sql);
    assert!(
        parsed.errors().is_empty(),
        "unexpected errors for {sql:?}: {:?}",
        parsed.errors()
    );
}

#[test]
fn select_top_and_fetch_rows_only() {
    assert_parse_clean("SELECT TOP 10 * FROM orders ORDER BY created_at DESC");
    assert_parse_clean("SELECT id FROM orders ORDER BY created_at FETCH FIRST 5 ROWS ONLY");
    assert_parse_clean("SELECT id FROM orders OFFSET 10 FETCH 5 ROWS ONLY");
}

#[test]
fn regex_and_like_quantified_predicates() {
    assert_parse_clean("SELECT * FROM users WHERE email RLIKE '.*@example[.]com'");
    assert_parse_clean("SELECT * FROM users WHERE email REGEXP '.*@example[.]com'");
    assert_parse_clean("SELECT * FROM users WHERE name LIKE ANY ('A%', 'B%')");
    assert_parse_clean("SELECT * FROM users WHERE name NOT ILIKE ALL ('tmp%', 'test%')");
}

#[test]
fn select_window_named_definitions() {
    let sql = "SELECT sum(amount) OVER w FROM orders WINDOW w AS (PARTITION BY customer_id ORDER BY created_at)";
    assert_parse_clean(sql);
    let parsed = parse_with_dialect(sql, Dialect::Snowflake);
    assert!(parsed
        .syntax()
        .descendants()
        .any(|node| node.kind() == SyntaxKind::WINDOW_SPEC));

    assert_parse_clean(
        "SELECT sum(amount) OVER w2 FROM orders WINDOW w AS (PARTITION BY customer_id), w2 AS (w ORDER BY created_at)",
    );
}

#[test]
fn snowflake_star_modifiers_parse_for_bare_and_qualified_star() {
    for sql in [
        "SELECT * EXCLUDE internal_id FROM customers",
        "SELECT * EXCLUDE (internal_id, deleted_at) FROM customers",
        "SELECT * RENAME customer_id AS id FROM customers",
        "SELECT * RENAME (customer_id AS id, customer_name AS name) FROM customers",
        "SELECT * REPLACE (upper(name) AS name) FROM customers",
        "SELECT * ILIKE 'dim_%' FROM customers",
        "SELECT c.* EXCLUDE (internal_id) FROM customers c",
    ] {
        assert_parse_clean(sql);
    }
}

#[test]
fn databricks_star_except_parses_for_bare_and_qualified_star() {
    assert_databricks_clean("SELECT * EXCEPT (internal_id, deleted_at) FROM customers");
    assert_databricks_clean("SELECT c.* EXCEPT (internal_id) FROM customers c");
}
