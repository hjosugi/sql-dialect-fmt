//! Phase 2b: high-frequency expression gaps — CASE, CAST(...)/TRY_CAST(...), semi-structured
//! `col:path` access, and the VALUES clause. Each must parse with no diagnostics and round-trip.

use sql_dialect_fmt_parser::{parse, SyntaxKind};

fn clean(s: &str) {
    let p = parse(s);
    assert!(
        p.errors().is_empty(),
        "unexpected errors for {s:?}: {:?}",
        p.errors()
    );
    assert_eq!(p.syntax().to_string(), s, "round-trip failed for {s:?}");
}

fn has(s: &str, kind: SyntaxKind) -> bool {
    parse(s).syntax().descendants().any(|n| n.kind() == kind)
}

#[test]
fn case_expressions() {
    clean("SELECT CASE WHEN a > 1 THEN 'big' WHEN a = 1 THEN 'one' ELSE 'small' END FROM t");
    clean("SELECT CASE x WHEN 1 THEN 'a' WHEN 2 THEN 'b' END AS label FROM t");
    clean("SELECT a FROM t WHERE CASE WHEN a IS NULL THEN 0 ELSE a END > 1");
    assert!(has("SELECT CASE WHEN a THEN 1 END", SyntaxKind::CASE_EXPR));
    assert!(has("SELECT CASE WHEN a THEN 1 END", SyntaxKind::CASE_WHEN));
}

#[test]
fn cast_function_form() {
    clean("SELECT CAST(a AS int), TRY_CAST(b AS varchar(10)) FROM t");
    clean("SELECT CAST(x AS number(38, 0)) FROM t");
    // both the `::` form and the function form land on CAST_EXPR
    assert!(has("SELECT CAST(a AS int) FROM t", SyntaxKind::CAST_EXPR));
    assert!(has("SELECT a::int FROM t", SyntaxKind::CAST_EXPR));
}

#[test]
fn semi_structured_path() {
    clean("SELECT payload:user.name FROM raw");
    clean("SELECT payload:user.name::string AS n FROM raw");
    clean("SELECT col:items[0]:id FROM raw");
    clean("SELECT v:\"Quoted Key\".x FROM raw");
    assert!(has(
        "SELECT payload:user.name FROM raw",
        SyntaxKind::JSON_ACCESS
    ));
}

#[test]
fn values_clause() {
    clean("VALUES (1, 2), (3, 4)");
    clean("SELECT * FROM (VALUES (1, 'a'), (2, 'b')) AS v (id, name)");
}

#[test]
fn values_in_subquery_and_setop() {
    clean("SELECT id FROM (VALUES (1), (2), (3)) AS t (id)");
    assert!(has(
        "SELECT * FROM (VALUES (1)) v",
        SyntaxKind::VALUES_CLAUSE
    ));
    assert!(has("VALUES (1), (2)", SyntaxKind::VALUES_ROW));
}

#[test]
fn still_lossless_on_combined() {
    clean(
        "SELECT\n  CASE WHEN p:active::boolean THEN 'on' ELSE 'off' END AS state,\n  CAST(p:score AS number) AS score\nFROM raw\nWHERE p:tags[0] = 'vip'",
    );
}
