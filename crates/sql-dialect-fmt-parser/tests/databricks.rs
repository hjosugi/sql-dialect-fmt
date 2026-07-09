//! Databricks/Spark SQL parser coverage for dialect-gated grammar.

use sql_dialect_fmt_lexer::tokenize_for_dialect;
use sql_dialect_fmt_parser::{parse_with_dialect, Dialect, Parse, SyntaxKind};

fn parse_databricks(sql: &str) -> Parse {
    let parsed = parse_with_dialect(sql, Dialect::Databricks);
    assert_eq!(
        parsed.syntax().to_string(),
        sql,
        "parse tree must round-trip for {sql:?}"
    );
    assert!(
        parsed.errors().is_empty(),
        "unexpected parse errors for {sql:?}: {:?}",
        parsed.errors()
    );
    parsed
}

fn has_node(sql: &str, kind: SyntaxKind) -> bool {
    parse_databricks(sql)
        .syntax()
        .descendants()
        .any(|node| node.kind() == kind)
}

#[test]
fn lateral_view_clauses_are_structured() {
    let sql = "SELECT * FROM events LATERAL VIEW explode(items) item AS item_id";
    assert!(has_node(sql, SyntaxKind::LATERAL_VIEW));
}

#[test]
fn table_time_travel_is_structured() {
    assert!(has_node(
        "SELECT * FROM events VERSION AS OF 12",
        SyntaxKind::AS_OF_TRAVEL
    ));
    assert!(has_node(
        "SELECT * FROM events TIMESTAMP AS OF '2024-01-01'",
        SyntaxKind::AS_OF_TRAVEL
    ));
}

#[test]
fn higher_order_function_lambdas_are_structured() {
    assert!(has_node(
        "SELECT transform(items, x -> x + 1) FROM events",
        SyntaxKind::LAMBDA_EXPR
    ));
    assert!(has_node(
        "SELECT zip_with(a, b, (x, y) -> x + y) FROM events",
        SyntaxKind::LAMBDA_EXPR
    ));
    assert!(has_node(
        "SELECT zip_with(a, b, (x, y) -> x + y) FROM events",
        SyntaxKind::LAMBDA_PARAMS
    ));
}

#[test]
fn delta_table_options_are_structured_as_create_properties() {
    let sql = "CREATE TABLE events (id BIGINT, payload STRING) USING DELTA LOCATION '/mnt/events' TBLPROPERTIES ('delta.enableChangeDataFeed' = 'true')";
    assert!(has_node(sql, SyntaxKind::CREATE_STMT));
    assert!(has_node(sql, SyntaxKind::OBJECT_PROPERTY));
}

#[test]
fn sql_scripting_blocks_are_enabled() {
    assert!(has_node("BEGIN\nSELECT 1;\nEND", SyntaxKind::BLOCK_STMT));
    assert!(has_node(
        "BEGIN ATOMIC\nDECLARE total_amount DECIMAL(10, 2);\nIF total_amount IS NULL THEN\nSELECT 0;\nEND IF;\nEND",
        SyntaxKind::IF_STMT
    ));
}

#[test]
fn backtick_quoted_identifiers_are_databricks_only() {
    let sql = "SELECT `a b` FROM `catalog`.`schema`.`table`";
    parse_databricks(sql);

    let databricks = tokenize_for_dialect(sql, Dialect::Databricks);
    assert!(databricks.errors.is_empty());
    assert!(databricks
        .tokens
        .iter()
        .any(|token| token.kind == SyntaxKind::QUOTED_IDENT && token.text == "`a b`"));

    let snowflake = tokenize_for_dialect(sql, Dialect::Snowflake);
    assert!(
        !snowflake.errors.is_empty(),
        "Snowflake mode should reject backtick-quoted identifiers"
    );
}

#[test]
fn snowflake_mode_does_not_accept_databricks_lambdas_cleanly() {
    let parsed = parse_with_dialect(
        "SELECT transform(items, x -> x + 1) FROM events",
        Dialect::Snowflake,
    );
    assert!(
        !parsed.errors().is_empty(),
        "Snowflake mode should not parse Databricks lambda arrows cleanly"
    );
}
