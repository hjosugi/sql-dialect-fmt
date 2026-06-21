//! CREATE FUNCTION / PROCEDURE with embedded-language bodies. Each must parse cleanly, round-trip
//! byte-for-byte, and expose the structural nodes (CREATE_FUNCTION, RETURNS_CLAUSE, LANGUAGE_CLAUSE,
//! FUNC_BODY) the formatter and highlighter rely on.

use snow_fmt_parser::{parse, SyntaxKind};

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
fn javascript_function() {
    let sql = "CREATE OR REPLACE FUNCTION add_js(a FLOAT, b FLOAT) RETURNS FLOAT LANGUAGE JAVASCRIPT AS $$ return A + B; $$";
    clean(sql);
    assert!(has(sql, SyntaxKind::CREATE_FUNCTION));
    assert!(has(sql, SyntaxKind::PARAM_LIST));
    assert!(has(sql, SyntaxKind::RETURNS_CLAUSE));
    assert!(has(sql, SyntaxKind::LANGUAGE_CLAUSE));
    assert!(has(sql, SyntaxKind::FUNC_BODY));
}

#[test]
fn procedure_with_options() {
    clean(
        "CREATE OR REPLACE PROCEDURE p(n INT) RETURNS STRING LANGUAGE JAVASCRIPT STRICT AS $$ return String(N); $$",
    );
}

#[test]
fn function_with_string_body_and_modifiers() {
    clean("CREATE SECURE FUNCTION g() RETURNS STRING LANGUAGE SQL AS 'select 1'");
}

#[test]
fn function_with_null_handling_and_runtime_options() {
    clean(
        "CREATE FUNCTION py(x INT) RETURNS INT LANGUAGE PYTHON RUNTIME_VERSION = '3.11' HANDLER = 'main' RETURNS NULL ON NULL INPUT AS $$ def main(x): return x $$",
    );
}

#[test]
fn returns_table_udtf() {
    clean("CREATE FUNCTION t() RETURNS TABLE (a INT, b STRING) AS $$ select 1, 'x' $$");
}

#[test]
fn round_trips_with_broken_or_partial_input() {
    // Must stay lossless even when incomplete (never panic, never lose bytes).
    for s in [
        "CREATE FUNCTION",
        "CREATE OR REPLACE FUNCTION f(",
        "CREATE FUNCTION f() RETURNS",
    ] {
        assert_eq!(
            parse(s).syntax().to_string(),
            s,
            "round-trip failed for {s:?}"
        );
    }
}
