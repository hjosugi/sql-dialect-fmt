//! Snowflake-specific lexer edge matrix.
//!
//! These tests are deliberately table-driven and small: they cover tricky dialect boundaries
//! without turning `cargo test` into a slow benchmark suite.

use sql_dialect_fmt_lexer::tokenize;
use sql_dialect_fmt_lexer::SyntaxKind::*;
use sql_dialect_fmt_test_support::lexer::{assert_lex_lossless, assert_lexes_non_trivia_to};

#[test]
fn snowflake_flow_pipe_boundaries() {
    let cases = [
        ("->>", vec![(FLOW_PIPE, "->>")]),
        ("->>>", vec![(FLOW_PIPE, "->>"), (GT, ">")]),
        ("-> > ", vec![(ARROW, "->"), (GT, ">")]),
        ("- >>", vec![(MINUS, "-"), (GT, ">"), (GT, ">")]),
        (
            "a->>b",
            vec![(IDENT, "a"), (FLOW_PIPE, "->>"), (IDENT, "b")],
        ),
        ("a->b", vec![(IDENT, "a"), (ARROW, "->"), (IDENT, "b")]),
    ];

    for (sql, expected) in cases {
        assert_lexes_non_trivia_to(sql, &expected);
    }
}

#[test]
fn flow_pipe_examples_from_current_snowflake_docs() {
    let sql = r#"SHOW TABLES
  ->> SELECT "created_on" AS creation_date, "name" AS table_name FROM $1
  ->> SELECT count(*) FROM $1;"#;
    let lexed = tokenize(sql);

    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    assert_eq!(
        lexed
            .tokens
            .iter()
            .filter(|token| token.kind == FLOW_PIPE)
            .count(),
        2
    );
    assert_eq!(
        lexed
            .tokens
            .iter()
            .filter(|token| token.kind == VARIABLE && token.text == "$1")
            .count(),
        2
    );
    assert_lex_lossless(sql);
}

#[test]
fn snowflake_semi_structured_and_stage_edges() {
    let cases = [
        (
            "payload:customer.id::STRING",
            vec![
                (IDENT, "payload"),
                (COLON, ":"),
                (IDENT, "customer"),
                (DOT, "."),
                (IDENT, "id"),
                (COLON2, "::"),
                (IDENT, "STRING"),
            ],
        ),
        (
            "arr[0]:items[1]::ARRAY",
            vec![
                (IDENT, "arr"),
                (L_BRACKET, "["),
                (INT_NUMBER, "0"),
                (R_BRACKET, "]"),
                (COLON, ":"),
                (IDENT, "items"),
                (L_BRACKET, "["),
                (INT_NUMBER, "1"),
                (R_BRACKET, "]"),
                (COLON2, "::"),
                (IDENT, "ARRAY"),
            ],
        ),
        (
            "@~/path/file.csv",
            vec![
                (AT, "@"),
                (TILDE, "~"),
                (SLASH, "/"),
                (IDENT, "path"),
                (SLASH, "/"),
                (IDENT, "file"),
                (DOT, "."),
                (IDENT, "csv"),
            ],
        ),
    ];

    for (sql, expected) in cases {
        assert_lexes_non_trivia_to(sql, &expected);
    }
}

#[test]
fn dollar_body_can_contain_sql_javascript_python_like_text() {
    let sql = r#"CREATE PROCEDURE p()
RETURNS STRING
LANGUAGE JAVASCRIPT
AS $$
const q = `SELECT ':not_a_path', ${not_close} FROM T WHERE x ->> y`;
return q.replace(/\/\*.*?\*\//g, "");
$$;"#;
    let lexed = tokenize(sql);

    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    let bodies: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|token| token.kind == DOLLAR_STRING)
        .collect();
    assert_eq!(bodies.len(), 1);
    assert!(bodies[0].text.contains("->>"));
    assert!(bodies[0].text.contains("${not_close}"));
    assert_lex_lossless(sql);
}

#[test]
fn long_input_stays_lossless_without_fixture_bloat() {
    let mut sql = String::from("SELECT\n");
    for i in 0..512 {
        if i > 0 {
            sql.push_str(",\n");
        }
        sql.push_str("    payload:");
        sql.push_str(&format!("field{i}"));
        sql.push_str("::STRING AS c");
        sql.push_str(&i.to_string());
    }
    sql.push_str("\nFROM @~/stage/path\n");
    sql.push_str("->> SELECT count(*) FROM $1;\n");

    let lexed = assert_lex_lossless(&sql);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    assert_eq!(
        lexed
            .tokens
            .iter()
            .filter(|token| token.kind == FLOW_PIPE)
            .count(),
        1
    );
}
