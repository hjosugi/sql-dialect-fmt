//! Structured ALTER statements (issue #30): `ALTER TABLE / VIEW / SESSION / WAREHOUSE / TASK`
//! (plus SCHEMA / DATABASE / MATERIALIZED VIEW / DYNAMIC TABLE) expose the object head as a
//! `NAME_REF` and each action clause as an `ALTER_ACTION`, with `SET key = value` pairs structured
//! as `OBJECT_PROPERTY` children. Unmodeled object kinds keep the historical lenient flat run.
//!
//! Every accepted form must parse diagnostic-free and round-trip byte-for-byte; broken/partial
//! input must still round-trip and never panic.

use sql_dialect_fmt_parser::{parse, parse_with_dialect, Dialect, SyntaxKind};
use sql_dialect_fmt_test_support::parser::{
    assert_has_node_kind, assert_parse_clean as clean, assert_parse_recovers as recovers,
};

fn count_nodes(sql: &str, kind: SyntaxKind) -> usize {
    parse(sql)
        .syntax()
        .descendants()
        .filter(|node| node.kind() == kind)
        .count()
}

#[test]
fn alter_table_column_actions_parse_clean_and_expose_actions() {
    for (sql, actions) in [
        ("ALTER TABLE t ADD COLUMN c INT", 1),
        ("ALTER TABLE t ADD COLUMN IF NOT EXISTS c NUMBER(10, 2)", 1),
        ("ALTER TABLE t DROP COLUMN c", 1),
        ("ALTER TABLE t DROP COLUMN a, b", 1), // one action drops several columns
        ("ALTER TABLE t RENAME COLUMN a TO b", 1),
        ("ALTER TABLE t RENAME TO t2", 1),
        (
            "ALTER TABLE db.sch.t ADD COLUMN a INT, ADD COLUMN b STRING",
            2,
        ),
        (
            "ALTER TABLE t ADD COLUMN a INT, DROP COLUMN b, RENAME COLUMN c TO d",
            3,
        ),
        (
            "ALTER TABLE t ALTER COLUMN c SET DEFAULT CURRENT_TIMESTAMP()",
            1,
        ),
        ("ALTER TABLE t MODIFY COLUMN c SET NOT NULL", 1),
        ("ALTER TABLE IF EXISTS t DROP COLUMN c", 1),
        ("ALTER TABLE t SWAP WITH u", 1),
        ("ALTER TABLE t CLUSTER BY (a, b)", 1),
        ("ALTER TABLE t DROP CONSTRAINT pk CASCADE", 1),
    ] {
        clean(sql);
        assert_has_node_kind(sql, SyntaxKind::ALTER_STMT);
        assert_has_node_kind(sql, SyntaxKind::NAME_REF);
        assert_eq!(
            count_nodes(sql, SyntaxKind::ALTER_ACTION),
            actions,
            "unexpected action count for {sql:?}"
        );
    }
}

#[test]
fn alter_table_governance_actions_split_on_action_commas_only() {
    // Commas inside USING (...) and before non-action words stay inside their action.
    let sql = "ALTER TABLE t MODIFY COLUMN email SET MASKING POLICY p USING(email,tenant_id),\
               MODIFY COLUMN email SET TAG g.c = 'restricted',SET TAG g.r = 'standard'";
    clean(sql);
    assert_eq!(count_nodes(sql, SyntaxKind::ALTER_ACTION), 3);

    let search = "ALTER TABLE t ADD SEARCH OPTIMIZATION ON EQUALITY(a, b), SUBSTRING(c)";
    clean(search);
    assert_eq!(count_nodes(search, SyntaxKind::ALTER_ACTION), 1);
}

#[test]
fn alter_session_set_structures_property_pairs() {
    let sql = "ALTER SESSION SET TIMEZONE = 'UTC', WEEK_START = 1, QUERY_TAG = 'etl'";
    clean(sql);
    assert_eq!(count_nodes(sql, SyntaxKind::ALTER_ACTION), 1);
    assert_eq!(count_nodes(sql, SyntaxKind::OBJECT_PROPERTY), 3);

    let unset = "ALTER SESSION UNSET QUERY_TAG, WEEK_START";
    clean(unset);
    assert_eq!(count_nodes(unset, SyntaxKind::ALTER_ACTION), 1);
}

#[test]
fn alter_warehouse_task_and_view_forms_parse_clean() {
    for sql in [
        "ALTER WAREHOUSE wh SET WAREHOUSE_SIZE = 'LARGE' AUTO_SUSPEND = 60",
        "ALTER WAREHOUSE wh SUSPEND",
        "ALTER WAREHOUSE IF EXISTS wh RESUME",
        "ALTER TASK t RESUME",
        "ALTER TASK ops.t SUSPEND",
        "ALTER TASK t SET SCHEDULE = '5 minutes', SUSPEND_TASK_AFTER_NUM_FAILURES = 3",
        "ALTER VIEW v RENAME TO v2",
        "ALTER VIEW v SET SECURE",
        "ALTER SCHEMA s RENAME TO s2",
        "ALTER DATABASE d SET DATA_RETENTION_TIME_IN_DAYS = 7",
        "ALTER MATERIALIZED VIEW mv RENAME TO mv2",
        "ALTER DYNAMIC TABLE dt REFRESH",
    ] {
        clean(sql);
        assert_has_node_kind(sql, SyntaxKind::ALTER_STMT);
        assert_has_node_kind(sql, SyntaxKind::ALTER_ACTION);
    }
}

#[test]
fn alter_set_properties_are_object_properties() {
    // The space-separated (no comma) property list is the Snowflake warehouse/task style.
    let sql = "ALTER WAREHOUSE wh SET WAREHOUSE_SIZE = 'LARGE' AUTO_SUSPEND = 60";
    clean(sql);
    assert_eq!(count_nodes(sql, SyntaxKind::OBJECT_PROPERTY), 2);
}

#[test]
fn case_insensitive_and_lowercase_spellings_parse_clean() {
    clean("alter table t add column c int");
    clean("Alter Session Set timezone = 'UTC'");
    clean("alter task t resume");
    clean("aLtEr TaBlE t ReNaMe To u");
}

#[test]
fn unmodeled_alter_kinds_stay_lenient_and_round_trip() {
    for sql in [
        "ALTER USER u SET PASSWORD = 'x' MUST_CHANGE_PASSWORD = TRUE",
        "ALTER PIPE p REFRESH",
        "ALTER STAGE s SET URL = 's3://bucket/'",
        "ALTER ACCOUNT SET STATEMENT_TIMEOUT_IN_SECONDS = 60",
        "ALTER FUNCTION f(INT) RENAME TO g",
    ] {
        let parsed = clean(sql);
        // The head is not one of the structured kinds, so no ALTER_ACTION is produced.
        assert!(
            !parsed
                .syntax()
                .descendants()
                .any(|n| n.kind() == SyntaxKind::ALTER_ACTION),
            "expected lenient flat run for {sql:?}"
        );
    }
}

#[test]
fn databricks_alter_table_forms_parse_clean() {
    for sql in [
        "ALTER TABLE t ADD COLUMNS (c1 INT, c2 STRING)",
        "ALTER TABLE t SET TBLPROPERTIES ('delta.appendOnly' = 'true')",
        "ALTER TABLE t RENAME TO u",
    ] {
        let parsed = parse_with_dialect(sql, Dialect::Databricks);
        assert_eq!(parsed.syntax().to_string(), sql, "round trip for {sql:?}");
        assert!(
            parsed.errors().is_empty(),
            "unexpected errors for {sql:?}: {:?}",
            parsed.errors()
        );
        assert!(parsed
            .syntax()
            .descendants()
            .any(|n| n.kind() == SyntaxKind::ALTER_ACTION));
    }
}

#[test]
fn alter_words_remain_usable_as_identifiers_elsewhere() {
    // None of the new contextual words (session, add, modify, rename, unset, suspend, resume,
    // swap, column, search, optimization) are reserved.
    clean("SELECT session, add, modify, rename, unset FROM t");
    clean("SELECT suspend, resume, swap, column1, search, optimization FROM t");
    clean("SELECT t.column FROM tbl t WHERE search > 1");
}

#[test]
fn alter_can_start_a_flow_chain() {
    let sql = "ALTER TABLE t ADD COLUMN c int ->> SELECT * FROM $1";
    clean(sql);
    assert_has_node_kind(sql, SyntaxKind::FLOW_STMT);
    assert_has_node_kind(sql, SyntaxKind::ALTER_ACTION);
}

#[test]
fn broken_and_partial_alter_round_trips_without_panic() {
    for sql in [
        "ALTER",
        "ALTER TABLE",
        "ALTER TABLE t",
        "ALTER TABLE t ADD",
        "ALTER TABLE t ADD COLUMN",
        "ALTER TABLE t ADD COLUMN c INT,",
        "ALTER TABLE t ADD COLUMN c INT, DROP",
        "ALTER TABLE IF",
        "ALTER SESSION SET",
        "ALTER SESSION SET TIMEZONE =",
        "ALTER SESSION SET TIMEZONE = 'UTC',",
        "ALTER TASK t SET SCHEDULE = ",
        "ALTER WAREHOUSE wh SET WAREHOUSE_SIZE =",
        "ALTER TABLE t SWAP WITH",
        "ALTER TABLE t CLUSTER BY (a,",
        "ALTER TABLE t MODIFY COLUMN c SET MASKING POLICY p USING(",
        "ALTER ,,, TABLE",
    ] {
        // Round-trips losslessly (diagnostics tolerated); the helper panics on lost bytes.
        recovers(sql);
    }
}
