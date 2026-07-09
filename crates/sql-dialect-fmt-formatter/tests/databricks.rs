//! Databricks/Spark SQL formatter coverage behind the dialect option.

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::tokenize_for_dialect;
use sql_dialect_fmt_syntax::{Dialect, SyntaxKind};

fn fmt(src: &str) -> String {
    format(
        src,
        &FormatOptions::default().with_dialect(Dialect::Databricks),
    )
}

fn significant_tokens(sql: &str) -> Vec<String> {
    tokenize_for_dialect(sql, Dialect::Databricks)
        .tokens
        .iter()
        .filter(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::SEMICOLON)
        .map(|token| token.text.to_ascii_uppercase())
        .collect()
}

fn assert_databricks_format(input: &str, expected: &str) {
    let out = fmt(input);
    assert_eq!(out, expected);
    assert_eq!(fmt(&out), out, "Databricks formatting must be idempotent");
    assert_eq!(
        significant_tokens(input),
        significant_tokens(&out),
        "Databricks formatting changed significant tokens"
    );
}

#[test]
fn formats_lateral_view_on_its_own_from_line() {
    assert_databricks_format(
        "select * from events lateral view explode(items) item as item_id",
        "SELECT *\nFROM events\nLATERAL VIEW explode(items) item AS item_id;\n",
    );
}

#[test]
fn formats_databricks_table_time_travel() {
    assert_databricks_format(
        "select * from events version as of 12",
        "SELECT *\nFROM events VERSION AS OF 12;\n",
    );
}

#[test]
fn formats_databricks_query_distribution_clauses() {
    assert_databricks_format(
        "select * from events distribute by bucket_id sort by event_ts desc",
        "SELECT *\nFROM events\nDISTRIBUTE BY bucket_id\nSORT BY event_ts DESC;\n",
    );
    assert_databricks_format(
        "select * from events cluster by bucket_id, event_ts",
        "SELECT *\nFROM events\nCLUSTER BY bucket_id, event_ts;\n",
    );
}

#[test]
fn formats_databricks_lexer_gap_constructs() {
    assert_databricks_format(
        "select a <=> b, r'raw\\n', x'0A0B' from t",
        "SELECT a <=> b, r'raw\\n', x'0A0B'\nFROM t;\n",
    );
}

#[test]
fn formats_higher_order_function_lambdas() {
    assert_databricks_format(
        "select transform(items, x -> x + 1) from events",
        "SELECT transform(items, x -> x + 1)\nFROM events;\n",
    );
    assert_databricks_format(
        "select zip_with(a, b, (x, y) -> x + y) from events",
        "SELECT zip_with(a, b, (x, y) -> x + y)\nFROM events;\n",
    );
}

#[test]
fn formats_delta_table_options_as_create_properties() {
    assert_databricks_format(
        "create table events (id bigint, payload string) using delta location '/mnt/events' tblproperties ('delta.enableChangeDataFeed' = 'true')",
        "CREATE TABLE events (id bigint, payload string)\n    USING delta\n    LOCATION '/mnt/events'\n    TBLPROPERTIES ('delta.enableChangeDataFeed' = 'true');\n",
    );
}

#[test]
fn formats_sql_scripting_blocks() {
    assert_databricks_format("begin\nselect 1;\nend", "BEGIN\n    SELECT 1;\nEND;\n");
    assert_databricks_format(
        "begin atomic\ndeclare total_amount decimal(10, 2);\nif total_amount is null then\nselect 0;\nend if;\nend",
        "BEGIN ATOMIC\n    DECLARE total_amount DECIMAL(10, 2);\n    IF total_amount IS NULL THEN\n        SELECT 0;\n    END IF;\nEND;\n",
    );
}

#[test]
fn formats_embedded_sql_body_with_databricks_dialect() {
    let src =
        "create procedure p() returns string language sql as 'begin select a <=> b from t; end'";
    let expected = "CREATE PROCEDURE p () RETURNS string LANGUAGE SQL AS '\nBEGIN\n    SELECT a <=> b FROM t;\nEND;\n';\n";
    let out = fmt(src);
    assert_eq!(out, expected);
    assert_eq!(fmt(&out), out);
}

#[test]
fn keeps_backtick_quoted_identifiers() {
    assert_databricks_format(
        "select `a b` from `catalog`.`schema`.`table`",
        "SELECT `a b`\nFROM `catalog`.`schema`.`table`;\n",
    );
}

#[test]
fn formats_star_except_modifiers() {
    assert_databricks_format(
        "select c.* except (internal_id, deleted_at) from customers c",
        "SELECT c.* EXCEPT (internal_id, deleted_at)\nFROM customers c;\n",
    );
}
