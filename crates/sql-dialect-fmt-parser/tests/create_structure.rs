//! Structural parsing coverage for generic CTAS bodies and routine signatures.

use sql_dialect_fmt_parser::SyntaxKind;
use sql_dialect_fmt_test_support::parser::assert_parse_clean;

#[test]
fn generic_create_query_bodies_are_structured() {
    for sql in [
        "CREATE DATA PRODUCT sales AS SELECT account_id, amount FROM raw.sales",
        "CREATE REPORT daily AS WITH src AS (SELECT 1 AS n) SELECT n FROM src",
        "CREATE DATASET constants AS VALUES (1), (2)",
        "CREATE SNAPSHOT s AS (SELECT id FROM source_table)",
    ] {
        let parsed = assert_parse_clean(sql);
        assert!(
            parsed.syntax().descendants().any(|node| matches!(
                node.kind(),
                SyntaxKind::SELECT_STMT
                    | SyntaxKind::WITH_QUERY
                    | SyntaxKind::VALUES_CLAUSE
                    | SyntaxKind::SUBQUERY
            )),
            "expected a structural query body for {sql:?}"
        );
    }
}

#[test]
fn non_query_as_surface_is_not_misclassified_as_ctas() {
    let sql = "CREATE CUSTOM POLICY p AS (value STRING) RETURNS STRING";
    let parsed = assert_parse_clean(sql);
    assert!(!parsed.syntax().descendants().any(|node| matches!(
        node.kind(),
        SyntaxKind::SELECT_STMT
            | SyntaxKind::WITH_QUERY
            | SyntaxKind::VALUES_CLAUSE
            | SyntaxKind::SUBQUERY
    )));
}

#[test]
fn routine_returns_and_language_are_distinct_nodes() {
    for sql in [
        "CREATE FUNCTION f(x NUMBER) RETURNS STRING NOT NULL LANGUAGE JAVASCRIPT AS $$ return 'ok'; $$",
        "CREATE PROCEDURE p() RETURNS TABLE(id NUMBER, name STRING) LANGUAGE SQL AS $$ BEGIN RETURN TABLE(SELECT 1, 'x'); END; $$",
        "CREATE FUNCTION py_f() RETURNS VARIANT LANGUAGE PYTHON RUNTIME_VERSION = '3.12' HANDLER = 'main' AS $$\ndef main():\n    return {}\n$$",
    ] {
        let parsed = assert_parse_clean(sql);
        let root = parsed.syntax();
        assert_eq!(
            root.descendants()
                .filter(|node| node.kind() == SyntaxKind::ROUTINE_RETURNS_CLAUSE)
                .count(),
            1,
            "expected one RETURNS clause for {sql:?}"
        );
        assert_eq!(
            root.descendants()
                .filter(|node| node.kind() == SyntaxKind::ROUTINE_LANGUAGE_CLAUSE)
                .count(),
            1,
            "expected one LANGUAGE clause for {sql:?}"
        );
    }
}

#[test]
fn null_input_behavior_is_not_a_second_return_type() {
    let sql =
        "CREATE FUNCTION f(x NUMBER) RETURNS NUMBER RETURNS NULL ON NULL INPUT LANGUAGE SQL AS 'x'";
    let parsed = assert_parse_clean(sql);
    assert_eq!(
        parsed
            .syntax()
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::ROUTINE_RETURNS_CLAUSE)
            .count(),
        1
    );
}
