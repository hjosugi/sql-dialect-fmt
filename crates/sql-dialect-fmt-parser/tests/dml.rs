//! Phase 6 DML: INSERT (single-table VALUES / SELECT, OVERWRITE, multi-table ALL/FIRST), UPDATE,
//! DELETE, and MERGE.
//!
//! Each case in the matrix must parse with zero diagnostics and round-trip byte-for-byte. The
//! structured cases additionally expose the nodes the formatter relies on (INSERT_STMT /
//! UPDATE_STMT / DELETE_STMT / MERGE_STMT plus their sub-clauses: INTO_CLAUSE, INSERT_WHEN,
//! SET_CLAUSE, ASSIGNMENT, MERGE_WHEN, VALUES_CLAUSE). Broken or partial input must still
//! round-trip losslessly and never panic.

use sql_dialect_fmt_parser::{parse, SyntaxKind};
use sql_dialect_fmt_test_support::parser::{assert_parse_clean, assert_parse_roundtrip};

/// Every DML shape in the matrix, asserted to parse with zero diagnostics and round-trip exactly.
const CLEAN: &[&str] = &[
    // ---- INSERT ... VALUES (single-table) ----
    "INSERT INTO t VALUES (1)",
    "INSERT INTO t VALUES (1, 2)",
    "INSERT INTO t (a, b) VALUES (1, 2)",
    "INSERT INTO t (a, b) VALUES (1, 2), (3, 4)",
    "INSERT INTO mydb.sch.t (a) VALUES (DEFAULT)",
    "INSERT INTO t VALUES (1, 'x', TRUE, NULL)",
    "INSERT INTO t (a, b) VALUES (a + 1, b * 2)",
    // ---- INSERT ... SELECT (single-table) ----
    "INSERT INTO t SELECT * FROM u",
    "INSERT INTO t SELECT a, b FROM u",
    "INSERT INTO t (a, b) SELECT a, b FROM u WHERE a > 0",
    "INSERT INTO t SELECT a FROM u UNION ALL SELECT a FROM v",
    "INSERT INTO t WITH c AS (SELECT 1 AS n) SELECT n FROM c",
    "INSERT INTO t SELECT a FROM u JOIN v ON u.id = v.id",
    // ---- INSERT OVERWRITE ----
    "INSERT OVERWRITE INTO t (a) VALUES (1)",
    "INSERT OVERWRITE INTO t SELECT * FROM u",
    "INSERT OVERWRITE INTO mydb.sch.t (a, b) VALUES (1, 2)",
    // ---- multi-table INSERT ALL (unconditional) ----
    "INSERT ALL INTO a INTO b (x) SELECT c1, c2 FROM src",
    "INSERT ALL INTO t1 VALUES (1) INTO t2 VALUES (2) SELECT 1, 2",
    "INSERT ALL INTO t1 (a) VALUES (c1) INTO t2 (b) VALUES (c2) SELECT c1, c2 FROM src",
    "INSERT OVERWRITE ALL INTO t1 INTO t2 SELECT a, b FROM src",
    // ---- multi-table INSERT ALL/FIRST (conditional) ----
    "INSERT ALL WHEN c1 > 0 THEN INTO t1 WHEN c1 < 0 THEN INTO t2 SELECT c1 FROM src",
    "INSERT FIRST WHEN sev >= 9 THEN INTO high ELSE INTO low SELECT sev FROM events",
    "INSERT FIRST WHEN a > 1 THEN INTO t1 WHEN a > 2 THEN INTO t2 ELSE INTO t3 SELECT a FROM s",
    "INSERT ALL WHEN c1 > 0 THEN INTO t1 VALUES (c1) ELSE INTO t2 VALUES (c2) SELECT c1, c2 FROM src",
    // ---- UPDATE ----
    "UPDATE t SET a = 1",
    "UPDATE t SET a = 1, b = a + 2",
    "UPDATE t SET a = 1 WHERE id = 5",
    "UPDATE t SET a = 1, b = a + 2 WHERE id = 5",
    "UPDATE t SET a = s.x FROM s WHERE t.id = s.id",
    "UPDATE t SET a = 1, b = a + 2 FROM s WHERE id = 5",
    "UPDATE mydb.sch.t SET a = NULL WHERE a IS NOT NULL",
    "UPDATE t SET a = (SELECT max(x) FROM u) WHERE id = 1",
    // ---- DELETE ----
    "DELETE FROM t",
    "DELETE FROM t WHERE x > 0",
    "DELETE FROM mydb.sch.t WHERE a IS NULL",
    "DELETE FROM t USING u WHERE t.id = u.id",
    "DELETE FROM t USING u, v WHERE t.id = u.id AND t.k = v.k",
    "DELETE FROM t USING (SELECT id FROM u WHERE x > 0) u WHERE t.id = u.id",
    // ---- MERGE ----
    "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN DELETE",
    "MERGE INTO tgt t USING src s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.v = s.v",
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT (id) VALUES (s.id)",
    "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT (id, v) VALUES (s.id, s.v)",
    "MERGE INTO tgt t USING src s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.v = s.v WHEN NOT MATCHED THEN INSERT (id) VALUES (s.id)",
    "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED AND s.del = TRUE THEN DELETE WHEN MATCHED THEN UPDATE SET t.v = s.v WHEN NOT MATCHED THEN INSERT (id) VALUES (s.id)",
    "MERGE INTO t USING (SELECT * FROM s) s ON t.id = s.id WHEN MATCHED THEN DELETE",
    "MERGE INTO t USING (WITH c AS (SELECT 1 id) SELECT * FROM c) s ON t.id = s.id WHEN MATCHED THEN DELETE",
    "MERGE INTO t USING s ON t.id = s.id AND t.k = s.k WHEN MATCHED THEN UPDATE SET t.v = s.v",
    "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED AND s.flag THEN DELETE WHEN NOT MATCHED AND s.ok THEN INSERT (id) VALUES (s.id)",
];

#[test]
fn all_dml_parses_clean_and_round_trips() {
    for s in CLEAN {
        assert_parse_clean(s);
    }
}

fn has(s: &str, kind: SyntaxKind) -> bool {
    parse(s).syntax().descendants().any(|n| n.kind() == kind)
}

fn count(s: &str, kind: SyntaxKind) -> usize {
    parse(s)
        .syntax()
        .descendants()
        .filter(|n| n.kind() == kind)
        .count()
}

#[test]
fn insert_values_exposes_structure() {
    let sql = "INSERT INTO t (a, b) VALUES (1, 2), (3, 4)";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::INSERT_STMT));
    assert!(has(sql, SyntaxKind::VALUES_CLAUSE));
    // Two parenthesized rows.
    assert_eq!(count(sql, SyntaxKind::VALUES_ROW), 2);
}

#[test]
fn insert_select_exposes_inner_query() {
    let sql = "INSERT INTO t SELECT a, b FROM u";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::INSERT_STMT));
    assert!(has(sql, SyntaxKind::SELECT_STMT));
}

#[test]
fn multi_table_insert_all_exposes_into_clauses() {
    // Two unconditional targets => two INTO_CLAUSE nodes.
    let sql = "INSERT ALL INTO a INTO b (x) SELECT c1, c2 FROM src";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::INSERT_STMT));
    assert_eq!(count(sql, SyntaxKind::INTO_CLAUSE), 2);
    assert!(has(sql, SyntaxKind::SELECT_STMT));
}

#[test]
fn conditional_insert_first_exposes_when_and_into() {
    // Two WHEN branches + an ELSE branch => two INSERT_WHEN, three INTO_CLAUSE total.
    let sql =
        "INSERT FIRST WHEN a > 1 THEN INTO t1 WHEN a > 2 THEN INTO t2 ELSE INTO t3 SELECT a FROM s";
    assert_parse_clean(sql);
    assert_eq!(count(sql, SyntaxKind::INSERT_WHEN), 2);
    assert_eq!(count(sql, SyntaxKind::INTO_CLAUSE), 3);
}

#[test]
fn update_exposes_set_and_assignments() {
    let sql = "UPDATE t SET a = 1, b = a + 2 FROM s WHERE id = 5";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::UPDATE_STMT));
    assert!(has(sql, SyntaxKind::SET_CLAUSE));
    assert_eq!(count(sql, SyntaxKind::ASSIGNMENT), 2);
    assert!(has(sql, SyntaxKind::FROM_CLAUSE));
    assert!(has(sql, SyntaxKind::WHERE_CLAUSE));
}

#[test]
fn delete_exposes_statement_and_where() {
    let sql = "DELETE FROM t USING u WHERE t.id = u.id";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::DELETE_STMT));
    assert!(has(sql, SyntaxKind::WHERE_CLAUSE));
}

#[test]
fn merge_exposes_each_when_clause() {
    // One MATCHED-UPDATE and one NOT-MATCHED-INSERT => two MERGE_WHEN nodes.
    let sql = "MERGE INTO tgt t USING src s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.v = s.v WHEN NOT MATCHED THEN INSERT (id) VALUES (s.id)";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::MERGE_STMT));
    assert_eq!(count(sql, SyntaxKind::MERGE_WHEN), 2);
    // The UPDATE branch carries a SET_CLAUSE; the INSERT branch a VALUES_CLAUSE.
    assert!(has(sql, SyntaxKind::SET_CLAUSE));
    assert!(has(sql, SyntaxKind::VALUES_CLAUSE));
}

#[test]
fn merge_with_subquery_source_exposes_inner_select() {
    let sql = "MERGE INTO t USING (SELECT * FROM s) s ON t.id = s.id WHEN MATCHED THEN DELETE";
    assert_parse_clean(sql);
    assert!(has(sql, SyntaxKind::MERGE_STMT));
    assert!(has(sql, SyntaxKind::SELECT_STMT));
    assert_eq!(count(sql, SyntaxKind::MERGE_WHEN), 1);
}

#[test]
fn dml_round_trips_with_broken_or_partial_input() {
    // Must stay lossless even when incomplete (never panic, never lose bytes).
    for s in [
        "INSERT",
        "INSERT INTO",
        "INSERT INTO t",
        "INSERT INTO t (",
        "INSERT INTO t (a,",
        "INSERT INTO t VALUES",
        "INSERT INTO t VALUES (",
        "INSERT ALL",
        "INSERT FIRST WHEN",
        "INSERT FIRST WHEN a > 1 THEN",
        "UPDATE",
        "UPDATE t",
        "UPDATE t SET",
        "UPDATE t SET a =",
        "DELETE",
        "DELETE FROM",
        "DELETE FROM t USING",
        "MERGE",
        "MERGE INTO",
        "MERGE INTO t USING",
        "MERGE INTO t USING s ON",
        "MERGE INTO t USING s ON t.id = s.id WHEN",
        "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN",
    ] {
        assert_parse_roundtrip(s);
        // And parsing them must not panic — reaching here proves it didn't.
    }
}

#[test]
fn dml_word_as_quoted_identifier_round_trips() {
    // Words that merely *look* like DML keywords stay identifiers when quoted.
    for s in [
        "INSERT INTO t (\"values\", \"merge\") VALUES (1, 2)",
        "UPDATE t SET \"update\" = 1 WHERE \"delete\" = 0",
    ] {
        assert_parse_clean(s);
    }
}
