//! Parser coverage for the Databricks/Delta maintenance + cache statements:
//! `VACUUM`, `OPTIMIZE … ZORDER BY`, `INSERT OVERWRITE`, the `MERGE` extensions
//! (`WHEN NOT MATCHED [BY TARGET] THEN INSERT *`, `WHEN NOT MATCHED BY SOURCE THEN …`),
//! `CACHE`/`UNCACHE`/`REFRESH`, and `DESCRIBE HISTORY`.
//!
//! Each case asserts, under the **Databricks** dialect, that it (1) parses with no diagnostics and
//! (2) round-trips byte-for-byte. A structural block asserts the bespoke `SyntaxKind` nodes are
//! produced. The cross-dialect guard block asserts the same text under **Snowflake** never panics
//! and round-trips losslessly — where these words stay ordinary identifiers, so Snowflake is
//! unchanged.

use snow_fmt_parser::{parse_with_dialect, Dialect, SyntaxKind};

/// Every entry must parse clean and round-trip losslessly under Databricks.
const CASES: &[&str] = &[
    // ---- VACUUM ----
    "VACUUM t",
    "VACUUM main.default.events",
    "VACUUM '/mnt/data/events'",
    "VACUUM t RETAIN 168 HOURS",
    "VACUUM t RETAIN 0 HOURS DRY RUN",
    "VACUUM t DRY RUN",
    "VACUUM `db`.`tbl` RETAIN 24 HOURS",
    // ---- OPTIMIZE ----
    "OPTIMIZE t",
    "OPTIMIZE main.default.events",
    "OPTIMIZE t WHERE a > 1",
    "OPTIMIZE t ZORDER BY (a)",
    "OPTIMIZE t ZORDER BY (a, b, c)",
    "OPTIMIZE t WHERE dt = '2024-01-01' ZORDER BY (id, ts)",
    // ---- INSERT OVERWRITE ----
    "INSERT OVERWRITE TABLE t SELECT * FROM s",
    "INSERT OVERWRITE t SELECT * FROM s",
    "INSERT OVERWRITE TABLE t PARTITION (dt = '2024-01-01') SELECT a, b FROM s",
    "INSERT OVERWRITE TABLE t PARTITION (dt) SELECT a, dt FROM s",
    "INSERT OVERWRITE TABLE t VALUES (1, 2), (3, 4)",
    "INSERT OVERWRITE TABLE t (a, b) SELECT a, b FROM s",
    // INSERT INTO must keep working unchanged.
    "INSERT INTO t (a, b) VALUES (1, 2)",
    "INSERT INTO t SELECT * FROM s",
    // ---- MERGE extensions ----
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT *",
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY TARGET THEN INSERT *",
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE THEN DELETE",
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE THEN UPDATE SET t.x = 0",
    "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.x = s.x WHEN NOT MATCHED THEN INSERT * WHEN NOT MATCHED BY SOURCE THEN DELETE",
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE AND t.flag = 1 THEN DELETE",
    // Classic Snowflake-compatible MERGE forms still parse under Databricks.
    "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.x = s.x",
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT (id) VALUES (s.id)",
    // ---- CACHE / UNCACHE / REFRESH ----
    "CACHE TABLE t",
    "CACHE LAZY TABLE t",
    "CACHE TABLE t OPTIONS ('storageLevel' = 'DISK_ONLY')",
    "CACHE TABLE t AS SELECT * FROM s",
    "CACHE TABLE t SELECT * FROM s",
    "CACHE LAZY TABLE t OPTIONS ('storageLevel' 'MEMORY_ONLY') SELECT a FROM s WHERE a > 0",
    "UNCACHE TABLE t",
    "UNCACHE TABLE IF EXISTS t",
    "UNCACHE TABLE main.default.events",
    "REFRESH TABLE t",
    "REFRESH t",
    "REFRESH '/mnt/data/events'",
    "REFRESH TABLE main.default.events",
    // ---- DESCRIBE HISTORY ----
    "DESCRIBE HISTORY t",
    "DESC HISTORY t",
    "DESCRIBE HISTORY main.default.events",
    "DESCRIBE HISTORY '/mnt/data/events'",
];

fn has_node(sql: &str, kind: SyntaxKind) -> bool {
    parse_with_dialect(sql, Dialect::Databricks)
        .syntax()
        .descendants()
        .any(|node| node.kind() == kind)
}

#[test]
fn all_cases_parse_clean_and_round_trip() {
    for sql in CASES {
        let parsed = parse_with_dialect(sql, Dialect::Databricks);
        assert_eq!(
            parsed.syntax().to_string(),
            *sql,
            "Databricks parse tree must round-trip for {sql:?}"
        );
        assert!(
            parsed.errors().is_empty(),
            "unexpected Databricks parse errors for {sql:?}: {:?}",
            parsed.errors()
        );
    }
}

// ---- structural assertions ----

#[test]
fn vacuum_is_structured() {
    assert!(has_node("VACUUM t", SyntaxKind::VACUUM_STMT));
    assert!(has_node(
        "VACUUM t RETAIN 0 HOURS DRY RUN",
        SyntaxKind::VACUUM_STMT
    ));
}

#[test]
fn vacuum_no_longer_mis_splits() {
    // The regression guard from the report: `VACUUM t` must be ONE statement, not two bare
    // identifier statements. The source file has exactly one child statement, a `VACUUM_STMT`.
    let root = parse_with_dialect("VACUUM t", Dialect::Databricks).syntax();
    let stmts: Vec<_> = root.children().collect();
    assert_eq!(stmts.len(), 1, "VACUUM t must be a single statement");
    assert_eq!(stmts[0].kind(), SyntaxKind::VACUUM_STMT);
}

#[test]
fn optimize_and_zorder_are_structured() {
    assert!(has_node("OPTIMIZE t", SyntaxKind::OPTIMIZE_STMT));
    assert!(has_node(
        "OPTIMIZE t ZORDER BY (a, b)",
        SyntaxKind::OPTIMIZE_STMT
    ));
    assert!(has_node(
        "OPTIMIZE t ZORDER BY (a, b)",
        SyntaxKind::ZORDER_CLAUSE
    ));
    // The ZORDER column list reuses the ordinary parenthesized COLUMN_LIST node.
    assert!(has_node(
        "OPTIMIZE t ZORDER BY (a, b)",
        SyntaxKind::COLUMN_LIST
    ));
    // The WHERE predicate reuses the ordinary WHERE_CLAUSE node.
    assert!(has_node("OPTIMIZE t WHERE a > 1", SyntaxKind::WHERE_CLAUSE));
}

#[test]
fn insert_overwrite_is_an_insert_stmt() {
    // INSERT OVERWRITE reuses the INSERT_STMT node (so existing INSERT machinery applies).
    assert!(has_node(
        "INSERT OVERWRITE TABLE t SELECT * FROM s",
        SyntaxKind::INSERT_STMT
    ));
    assert!(has_node(
        "INSERT OVERWRITE TABLE t PARTITION (dt = '2024-01-01') SELECT a FROM s",
        SyntaxKind::INSERT_STMT
    ));
    // It still produces exactly one statement (no mis-split on the bare table word).
    let root = parse_with_dialect(
        "INSERT OVERWRITE TABLE t SELECT * FROM s",
        Dialect::Databricks,
    )
    .syntax();
    assert_eq!(root.children().count(), 1);
}

#[test]
fn merge_extensions_reuse_merge_nodes() {
    for sql in [
        "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT *",
        "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE THEN DELETE",
    ] {
        assert!(has_node(sql, SyntaxKind::MERGE_STMT), "{sql}");
        assert!(has_node(sql, SyntaxKind::MERGE_WHEN), "{sql}");
    }
}

#[test]
fn cache_uncache_refresh_describe_history_are_structured() {
    assert!(has_node("CACHE TABLE t", SyntaxKind::CACHE_STMT));
    assert!(has_node(
        "CACHE TABLE t AS SELECT * FROM s",
        SyntaxKind::CACHE_STMT
    ));
    // The CACHE defining query is a real SELECT subtree.
    assert!(has_node(
        "CACHE TABLE t AS SELECT * FROM s",
        SyntaxKind::SELECT_STMT
    ));
    assert!(has_node("UNCACHE TABLE t", SyntaxKind::UNCACHE_STMT));
    assert!(has_node("REFRESH TABLE t", SyntaxKind::REFRESH_STMT));
    assert!(has_node(
        "DESCRIBE HISTORY t",
        SyntaxKind::DESCRIBE_HISTORY_STMT
    ));
    assert!(has_node(
        "DESC HISTORY t",
        SyntaxKind::DESCRIBE_HISTORY_STMT
    ));
}

// ---- cross-dialect guards: these must NOT be recognized under Snowflake ----

#[test]
fn delta_commands_are_not_recognized_under_snowflake() {
    // Under Snowflake, the leading words stay ordinary identifiers (they are not reserved and not
    // recognized contextually), so the dedicated nodes are never produced. Every text still
    // round-trips losslessly and never panics.
    for sql in CASES {
        let parsed = parse_with_dialect(sql, Dialect::Snowflake);
        assert_eq!(
            parsed.syntax().to_string(),
            *sql,
            "Snowflake must round-trip losslessly for {sql:?}"
        );
        let has_delta_node = parsed.syntax().descendants().any(|n| {
            matches!(
                n.kind(),
                SyntaxKind::VACUUM_STMT
                    | SyntaxKind::OPTIMIZE_STMT
                    | SyntaxKind::ZORDER_CLAUSE
                    | SyntaxKind::CACHE_STMT
                    | SyntaxKind::UNCACHE_STMT
                    | SyntaxKind::REFRESH_STMT
                    | SyntaxKind::DESCRIBE_HISTORY_STMT
            )
        });
        assert!(
            !has_delta_node,
            "Snowflake must not produce a Delta-command node for {sql:?}"
        );
    }
}

#[test]
fn vacuum_is_two_bare_statements_under_snowflake() {
    // Confirms the Snowflake side is intentionally unchanged: `VACUUM t` is two ordinary expression
    // statements there (the historical behavior), not a VACUUM statement.
    let root = parse_with_dialect("VACUUM t", Dialect::Snowflake).syntax();
    assert!(root
        .descendants()
        .all(|n| n.kind() != SyntaxKind::VACUUM_STMT));
    assert_eq!(root.to_string(), "VACUUM t");
}

#[test]
fn command_words_are_plain_identifiers_in_expression_position_under_databricks() {
    // The contextual leading words must still work as ordinary identifiers when they are not at a
    // statement start that looks like a command — e.g. as select items / column names.
    for sql in [
        "SELECT vacuum, optimize, cache, refresh, uncache FROM t",
        "SELECT history, zorder, retain FROM t",
        "SELECT a FROM optimize",
    ] {
        let parsed = parse_with_dialect(sql, Dialect::Databricks);
        assert!(
            parsed.errors().is_empty(),
            "Databricks should treat command words as identifiers here: {sql:?} {:?}",
            parsed.errors()
        );
        assert_eq!(parsed.syntax().to_string(), sql);
    }
}
