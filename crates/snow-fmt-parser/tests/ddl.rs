//! Phase 7 core DDL: CREATE TABLE / CREATE VIEW / DROP.
//!
//! Each case must parse cleanly, round-trip byte-for-byte, and (for the structured ones) expose the
//! nodes the formatter relies on (CREATE_STMT / DROP_STMT / COLUMN_DEF_LIST / COLUMN_DEF). Broken or
//! partial input must still round-trip losslessly and never panic.

use snow_fmt_parser::{parse, SyntaxKind};
use snow_fmt_test_support::parser::{assert_parse_clean, assert_parse_roundtrip};

/// Every DDL shape in the matrix, asserted to parse with zero diagnostics and round-trip exactly.
const CLEAN: &[&str] = &[
    // CREATE TABLE — columns & types
    "CREATE TABLE t (id INT)",
    "CREATE TABLE t (id INT, name VARCHAR(100))",
    "CREATE TABLE mydb.sch.t (id NUMBER(38, 0), ts TIMESTAMP_NTZ, payload VARIANT)",
    // inline column constraints
    "CREATE TABLE t (id INT NOT NULL)",
    "CREATE TABLE t (id INT DEFAULT 0)",
    "CREATE TABLE t (name STRING DEFAULT 'anon')",
    "CREATE TABLE t (id INT NOT NULL DEFAULT 0, name STRING)",
    "CREATE TABLE t (id INT PRIMARY KEY)",
    "CREATE TABLE t (id INT UNIQUE)",
    "CREATE TABLE t (a VARCHAR(10) COLLATE 'en-ci')",
    "CREATE TABLE t (a INT COMMENT 'the a column')",
    "CREATE TABLE t (id INT AUTOINCREMENT, name STRING)",
    // out-of-line table constraints
    "CREATE TABLE t (a INT, b INT, PRIMARY KEY (a))",
    "CREATE TABLE t (a INT, b INT, PRIMARY KEY (a, b))",
    "CREATE TABLE t (a INT, b INT, UNIQUE (a, b))",
    "CREATE TABLE t (a INT, b INT, FOREIGN KEY (b) REFERENCES u (id))",
    "CREATE TABLE t (a INT, CONSTRAINT pk PRIMARY KEY (a))",
    "CREATE TABLE t (a INT, b INT, CONSTRAINT fk FOREIGN KEY (b) REFERENCES u (id))",
    "CREATE TABLE t (a INT, CHECK (a > 0))",
    "CREATE TABLE t (a INT, b INT, c INT, PRIMARY KEY (a), UNIQUE (b), FOREIGN KEY (c) REFERENCES u (id))",
    // modifiers / IF NOT EXISTS / OR REPLACE
    "CREATE OR REPLACE TABLE t (a INT)",
    "CREATE TEMPORARY TABLE t (a INT)",
    "CREATE TRANSIENT TABLE t (a INT)",
    "CREATE VOLATILE TABLE t (a INT)",
    "CREATE LOCAL TEMPORARY TABLE t (a INT)",
    "CREATE TABLE IF NOT EXISTS t (a INT)",
    // CLUSTER BY
    "CREATE TABLE t (a INT, b INT) CLUSTER BY (a)",
    "CREATE TABLE t (a INT, b INT) CLUSTER BY (a, b)",
    // CLONE
    "CREATE TABLE t CLONE src",
    "CREATE OR REPLACE TABLE t CLONE mydb.sch.src",
    // CTAS
    "CREATE TABLE t AS SELECT a, b FROM u",
    "CREATE TABLE t (x, y) AS SELECT a, b FROM s",
    "CREATE TABLE t AS WITH c AS (SELECT 1 AS n) SELECT n FROM c",
    "CREATE TABLE t AS SELECT a FROM u UNION ALL SELECT a FROM v",
    // CREATE VIEW
    "CREATE VIEW v AS SELECT a FROM t",
    "CREATE OR REPLACE VIEW v AS SELECT * FROM t",
    "CREATE SECURE VIEW v AS SELECT a FROM t",
    "CREATE MATERIALIZED VIEW mv AS SELECT a FROM t WHERE a > 0",
    "CREATE RECURSIVE VIEW rv AS SELECT 1",
    "CREATE OR REPLACE SECURE VIEW v (a, b) AS SELECT a, b FROM t",
    "CREATE VIEW IF NOT EXISTS v AS SELECT a FROM t",
    "CREATE VIEW v (id, total) COMMENT = 'a view' AS SELECT id, sum(x) FROM t GROUP BY id",
    "CREATE OR REPLACE SECURE MATERIALIZED VIEW mv AS SELECT a FROM t",
    // DROP
    "DROP TABLE t",
    "DROP TABLE IF EXISTS t",
    "DROP TABLE IF EXISTS db.s.t",
    "DROP TABLE t CASCADE",
    "DROP TABLE IF EXISTS t CASCADE",
    "DROP TABLE IF EXISTS t RESTRICT",
    "DROP VIEW v",
    "DROP VIEW IF EXISTS v",
    "DROP VIEW IF EXISTS db.s.v RESTRICT",
];

#[test]
fn all_ddl_parses_clean_and_round_trips() {
    for s in CLEAN {
        assert_parse_clean(s);
    }
}

fn has(s: &str, kind: SyntaxKind) -> bool {
    parse(s).syntax().descendants().any(|n| n.kind() == kind)
}

#[test]
fn create_table_exposes_structure() {
    let sql = "CREATE TABLE t (id INT NOT NULL, name STRING, PRIMARY KEY (id))";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::CREATE_STMT));
    assert!(has(sql, SyntaxKind::COLUMN_DEF_LIST));
    assert!(has(sql, SyntaxKind::COLUMN_DEF));
}

#[test]
fn create_table_column_def_count() {
    // Two columns + one out-of-line constraint = three COLUMN_DEF entries.
    let sql = "CREATE TABLE t (a INT, b INT, PRIMARY KEY (a))";
    let p = parse(sql);
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    let list = p
        .syntax()
        .descendants()
        .find(|n| n.kind() == SyntaxKind::COLUMN_DEF_LIST)
        .expect("a COLUMN_DEF_LIST");
    let defs = list
        .children()
        .filter(|n| n.kind() == SyntaxKind::COLUMN_DEF)
        .count();
    assert_eq!(defs, 3, "expected 3 column/constraint defs");
}

#[test]
fn ctas_exposes_select_inside_create() {
    let sql = "CREATE TABLE t AS SELECT a FROM u";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::CREATE_STMT));
    assert!(has(sql, SyntaxKind::SELECT_STMT));
}

#[test]
fn create_view_exposes_select() {
    let sql = "CREATE OR REPLACE SECURE VIEW v AS SELECT a, b FROM t";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::CREATE_STMT));
    assert!(has(sql, SyntaxKind::SELECT_STMT));
}

#[test]
fn drop_exposes_statement_node() {
    for sql in ["DROP TABLE t", "DROP VIEW IF EXISTS v CASCADE"] {
        assert_parse_clean(sql);
        assert!(has(sql, SyntaxKind::DROP_STMT));
    }
}

#[test]
fn ddl_round_trips_with_broken_or_partial_input() {
    // Must stay lossless even when incomplete (never panic, never lose bytes).
    for s in [
        "CREATE TABLE",
        "CREATE TABLE t (",
        "CREATE TABLE t (a INT,",
        "CREATE OR REPLACE TABLE t (a INT, PRIMARY KEY (",
        "CREATE VIEW v AS",
        "DROP TABLE",
        "DROP TABLE IF EXISTS",
        "CREATE TABLE t CLONE",
    ] {
        assert_parse_roundtrip(s);
        // And parsing them must not panic — reaching here proves it didn't.
    }
}

#[test]
fn ddl_word_as_quoted_identifier_round_trips() {
    // Words that merely *look* like DDL options stay identifiers when quoted.
    for s in [
        "CREATE TABLE t (\"comment\" INT, \"key\" STRING)",
        "CREATE TABLE t (\"default\" INT)",
    ] {
        assert_parse_clean(s);
    }
}
