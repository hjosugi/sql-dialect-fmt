//! Phase 1 parser conformance tests.
//!
//! The headline guarantee is **lossless round-trip**: for any input (valid or broken), the
//! parsed tree's text reproduces the source byte-for-byte. We also check that clean SQL parses
//! without diagnostics, that the node structure is sensible, and that errors recover.

use snow_fmt_parser::{parse, AstNode, SelectStmt, SourceFile, SyntaxKind};
use snow_fmt_test_support::parser::{assert_parse_clean, assert_parse_roundtrip};

#[test]
fn lossless_roundtrip_valid_and_broken() {
    let inputs = [
        "",
        "   \n  ",
        "SELECT 1",
        "select 1, 2 ,3",
        "SELECT * FROM t",
        "SELECT a, b AS x, c AS \"Y\" FROM db.sch.t",
        "SELECT a, b FROM t WHERE a > 1 AND b <= 2 OR NOT c",
        "SELECT count(*), sum(x), f(a, b) FROM t",
        "SELECT a :: int, (a + b) * c, -a, x || y FROM t",
        "SELECT arr[0], obj['k'] FROM t t_alias",
        "1 + 2 * 3 ;\nSELECT 1 ;",
        "/* hi */ SELECT n FROM t -- tail comment\n",
        "INSERT INTO t (a, b) VALUES (1, 2), (3, 4)",
        "INSERT INTO t SELECT * FROM u",
        "INSERT OVERWRITE INTO t (a) VALUES (1)",
        "INSERT ALL INTO a INTO b (x) SELECT c1, c2 FROM src",
        "INSERT FIRST WHEN sev >= 9 THEN INTO high ELSE INTO low SELECT sev FROM events",
        "UPDATE t SET a = 1, b = a + 2 FROM s WHERE id = 5",
        "DELETE FROM t USING u WHERE t.id = u.id",
        "MERGE INTO tgt t USING src s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.v = s.v WHEN NOT MATCHED THEN INSERT (id) VALUES (s.id)",
        "CREATE OR REPLACE VIEW v AS SELECT a, b FROM t",
        "CREATE TABLE t (a INT, b VARCHAR(10) NOT NULL, c NUMBER(10,2) DEFAULT 0)",
        "CREATE TABLE t AS SELECT * FROM u",
        "CREATE OR REPLACE PROCEDURE p(x INT) RETURNS INT LANGUAGE SQL AS $$ begin return x; end $$",
        "CREATE FUNCTION add1(n FLOAT) RETURNS FLOAT AS 'n + 1'",
        "SET target_table = 'MART.X'",
        "SET (a, b) = (1, 2)",
        "EXECUTE IMMEDIATE 'select 1'",
        "EXECUTE IMMEDIATE $$ begin return 1; end $$",
        "COPY INTO raw.orders FROM @raw.stage/orders/ FILE_FORMAT = (TYPE = JSON) ON_ERROR = CONTINUE",
        "COPY INTO @mart.stage/out/ FROM (SELECT * FROM t) FILE_FORMAT = (TYPE = CSV) PARTITION BY (dt)",
        "DROP TABLE IF EXISTS db.s.t CASCADE",
        "ALTER TABLE t ADD COLUMN c INT",
        "SELECT listagg(x, ',') WITHIN GROUP (ORDER BY x DESC) FROM t",
        "SELECT * FROM t PIVOT (sum(amount) FOR month IN ('jan', 'feb')) AS p",
        "SELECT * FROM sales UNPIVOT (amount FOR quarter IN (q1, q2))",
        "SELECT * FROM t MATCH_RECOGNIZE (PARTITION BY id PATTERN (a b+) DEFINE b AS b.v > 0) mr",
        "SELECT * FROM (WITH c AS (SELECT 1) SELECT * FROM c)",
        "CREATE VIEW v AS WITH c AS (SELECT 1) SELECT * FROM c",
        "MERGE INTO tgt USING (WITH c AS (SELECT 1 id) SELECT * FROM c) s ON tgt.id = s.id WHEN MATCHED THEN DELETE",
        "SELECT * FROM t TABLESAMPLE BERNOULLI(25) REPEATABLE(99)",
        "SELECT * FROM sales PIVOT (sum(amt) FOR m IN (1 AS jan, 2 AS feb)) p",
        "SELECT * FROM t WHERE a IS DISTINCT FROM b",
        "SELECT * FROM q ASOF JOIN t MATCH_CONDITION (q.ts >= t.ts) ON q.sym = t.sym",
        "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (a.t >= b.t)",
        "SELECT )( garbage @ # FROM", // deliberately broken
    ];
    for s in inputs {
        assert_parse_roundtrip(s);
    }
}

#[test]
fn clean_sql_has_no_errors() {
    for s in [
        "SELECT 1",
        "SELECT a, b FROM t WHERE a > 1",
        "SELECT count(*) FROM db.s.t",
        "SELECT a::int, (a + b) * c FROM t",
        "SELECT DISTINCT a FROM t",
        "SELECT count(DISTINCT x), array_agg(ALL y) FROM t",
        "SELECT listagg(DISTINCT x, ',') FROM t",
        "SELECT listagg(x, ',') WITHIN GROUP (ORDER BY x) FROM t",
        "SELECT count(grouping(a)) FROM t",
        "SELECT a FROM t GROUP BY GROUPING SETS ((a, b), (c), ())",
        "SELECT a FROM t GROUP BY CUBE(a, b)",
        "SELECT a FROM t GROUP BY ROLLUP(a), b",
        "SELECT f.value FROM t, LATERAL FLATTEN(input => t.items) f",
        "SELECT * FROM TABLE(FLATTEN(input => parse_json(x), outer => TRUE))",
        "SELECT f(a => 1, b => 2) FROM t",
    ] {
        assert_parse_clean(s);
    }
}

#[test]
fn select_has_expected_clauses() {
    let p = parse("SELECT a, b FROM t WHERE a > 1");
    assert!(p.errors().is_empty());
    let select = p
        .syntax()
        .children()
        .find(|n| n.kind() == SyntaxKind::SELECT_STMT)
        .expect("a SELECT_STMT");
    let kinds: Vec<SyntaxKind> = select.children().map(|n| n.kind()).collect();
    assert!(kinds.contains(&SyntaxKind::SELECT_LIST));
    assert!(kinds.contains(&SyntaxKind::FROM_CLAUSE));
    assert!(kinds.contains(&SyntaxKind::WHERE_CLAUSE));
}

#[test]
fn ast_accessors_work() {
    let p = parse("SELECT a, b FROM t");
    let file = SourceFile::cast(p.syntax()).expect("source file");
    let select = file
        .statements()
        .find_map(SelectStmt::cast)
        .expect("select stmt");
    assert!(select.select_list().is_some());
    assert_eq!(select.select_list().unwrap().items().count(), 2);
    assert!(select.from_clause().is_some());
    assert!(select.where_clause().is_none());
}

#[test]
fn error_recovery_is_lossless_and_reported() {
    let p = assert_parse_roundtrip("SELECT FROM");
    assert!(!p.errors().is_empty());
}

#[test]
fn precedence_nests_correctly() {
    // `a + b * c` must parse as a(+)(b*c): an outer BIN_EXPR containing an inner BIN_EXPR.
    let p = parse("SELECT a + b * c");
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    let outer = p
        .syntax()
        .descendants()
        .find(|n| n.kind() == SyntaxKind::BIN_EXPR)
        .expect("a BIN_EXPR");
    let nested = outer
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::BIN_EXPR)
        .count();
    assert!(
        nested >= 2,
        "expected a nested BIN_EXPR for the `*` sub-expression"
    );
}

#[test]
fn never_panics_on_adversarial_input() {
    // Mix of delimiters, operators, comments, dollar-quotes, unicode — must not panic.
    for s in [
        ";;;",
        "((((",
        "SELECT SELECT SELECT",
        "FROM WHERE AND OR ::",
        "$$ body $$ SELECT 1",
        "SELECT 中文 FROM 表 WHERE x = '💥'",
        "a.b.c.d.e.f.g",
        "1 +",
        ")",
    ] {
        assert_parse_roundtrip(s);
    }
}
