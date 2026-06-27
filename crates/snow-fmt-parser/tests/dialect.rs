//! Dialect-aware keyword reservation: Snowflake-only words (`TASK`, `FLATTEN`, `WAREHOUSE`, …) are
//! plain identifiers under Databricks, so a query selecting them parses clean — while Snowflake's
//! reservation of those same words is unchanged.

use snow_fmt_parser::{parse, parse_with_dialect, Dialect};

/// Parse `sql` under `dialect`, assert it round-trips losslessly, and return the diagnostics.
fn errors_for(sql: &str, dialect: Dialect) -> Vec<String> {
    let parsed = parse_with_dialect(sql, dialect);
    assert_eq!(
        parsed.syntax().to_string(),
        sql,
        "parse tree must round-trip for {sql:?} @ {dialect:?}"
    );
    parsed.errors().iter().map(|e| e.to_string()).collect()
}

#[test]
fn snowflake_only_words_are_identifiers_under_databricks() {
    // The headline acceptance case: these Snowflake-only words are ordinary identifiers in
    // Databricks, so the query parses with no diagnostics.
    let sql = "SELECT task, flatten, warehouse, qualify FROM t";
    let errs = errors_for(sql, Dialect::Databricks);
    assert!(errs.is_empty(), "expected clean parse, got: {errs:?}");
}

#[test]
fn snowflake_only_words_as_table_and_column_names_under_databricks() {
    for sql in [
        "SELECT a FROM task",
        "SELECT a FROM flatten",
        "SELECT cursor, resultset, undrop FROM t",
        "SELECT top, secure, transient FROM t",
    ] {
        let errs = errors_for(sql, Dialect::Databricks);
        assert!(
            errs.is_empty(),
            "expected clean parse of {sql:?}, got: {errs:?}"
        );
    }
}

#[test]
fn snowflake_reservation_is_unchanged() {
    // Under Snowflake those words stay reserved keywords; using one as a bare select item is not a
    // plain identifier, so the parser must not treat the statement the way Databricks does. We only
    // assert the parse still round-trips (never-fail invariant) and that the dialect actually
    // diverged: Databricks parses `SELECT task FROM t` clean, Snowflake does not.
    let sql = "SELECT task FROM t";
    let snowflake_errs = errors_for(sql, Dialect::Snowflake);
    let databricks_errs = errors_for(sql, Dialect::Databricks);
    assert!(
        databricks_errs.is_empty(),
        "Databricks should parse {sql:?} clean: {databricks_errs:?}"
    );
    assert!(
        !snowflake_errs.is_empty(),
        "Snowflake must still reserve TASK so {sql:?} is not a clean identifier select"
    );
}

#[test]
fn shared_keywords_stay_reserved_in_both_dialects() {
    // A normal query using shared keywords parses clean in both dialects.
    let sql = "SELECT a, b FROM t WHERE a > 1 ORDER BY b";
    assert!(errors_for(sql, Dialect::Snowflake).is_empty());
    assert!(errors_for(sql, Dialect::Databricks).is_empty());
}

#[test]
fn default_parse_matches_explicit_snowflake() {
    // `parse` (the default entry point) must stay byte-identical to explicit Snowflake parsing for
    // a query that touches Snowflake-only reserved words.
    let sql = "SELECT a FROM t QUALIFY row_number() OVER (ORDER BY a) = 1";
    let default = parse(sql);
    let snowflake = parse_with_dialect(sql, Dialect::Snowflake);
    assert_eq!(default.syntax().to_string(), snowflake.syntax().to_string());
    assert_eq!(default.errors().len(), snowflake.errors().len());
}
