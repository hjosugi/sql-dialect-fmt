//! Formatter invariant matrix + goldens for the Databricks/Delta maintenance + cache statements:
//! `VACUUM`, `OPTIMIZE … ZORDER BY`, `RESTORE`, `ANALYZE TABLE`, `MSCK REPAIR TABLE`,
//! `INSERT OVERWRITE`, the `MERGE` extensions, `CACHE`/`UNCACHE`/`REFRESH`, and
//! `DESCRIBE HISTORY`.
//!
//! Every case is asserted under the **Databricks** dialect to:
//!   1. parse with no errors,
//!   2. round-trip byte-exact losslessly,
//!   3. format idempotently,
//!   4. re-parse clean after formatting, and
//!   5. preserve its case-folded significant-token stream.
//!
//! A block of exact-string goldens pins the layout. A final guard confirms the same text under
//! **Snowflake** never panics and round-trips losslessly (the words stay plain identifiers there).

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::tokenize_for_dialect;
use sql_dialect_fmt_parser::parse_with_dialect;
use sql_dialect_fmt_syntax::{Dialect, SyntaxKind};

fn fmt(src: &str) -> String {
    format(
        src,
        &FormatOptions::default().with_dialect(Dialect::Databricks),
    )
}

/// Case-folded significant tokens under the Databricks dialect (drops trivia + synthesized `;`).
fn signature(sql: &str) -> Vec<String> {
    tokenize_for_dialect(sql, Dialect::Databricks)
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

/// Every case parses clean under Databricks, so all five invariants apply.
const CASES: &[&str] = &[
    // ---- VACUUM ----
    "vacuum t",
    "vacuum main.default.events",
    "vacuum '/mnt/data/events'",
    "vacuum t retain 168 hours",
    "vacuum t retain 0 hours dry run",
    "vacuum t dry run",
    // ---- OPTIMIZE ----
    "optimize t",
    "optimize t where a > 1",
    "optimize t zorder by (a)",
    "optimize t zorder by (a, b, c)",
    "optimize t where dt = '2024-01-01' zorder by (id, ts)",
    // ---- INSERT OVERWRITE ----
    "insert overwrite table t select * from s",
    "insert overwrite t select * from s",
    "insert overwrite table t partition (dt = '2024-01-01') select a, b from s",
    "insert overwrite table t values (1, 2), (3, 4)",
    "insert overwrite table t (a, b) select a, b from s",
    "insert into t (a, b) values (1, 2)",
    // ---- MERGE extensions ----
    "merge into t using s on t.id = s.id when not matched then insert *",
    "merge into t using s on t.id = s.id when not matched by target then insert *",
    "merge into t using s on t.id = s.id when not matched by source then delete",
    "merge into t using s on t.id = s.id when not matched by source then update set t.x = 0",
    "merge into t using s on t.id = s.id when matched then update set t.x = s.x when not matched then insert * when not matched by source then delete",
    "merge into t using s on t.id = s.id when not matched by source and t.flag = 1 then delete",
    // ---- CACHE / UNCACHE / REFRESH ----
    "cache table t",
    "cache lazy table t",
    "cache table t options ('storageLevel' = 'DISK_ONLY')",
    "cache table t as select * from s",
    "cache table t select * from s",
    "uncache table t",
    "uncache table if exists t",
    "refresh table t",
    "refresh t",
    "refresh '/mnt/data/events'",
    // ---- DESCRIBE HISTORY ----
    "describe history t",
    "desc history t",
    "describe history main.default.events",
    // ---- RESTORE / ANALYZE / MSCK REPAIR ----
    "restore table t to version as of 5",
    "restore t to timestamp as of '2024-01-01'",
    "analyze table t compute statistics",
    "analyze table t compute statistics for columns a, b",
    "msck repair table t",
    "msck repair table main.default.events sync partitions",
];

#[test]
fn all_cases_parse_clean() {
    for sql in CASES {
        let errors = parse_with_dialect(sql, Dialect::Databricks)
            .errors()
            .to_vec();
        assert!(
            errors.is_empty(),
            "Databricks parse errors for {sql:?}: {errors:?}"
        );
    }
}

#[test]
fn all_cases_round_trip_losslessly() {
    for sql in CASES {
        let tree = parse_with_dialect(sql, Dialect::Databricks)
            .syntax()
            .to_string();
        assert_eq!(&tree, sql, "Databricks parse tree must round-trip");
    }
}

#[test]
fn formatting_is_idempotent() {
    for sql in CASES {
        let once = fmt(sql);
        assert_eq!(once, fmt(&once), "not idempotent:\n{sql}\n---\n{once}");
    }
}

#[test]
fn formatted_output_is_valid_databricks_sql() {
    for sql in CASES {
        let formatted = fmt(sql);
        let errors = parse_with_dialect(&formatted, Dialect::Databricks)
            .errors()
            .to_vec();
        assert!(
            errors.is_empty(),
            "formatted output invalid under Databricks for {sql:?}: {errors:?}\n---\n{formatted}"
        );
    }
}

#[test]
fn formatting_preserves_tokens() {
    for sql in CASES {
        let formatted = fmt(sql);
        assert_eq!(
            signature(sql),
            signature(&formatted),
            "token sequence changed:\n{sql}\n---\n{formatted}"
        );
    }
}

// ---- exact-string goldens (layout opinions, pinned) ----

#[test]
fn golden_vacuum_one_line() {
    assert_eq!(fmt("vacuum t"), "VACUUM t;\n");
    assert_eq!(
        fmt("vacuum t retain 168 hours dry run"),
        "VACUUM t RETAIN 168 HOURS DRY RUN;\n"
    );
    assert_eq!(
        fmt("vacuum '/mnt/data/events' retain 0 hours"),
        "VACUUM '/mnt/data/events' RETAIN 0 HOURS;\n"
    );
}

#[test]
fn golden_optimize_with_where_and_zorder() {
    assert_eq!(fmt("optimize t"), "OPTIMIZE t;\n");
    assert_eq!(
        fmt("optimize t where a > 1 zorder by (a, b)"),
        "OPTIMIZE t\nWHERE a > 1\nZORDER BY (a, b);\n"
    );
    assert_eq!(
        fmt("optimize t zorder by (a, b, c)"),
        "OPTIMIZE t\nZORDER BY (a, b, c);\n"
    );
}

#[test]
fn golden_insert_overwrite() {
    assert_eq!(
        fmt("insert overwrite table t select * from s"),
        "INSERT OVERWRITE TABLE t\nSELECT *\nFROM s;\n"
    );
    assert_eq!(
        fmt("insert overwrite table t partition (dt = '2024-01-01') select a, b from s"),
        "INSERT OVERWRITE TABLE t PARTITION (dt = '2024-01-01')\nSELECT a, b\nFROM s;\n"
    );
}

#[test]
fn golden_merge_extensions() {
    assert_eq!(
        fmt("merge into t using s on t.id = s.id when not matched then insert *"),
        "MERGE INTO t\nUSING s\nON t.id = s.id\nWHEN NOT MATCHED THEN INSERT *;\n"
    );
    assert_eq!(
        fmt("merge into t using s on t.id = s.id when not matched by source then delete"),
        "MERGE INTO t\nUSING s\nON t.id = s.id\nWHEN NOT MATCHED BY SOURCE THEN DELETE;\n"
    );
}

#[test]
fn golden_cache_uncache_refresh() {
    assert_eq!(fmt("cache table t"), "CACHE TABLE t;\n");
    assert_eq!(fmt("cache lazy table t"), "CACHE LAZY TABLE t;\n");
    assert_eq!(
        fmt("cache table t as select * from s"),
        "CACHE TABLE t AS\nSELECT *\nFROM s;\n"
    );
    assert_eq!(
        fmt("uncache table if exists t"),
        "UNCACHE TABLE IF EXISTS t;\n"
    );
    assert_eq!(fmt("refresh table t"), "REFRESH TABLE t;\n");
}

#[test]
fn golden_describe_history() {
    assert_eq!(fmt("describe history t"), "DESCRIBE HISTORY t;\n");
    assert_eq!(fmt("desc history t"), "DESC HISTORY t;\n");
}

#[test]
fn golden_restore_analyze_msck() {
    assert_eq!(
        fmt("restore table t to version as of 5"),
        "RESTORE TABLE t TO VERSION AS OF 5;\n"
    );
    assert_eq!(
        fmt("analyze table t compute statistics for columns a, b"),
        "ANALYZE TABLE t COMPUTE STATISTICS FOR COLUMNS a, b;\n"
    );
    assert_eq!(
        fmt("msck repair table main.default.events sync partitions"),
        "MSCK REPAIR TABLE main.default.events SYNC PARTITIONS;\n"
    );
}

// ---- cross-dialect guard: Snowflake never panics and round-trips losslessly ----

#[test]
fn snowflake_never_panics_and_round_trips() {
    for sql in CASES {
        // Parser side: lossless round-trip under Snowflake (words stay plain identifiers).
        let tree = parse_with_dialect(sql, Dialect::Snowflake)
            .syntax()
            .to_string();
        assert_eq!(
            &tree, sql,
            "Snowflake must round-trip losslessly for {sql:?}"
        );
        // Formatter side: must produce some output without panicking, and be idempotent.
        let once = format(sql, &FormatOptions::default());
        let twice = format(&once, &FormatOptions::default());
        assert_eq!(
            once, twice,
            "Snowflake formatting must be idempotent:\n{sql}"
        );
    }
}
