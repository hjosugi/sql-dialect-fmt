//! Formatting coverage for structural generic CTAS bodies and routine signature clauses.

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_parser::parse;

fn fmt(sql: &str) -> String {
    format(sql, &FormatOptions::default())
}

#[test]
fn generic_ctas_query_uses_structural_layout() {
    let sql =
        "CREATE DATA PRODUCT sales AS SELECT account_id, amount FROM raw.sales WHERE amount > 0";
    let formatted = fmt(sql);
    assert_eq!(
        formatted,
        "CREATE DATA PRODUCT sales AS\nSELECT account_id, amount\nFROM raw.sales\nWHERE amount > 0;\n"
    );
    assert!(parse(&formatted).errors().is_empty());
    assert_eq!(fmt(&formatted), formatted);
}

#[test]
fn routine_signature_nodes_preserve_existing_layout() {
    let sql = "create procedure p() returns table(id number, name string) language sql as $$ begin return table(select 1, 'x'); end; $$";
    let formatted = fmt(sql);
    assert_eq!(
        formatted,
        "CREATE PROCEDURE p () RETURNS TABLE (id number, name string) LANGUAGE SQL AS $$\nBEGIN\n    RETURN TABLE(SELECT 1, 'x');\nEND;\n$$;\n"
    );
    assert!(parse(&formatted).errors().is_empty());
    assert_eq!(fmt(&formatted), formatted);
}

#[test]
#[cfg(feature = "embedded-javascript")]
fn language_clause_wins_over_a_parameter_named_language() {
    let sql = "create function f(language string) returns string language javascript as $$ return language; $$";
    assert_eq!(
        fmt(sql),
        "CREATE FUNCTION f (LANGUAGE string) RETURNS string LANGUAGE JAVASCRIPT AS $$\nreturn language;\n$$;\n"
    );
}
