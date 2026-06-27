//! Comprehensive Databricks/Spark SQL **parser** invariant matrix + cross-dialect guards.
//!
//! The `CASES` array mirrors the formatter matrix (`snow-fmt-formatter/tests/databricks_matrix.rs`)
//! and asserts, under the Databricks dialect, that every case (1) parses with no diagnostics and
//! (2) round-trips byte-for-byte. A second block asserts the *structure* (specific `SyntaxKind`
//! nodes are produced for the Databricks-specific constructs). A third block is the cross-dialect
//! guard set: Snowflake-only syntax must NOT be silently accepted under Databricks where the two
//! dialects must differ, and Databricks-only syntax must NOT parse clean under Snowflake.

use snow_fmt_lexer::tokenize_for_dialect;
use snow_fmt_parser::{parse, parse_with_dialect, Dialect, SyntaxKind};

/// The shared corpus: every entry must parse clean and round-trip losslessly under Databricks.
const CASES: &[&str] = &[
    // ---- shared SQL ----
    "SELECT a, b FROM t",
    "SELECT a, b FROM t WHERE a > 1",
    "SELECT DISTINCT a FROM t",
    "SELECT count(DISTINCT a) FROM t",
    "SELECT a FROM t ORDER BY a DESC NULLS LAST",
    "SELECT a FROM t LIMIT 10 OFFSET 5",
    "SELECT a FROM t JOIN u ON t.id = u.id",
    "SELECT a FROM t LEFT JOIN u ON t.id = u.id",
    "SELECT a FROM t LEFT OUTER JOIN u ON t.id = u.id",
    "SELECT a FROM t FULL OUTER JOIN u ON t.id = u.id",
    "SELECT a FROM t CROSS JOIN u",
    "WITH c AS (SELECT 1 AS n) SELECT n FROM c",
    "WITH a AS (SELECT 1), b AS (SELECT 2) SELECT * FROM a, b",
    "SELECT sum(x) OVER (PARTITION BY a ORDER BY b) FROM t",
    "SELECT a FROM t QUALIFY row_number() OVER (ORDER BY b) = 1",
    "SELECT a FROM t QUALIFY rank() OVER (PARTITION BY a ORDER BY b) = 1",
    "SELECT a, count(*) FROM t GROUP BY a",
    "SELECT a, count(*) FROM t GROUP BY cube (a, b)",
    "SELECT a, count(*) FROM t GROUP BY rollup (a, b)",
    "SELECT a, count(*) FROM t GROUP BY grouping sets ((a), (b))",
    "SELECT a FROM t GROUP BY a HAVING count(*) > 1",
    "SELECT a FROM t UNION SELECT a FROM u",
    "SELECT a FROM t UNION ALL SELECT a FROM u",
    "SELECT a FROM t INTERSECT SELECT a FROM u",
    "SELECT a FROM t EXCEPT SELECT a FROM u",
    "SELECT CASE WHEN a > 0 THEN 1 ELSE 0 END FROM t",
    "SELECT CAST(a AS int) FROM t",
    "SELECT CAST(a AS decimal(10, 2)) FROM t",
    "SELECT a FROM t WHERE a IN (SELECT b FROM u)",
    "SELECT a FROM (SELECT a FROM u) sub",
    "INSERT INTO t (a, b) VALUES (1, 2)",
    "UPDATE t SET a = 1 WHERE b = 2",
    "DELETE FROM t WHERE a = 1",
    "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.x = s.x",
    "SELECT * FROM t PIVOT (sum(x) FOR k IN ('a', 'b'))",
    "SELECT * FROM t TABLESAMPLE (10 percent)",
    // ---- databricks: backtick identifiers ----
    "SELECT `a` FROM t",
    "SELECT `a b` FROM t",
    "SELECT `it``s` FROM t",
    "SELECT c AS `my col` FROM t",
    "SELECT `weird``col` AS `out``put` FROM `db`.`tbl`",
    // ---- databricks: Unity Catalog 3-level names ----
    "SELECT * FROM cat.sch.tbl",
    "SELECT * FROM `catalog`.`schema`.`table`",
    "SELECT a FROM main.default.events e JOIN main.default.users u ON e.uid = u.id",
    // ---- databricks: CREATE TABLE USING + options ----
    "CREATE TABLE t (id BIGINT) USING DELTA",
    "CREATE TABLE t (id BIGINT) USING parquet LOCATION '/mnt/x'",
    "CREATE TABLE t (id BIGINT) USING DELTA PARTITIONED BY (id)",
    "CREATE TABLE t (id BIGINT) USING DELTA CLUSTER BY (id)",
    "CREATE TABLE t (id BIGINT) USING DELTA TBLPROPERTIES ('k' = 'v')",
    "CREATE TABLE events (id BIGINT, payload STRING) USING DELTA LOCATION '/mnt/events' TBLPROPERTIES ('delta.enableChangeDataFeed' = 'true')",
    "CREATE OR REPLACE TABLE t (id int) USING DELTA",
    // ---- databricks: LATERAL VIEW ----
    "SELECT * FROM events LATERAL VIEW explode(items) t AS item",
    "SELECT * FROM events LATERAL VIEW OUTER explode(items) t AS item",
    "SELECT * FROM events LATERAL VIEW posexplode(items) t AS pos, item",
    // ---- databricks: time travel ----
    "SELECT * FROM t VERSION AS OF 5",
    "SELECT * FROM t TIMESTAMP AS OF '2024-01-01'",
    "SELECT * FROM a JOIN b VERSION AS OF 3 ON a.id = b.id",
    "SELECT * FROM t1 JOIN t2 TIMESTAMP AS OF '2024-01-01' ON t1.id = t2.id",
    // ---- databricks: higher-order lambdas ----
    "SELECT transform(xs, x -> x + 1) FROM t",
    "SELECT filter(xs, x -> x > 0) FROM t",
    "SELECT aggregate(xs, 0, (acc, x) -> acc + x) FROM t",
    "SELECT reduce(xs, 0, (acc, x) -> acc + x, acc -> acc) FROM t",
    "SELECT zip_with(a, b, (x, y) -> x + y) FROM t",
    "SELECT transform(xs, x -> transform(x, y -> y + 1)) FROM t",
    "SELECT transform(filter(xs, x -> x > 0), y -> y * 2) FROM t",
    // ---- databricks: operators ----
    "SELECT a || b FROM t",
    "SELECT a::int FROM t",
    // ---- databricks: Snowflake-only words become plain identifiers ----
    "SELECT task, flatten, warehouse FROM t",
    "SELECT a FROM task",
    "SELECT cursor, resultset, undrop FROM t",
    // ---- databricks: Delta maintenance + cache statements (full matrix in databricks_delta.rs) ----
    "VACUUM t RETAIN 168 HOURS DRY RUN",
    "OPTIMIZE t WHERE a > 1 ZORDER BY (a, b)",
    "INSERT OVERWRITE TABLE t PARTITION (dt = '2024-01-01') SELECT a, b FROM s",
    "CACHE TABLE t AS SELECT * FROM s",
    "UNCACHE TABLE IF EXISTS t",
    "REFRESH TABLE t",
    "DESCRIBE HISTORY t",
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE THEN DELETE",
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT *",
];

fn parse_databricks_clean(sql: &str) {
    let parsed = parse_with_dialect(sql, Dialect::Databricks);
    assert_eq!(
        parsed.syntax().to_string(),
        sql,
        "Databricks parse tree must round-trip for {sql:?}"
    );
    assert!(
        parsed.errors().is_empty(),
        "unexpected Databricks parse errors for {sql:?}: {:?}",
        parsed.errors()
    );
}

fn has_node(sql: &str, kind: SyntaxKind) -> bool {
    parse_with_dialect(sql, Dialect::Databricks)
        .syntax()
        .descendants()
        .any(|node| node.kind() == kind)
}

#[test]
fn all_cases_parse_clean_and_round_trip() {
    for sql in CASES {
        parse_databricks_clean(sql);
    }
}

// ---- structural assertions ----

#[test]
fn lateral_view_is_structured() {
    assert!(has_node(
        "SELECT * FROM events LATERAL VIEW explode(items) t AS item",
        SyntaxKind::LATERAL_VIEW
    ));
    assert!(has_node(
        "SELECT * FROM events LATERAL VIEW OUTER posexplode(items) t AS pos, item",
        SyntaxKind::LATERAL_VIEW
    ));
}

#[test]
fn time_travel_is_structured() {
    assert!(has_node(
        "SELECT * FROM t VERSION AS OF 5",
        SyntaxKind::AS_OF_TRAVEL
    ));
    assert!(has_node(
        "SELECT * FROM t TIMESTAMP AS OF '2024-01-01'",
        SyntaxKind::AS_OF_TRAVEL
    ));
}

#[test]
fn lambdas_are_structured() {
    assert!(has_node(
        "SELECT transform(xs, x -> x + 1) FROM t",
        SyntaxKind::LAMBDA_EXPR
    ));
    assert!(has_node(
        "SELECT zip_with(a, b, (x, y) -> x + y) FROM t",
        SyntaxKind::LAMBDA_EXPR
    ));
    assert!(has_node(
        "SELECT zip_with(a, b, (x, y) -> x + y) FROM t",
        SyntaxKind::LAMBDA_PARAMS
    ));
}

#[test]
fn qualify_is_structured_under_databricks() {
    // The regression guard for the QUALIFY fix: under Databricks the clause must produce a
    // QUALIFY_CLAUSE node, not a bare identifier select item.
    assert!(has_node(
        "SELECT a FROM t QUALIFY row_number() OVER (ORDER BY b) = 1",
        SyntaxKind::QUALIFY_CLAUSE
    ));
}

#[test]
fn delta_table_options_are_structured() {
    let sql = "CREATE TABLE events (id BIGINT) USING DELTA LOCATION '/mnt/events' TBLPROPERTIES ('delta.enableChangeDataFeed' = 'true')";
    assert!(has_node(sql, SyntaxKind::CREATE_STMT));
    assert!(has_node(sql, SyntaxKind::OBJECT_PROPERTY));
}

// ---- cross-dialect guards ----

#[test]
fn backtick_identifiers_are_databricks_only() {
    let sql = "SELECT `a b` FROM `catalog`.`schema`.`table`";
    // Clean under Databricks.
    parse_databricks_clean(sql);
    let databricks = tokenize_for_dialect(sql, Dialect::Databricks);
    assert!(databricks.errors.is_empty());
    assert!(databricks
        .tokens
        .iter()
        .any(|t| t.kind == SyntaxKind::QUOTED_IDENT && t.text == "`a b`"));
    // Snowflake must reject the backtick at the lexer.
    let snowflake = tokenize_for_dialect(sql, Dialect::Snowflake);
    assert!(
        !snowflake.errors.is_empty(),
        "Snowflake mode must reject backtick-quoted identifiers"
    );
}

#[test]
fn snowflake_dollar_constructs_are_not_databricks() {
    // `$1` positional references and `$$ ... $$` dollar-quoted bodies are Snowflake-only. Under
    // Databricks the lexer has no dollar-quoting/positional-ref rule, so the `$` falls out as a
    // bare `DOLLAR` token rather than a single `VARIABLE` / `DOLLAR_STRING` — and the parser then
    // reports errors. Snowflake accepts both cleanly. (Lexing itself does not error in either
    // dialect; the divergence is in tokenization and in the parse result.)
    for (sql, snow_kind) in [
        ("SELECT $1 FROM t", SyntaxKind::VARIABLE),
        ("SELECT $$body$$ FROM t", SyntaxKind::DOLLAR_STRING),
    ] {
        let snowflake = tokenize_for_dialect(sql, Dialect::Snowflake);
        assert!(
            snowflake.tokens.iter().any(|t| t.kind == snow_kind),
            "Snowflake should tokenize {sql:?} as a single {snow_kind:?}"
        );
        assert!(parse_with_dialect(sql, Dialect::Snowflake)
            .errors()
            .is_empty());

        let databricks = tokenize_for_dialect(sql, Dialect::Databricks);
        assert!(
            databricks.tokens.iter().all(|t| t.kind != snow_kind),
            "Databricks must NOT produce a {snow_kind:?} token for {sql:?}"
        );
        assert!(
            databricks
                .tokens
                .iter()
                .any(|t| t.kind == SyntaxKind::DOLLAR),
            "Databricks should leave a bare DOLLAR token for {sql:?}"
        );
        assert!(
            !parse_with_dialect(sql, Dialect::Databricks)
                .errors()
                .is_empty(),
            "Databricks must not parse Snowflake dollar construct {sql:?} cleanly"
        );
        // Lossless either way.
        assert_eq!(
            parse_with_dialect(sql, Dialect::Databricks)
                .syntax()
                .to_string(),
            sql
        );
    }
}

#[test]
fn snowflake_stage_refs_are_not_databricks() {
    // `@stage` path refs are Snowflake-only: Snowflake lexes the `@` as an `AT` token and parses
    // clean, while Databricks has no stage-ref rule, so the `@` lex-errors.
    let sql = "SELECT * FROM @stage";
    let snowflake = tokenize_for_dialect(sql, Dialect::Snowflake);
    assert!(
        snowflake.errors.is_empty(),
        "Snowflake should lex @stage clean"
    );
    assert!(snowflake.tokens.iter().any(|t| t.kind == SyntaxKind::AT));

    let databricks = tokenize_for_dialect(sql, Dialect::Databricks);
    assert!(
        !databricks.errors.is_empty(),
        "Databricks must reject the @stage reference at the lexer"
    );
}

#[test]
fn snowflake_only_words_are_identifiers_under_databricks_but_reserved_in_snowflake() {
    // `TASK` / `FLATTEN` are reserved in Snowflake (so a bare select item is not clean) but plain
    // identifiers in Databricks (so it parses clean).
    let sql = "SELECT task, flatten FROM t";
    let databricks = parse_with_dialect(sql, Dialect::Databricks);
    assert!(
        databricks.errors().is_empty(),
        "Databricks should treat task/flatten as identifiers: {:?}",
        databricks.errors()
    );
    assert_eq!(databricks.syntax().to_string(), sql);

    let snowflake = parse_with_dialect(sql, Dialect::Snowflake);
    assert!(
        !snowflake.errors().is_empty(),
        "Snowflake must keep task/flatten reserved"
    );
    // Never-fail / lossless still holds in the dialect that errors.
    assert_eq!(snowflake.syntax().to_string(), sql);
}

#[test]
fn databricks_lambdas_do_not_parse_clean_under_snowflake() {
    let parsed = parse_with_dialect(
        "SELECT transform(items, x -> x + 1) FROM events",
        Dialect::Snowflake,
    );
    assert!(
        !parsed.errors().is_empty(),
        "Snowflake must not parse Databricks lambda arrows cleanly"
    );
    // Lossless even when it errors.
    assert_eq!(
        parsed.syntax().to_string(),
        "SELECT transform(items, x -> x + 1) FROM events"
    );
}

#[test]
fn qualify_stays_reserved_in_both_dialects() {
    // Regression guard: making QUALIFY shared must not change Snowflake. `parse` (default) and an
    // explicit Snowflake parse remain byte-identical, and Databricks now agrees.
    let sql = "SELECT a FROM t QUALIFY row_number() OVER (ORDER BY a) = 1";
    let default = parse(sql);
    let snowflake = parse_with_dialect(sql, Dialect::Snowflake);
    let databricks = parse_with_dialect(sql, Dialect::Databricks);
    assert!(default.errors().is_empty());
    assert!(snowflake.errors().is_empty());
    assert!(databricks.errors().is_empty());
    assert_eq!(default.syntax().to_string(), snowflake.syntax().to_string());
}

#[test]
fn never_fails_on_databricks_gap_constructs() {
    // Constructs the grammar does not yet model (gap report) must still never panic and must
    // round-trip losslessly under BOTH dialects, even though they produce diagnostics.
    for sql in [
        // Higher-order `exists(array, lambda)` is not modeled as a generator (the bare `exists`
        // predicate `EXISTS (subquery)` is); the lambda arrow still round-trips verbatim.
        "SELECT exists(xs, x -> x > 0) FROM t",
    ] {
        for dialect in [Dialect::Snowflake, Dialect::Databricks] {
            let parsed = parse_with_dialect(sql, dialect);
            assert_eq!(
                parsed.syntax().to_string(),
                sql,
                "lossless round-trip must hold for gap construct {sql:?} @ {dialect:?}"
            );
        }
    }
}

#[test]
fn delta_commands_now_parse_clean_under_databricks_and_round_trip_under_snowflake() {
    // Statements that were previously gap constructs are now first-class under Databricks: they
    // parse clean there, while remaining lossless-but-unrecognized under Snowflake (the leading
    // words stay plain identifiers). `VACUUM t` in particular no longer mis-splits under Databricks.
    for sql in [
        "OPTIMIZE t ZORDER BY (a)",
        "VACUUM t",
        "VACUUM t RETAIN 0 HOURS DRY RUN",
        "CACHE TABLE t",
        "REFRESH TABLE t",
        "INSERT OVERWRITE TABLE t SELECT * FROM s",
        "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE THEN DELETE",
        "DESCRIBE HISTORY t",
    ] {
        let databricks = parse_with_dialect(sql, Dialect::Databricks);
        assert!(
            databricks.errors().is_empty(),
            "Databricks must now parse {sql:?} clean: {:?}",
            databricks.errors()
        );
        assert_eq!(databricks.syntax().to_string(), sql);

        let snowflake = parse_with_dialect(sql, Dialect::Snowflake);
        assert_eq!(
            snowflake.syntax().to_string(),
            sql,
            "Snowflake must round-trip losslessly for {sql:?}"
        );
    }
}
