//! Databricks/Spark SQL formatter coverage behind the dialect option.

use snow_fmt_formatter::{format, FormatOptions};
use snow_fmt_lexer::tokenize_for_dialect;
use snow_fmt_syntax::{Dialect, SyntaxKind};

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
fn keeps_backtick_quoted_identifiers() {
    assert_databricks_format(
        "select `a b` from `catalog`.`schema`.`table`",
        "SELECT `a b`\nFROM `catalog`.`schema`.`table`;\n",
    );
}
