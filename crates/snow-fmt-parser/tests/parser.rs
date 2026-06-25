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
        "GRANT SELECT, INSERT ON TABLE db.s.t TO ROLE analyst",
        "REVOKE USAGE ON WAREHOUSE wh FROM ROLE r",
        "CALL db.sch.proc(1, 2, 'x')",
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
        "SELECT * FROM t AT (TIMESTAMP => '2024-01-01'::timestamp)",
        "SELECT * FROM orders BEFORE (STATEMENT => 'abc') o",
        "BEGIN\nLET x := 1;\nRETURN x;\nEND",
        "DECLARE\nv INT DEFAULT 0;\nBEGIN\nIF (v > 0) THEN\nRETURN 1;\nELSE\nRETURN 0;\nEND IF;\nEND",
        "BEGIN\nFOR i IN 1 TO 3 DO\nINSERT INTO t VALUES (i);\nEND FOR;\nEND",
        "BEGIN\nIF (x > 0) THEN\nRETURN 1;\nEND", // malformed: missing END IF — must still round-trip
        "SELECT )( garbage @ # FROM", // deliberately broken
    ];
    for s in inputs {
        assert_parse_roundtrip(s);
    }
}

#[test]
fn scripting_blocks_parse_into_structured_nodes() {
    let p = parse("DECLARE\nv INT DEFAULT 0;\nBEGIN\nIF (v > 0) THEN\nv := 1;\nEND IF;\nEND");
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    let block = p
        .syntax()
        .children()
        .find(|n| n.kind() == SyntaxKind::BLOCK_STMT)
        .expect("a BLOCK_STMT");
    let kinds: Vec<SyntaxKind> = block.descendants().map(|n| n.kind()).collect();
    assert!(kinds.contains(&SyntaxKind::DECLARE_SECTION));
    assert!(kinds.contains(&SyntaxKind::IF_STMT));
    assert!(kinds.contains(&SyntaxKind::STMT_LIST));
}

#[test]
fn let_with_case_expression_is_not_split_at_inner_end() {
    // A `LET`/assignment whose right-hand side is a `CASE … END` expression must be one statement —
    // consume-to-`;` must not stop at the expression's inner `END`.
    let p =
        parse("BEGIN\nLET label := (CASE WHEN x > 0 THEN 'p' ELSE 'n' END);\nRETURN label;\nEND");
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    let block = p
        .syntax()
        .children()
        .find(|n| n.kind() == SyntaxKind::BLOCK_STMT)
        .expect("a BLOCK_STMT");
    let stmt_list = block
        .descendants()
        .find(|n| n.kind() == SyntaxKind::STMT_LIST)
        .expect("a STMT_LIST");
    // Exactly two statements in the body: the LET and the RETURN.
    assert_eq!(stmt_list.children().count(), 2);
}

#[test]
fn malformed_block_errors_so_formatter_keeps_it_verbatim() {
    // A block missing its `END IF` must not parse cleanly; the error makes the formatter fall back to
    // a verbatim copy rather than emitting a corrupted block.
    let p = parse("BEGIN\nIF (x > 0) THEN\nRETURN 1;\nEND");
    assert!(!p.errors().is_empty());
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
        "GRANT SELECT, INSERT ON TABLE db.s.t TO ROLE analyst",
        "GRANT SELECT (c1, c2) ON VIEW v TO ROLE reader",
        "GRANT OWNERSHIP ON TABLE t TO ROLE admin COPY CURRENT GRANTS",
        "REVOKE USAGE ON WAREHOUSE wh FROM ROLE r",
        "CALL refresh_all()",
        "CALL db.sch.load_data('2026-01-01', 42, TRUE)",
        "USE ROLE sysadmin",
        "USE WAREHOUSE compute_wh",
        "USE SCHEMA db.analytics",
        "SHOW TABLES IN SCHEMA db.s",
        "DESCRIBE TABLE db.s.t",
        "DESC USER u",
        "TRUNCATE TABLE db.s.t",
        "COMMENT ON TABLE db.s.t IS 'facts'",
        "COMMENT ON COLUMN db.s.t.c IS 'a column'",
        "COMMIT",
        "COMMIT WORK",
        "ROLLBACK",
        "ROLLBACK TO SAVEPOINT sp1",
        "BEGIN TRANSACTION",
        "BEGIN WORK",
        "BEGIN TRANSACTION NAME my_txn",
        "UNDROP TABLE db.s.t",
        "UNDROP SCHEMA db.s",
        // `DESC` as a sort direction must still parse inside ORDER BY, not as a DESCRIBE statement.
        "SELECT a FROM t ORDER BY a DESC, b ASC",
        // `comment` stays an ordinary identifier when not the head of a COMMENT ON statement.
        "SELECT comment, id FROM t WHERE comment IS NOT NULL",
    ] {
        assert_parse_clean(s);
    }
}

#[test]
fn transaction_and_undrop_with_operands_are_single_statements() {
    // Regression: `COMMIT WORK`, `ROLLBACK TO SAVEPOINT s`, and `UNDROP SCHEMA s` once parsed as
    // several bare-identifier statements, which the formatter split apart with inserted semicolons.
    for (sql, kind) in [
        ("COMMIT WORK", SyntaxKind::TRANSACTION_STMT),
        ("ROLLBACK TO SAVEPOINT sp1", SyntaxKind::TRANSACTION_STMT),
        ("UNDROP SCHEMA db.s", SyntaxKind::UNDROP_STMT),
    ] {
        let p = parse(sql);
        assert!(p.errors().is_empty(), "{sql} should parse cleanly");
        let stmts: Vec<_> = p.syntax().children().collect();
        assert_eq!(stmts.len(), 1, "{sql} must be a single statement");
        assert_eq!(stmts[0].kind(), kind);
    }
}

#[test]
fn begin_transaction_parses_but_scripting_block_stays_verbatim() {
    // `BEGIN TRANSACTION` / `BEGIN;` is a transaction statement and parses cleanly.
    let txn = parse("BEGIN TRANSACTION");
    assert!(txn.errors().is_empty());
    assert!(txn
        .syntax()
        .children()
        .any(|n| n.kind() == SyntaxKind::TRANSACTION_STMT));

    // A Snowflake Scripting block (`BEGIN <stmt>; … END`) must NOT be captured as a transaction —
    // it has no dedicated grammar yet, so it should error (and the formatter keeps it verbatim)
    // rather than splitting its `;`-separated body apart.
    let block = parse("BEGIN\nINSERT INTO t VALUES (1);\nINSERT INTO t VALUES (2);\nEND");
    let begins_a_txn = block
        .syntax()
        .descendants()
        .any(|n| n.kind() == SyntaxKind::TRANSACTION_STMT);
    assert!(
        !begins_a_txn,
        "a scripting BEGIN ... END block must not parse as a transaction statement"
    );
}

#[test]
fn comment_keyword_does_not_shadow_the_comment_identifier() {
    // `COMMENT ON ...` is a statement, but `comment` elsewhere is a plain identifier (a very common
    // column name) — the contextual keyword must only fire before `ON`.
    let stmt = parse("COMMENT ON TABLE t IS 'x'");
    assert!(stmt.errors().is_empty());
    assert!(stmt
        .syntax()
        .children()
        .any(|n| n.kind() == SyntaxKind::COMMENT_STMT));

    let col = parse("SELECT comment FROM t");
    assert!(col.errors().is_empty());
    assert!(
        !col.syntax()
            .descendants()
            .any(|n| n.kind() == SyntaxKind::COMMENT_STMT),
        "`comment` as a column must not parse as a COMMENT_STMT"
    );
}

#[test]
fn use_role_is_one_statement_not_split() {
    // Regression: `USE ROLE r` (no semicolons) once parsed as three bare-identifier statements,
    // which the formatter then split with inserted semicolons. It must be a single USE_STMT.
    let p = parse("USE ROLE sysadmin");
    assert!(p.errors().is_empty());
    let stmts: Vec<_> = p.syntax().children().collect();
    assert_eq!(stmts.len(), 1, "USE ROLE must be a single statement");
    assert_eq!(stmts[0].kind(), SyntaxKind::USE_STMT);
}

#[test]
fn call_parses_into_a_call_stmt() {
    let p = parse("CALL db.sch.proc(1, 2)");
    assert!(p.errors().is_empty());
    assert!(
        p.syntax()
            .children()
            .any(|n| n.kind() == SyntaxKind::CALL_STMT),
        "CALL should produce a CALL_STMT"
    );
}

#[test]
fn grant_and_revoke_parse_into_dedicated_nodes() {
    for (sql, kind) in [
        ("GRANT SELECT ON TABLE t TO ROLE r", SyntaxKind::GRANT_STMT),
        (
            "REVOKE SELECT ON TABLE t FROM ROLE r",
            SyntaxKind::REVOKE_STMT,
        ),
    ] {
        let p = parse(sql);
        assert!(p.errors().is_empty(), "{sql} should parse cleanly");
        assert!(
            p.syntax().children().any(|n| n.kind() == kind),
            "{sql} should produce a {kind:?}"
        );
    }
}

#[test]
fn create_object_kinds_without_a_body_parse_cleanly() {
    // Object kinds with no query body parse leniently into a CREATE_STMT and format inline.
    for s in [
        "CREATE SCHEMA IF NOT EXISTS analytics",
        "CREATE OR REPLACE DATABASE d CLONE src",
        "CREATE WAREHOUSE wh WITH WAREHOUSE_SIZE = XSMALL",
        "CREATE SEQUENCE seq START = 1 INCREMENT = 1",
        "CREATE STAGE st URL = 's3://b/p'",
        "CREATE OR REPLACE FILE FORMAT ff TYPE = JSON",
    ] {
        let p = parse(s);
        assert!(p.errors().is_empty(), "{s} should parse cleanly");
        assert!(
            p.syntax()
                .children()
                .any(|n| n.kind() == SyntaxKind::CREATE_STMT),
            "{s} should produce a CREATE_STMT"
        );
    }
}

#[test]
fn create_task_with_dml_body_is_structural() {
    // A body-bearing CREATE (e.g. a task's DML) now parses structurally: the property region and the
    // `AS <body>` are real nodes so the formatter can lay each on its own line (Phase 7 object DDL).
    let src = "CREATE TASK t WAREHOUSE = wh AS\nINSERT INTO log\nSELECT 1";
    let p = parse(src);
    assert!(
        p.errors().is_empty(),
        "CREATE TASK ... AS <dml> should parse cleanly: {:?}",
        p.errors()
    );
    assert_eq!(p.syntax().to_string(), src, "round-trip failed");
    let create = p
        .syntax()
        .children()
        .find(|n| n.kind() == SyntaxKind::CREATE_STMT)
        .expect("a CREATE_STMT");
    let kinds: Vec<SyntaxKind> = create.descendants().map(|n| n.kind()).collect();
    assert!(
        kinds.contains(&SyntaxKind::OBJECT_PROPERTY),
        "expected an OBJECT_PROPERTY for WAREHOUSE = wh: {kinds:?}"
    );
    assert!(
        kinds.contains(&SyntaxKind::INSERT_STMT),
        "expected the AS body to parse as an INSERT_STMT: {kinds:?}"
    );
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
