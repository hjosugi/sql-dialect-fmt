//! Parser resilience and high-value Snowflake edge cases.
//!
//! The parser must be useful before it knows the entire Snowflake grammar: clean common SELECTs
//! should be diagnostic-free, while newer/unsupported statements must still round-trip and recover.

use sql_dialect_fmt_parser::SyntaxKind;
use sql_dialect_fmt_test_support::parser::{
    assert_has_node_kind, assert_parse_clean as clean, assert_parse_recovers as recovers,
};

#[test]
fn snowflake_select_trailing_commas_and_alias_edges() {
    clean("SELECT a, b, FROM t");
    clean("SELECT a AS x, b y, count(*) AS total FROM t");
    clean("SELECT t.* FROM db.schema.table_name AS t");
    clean("SELECT obj['k'], arr[0], payload::VARIANT FROM t");
    clean("SELECT \"長芋\" AS item_name FROM \"畑\" WHERE \"長芋\" LIKE '長芋%'");
}

#[test]
fn nested_predicates_and_subqueries_stay_structured() {
    let sql = "SELECT id FROM t WHERE (a = 1 OR b BETWEEN 2 AND 3) AND c NOT IN (SELECT c FROM u WHERE u.flag IS TRUE) AND EXISTS (SELECT 1 FROM v)";
    clean(sql);
    assert_has_node_kind(sql, SyntaxKind::BETWEEN_EXPR);
    assert_has_node_kind(sql, SyntaxKind::IN_EXPR);
    assert_has_node_kind(sql, SyntaxKind::SUBQUERY);
}

#[test]
fn long_select_list_is_fast_and_lossless() {
    let mut sql = String::from("SELECT ");
    for i in 0..384 {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push('c');
        sql.push_str(&i.to_string());
        sql.push_str(" AS alias_");
        sql.push_str(&i.to_string());
    }
    sql.push_str(" FROM wide_table WHERE c0 IS NOT NULL ORDER BY c0 LIMIT 10");

    clean(&sql);
    assert_has_node_kind(&sql, SyntaxKind::SELECT_LIST);
    assert_has_node_kind(&sql, SyntaxKind::ORDER_BY_CLAUSE);
}

#[test]
fn long_cte_chain_is_lossless() {
    let mut sql = String::from("WITH ");
    for i in 0..64 {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push('c');
        sql.push_str(&i.to_string());
        sql.push_str(" AS (SELECT ");
        sql.push_str(&i.to_string());
        sql.push_str(" AS n)");
    }
    sql.push_str(" SELECT n FROM c63");

    clean(&sql);
    assert_has_node_kind(&sql, SyntaxKind::WITH_CLAUSE);
    assert_has_node_kind(&sql, SyntaxKind::CTE);
}

#[test]
fn unsupported_snowflake_statements_recover_losslessly() {
    for sql in [
        "SHOW TABLES ->> SELECT \"name\" FROM $1;",
        "CREATE OR REPLACE DYNAMIC TABLE dt TARGET_LAG = '1 minute' WAREHOUSE = wh AS SELECT * FROM t;",
        "COPY INTO @stage/path FROM (SELECT * FROM t) FILE_FORMAT = (TYPE = PARQUET);",
        "CREATE PROCEDURE p() RETURNS STRING LANGUAGE SQL AS $$ BEGIN RETURN 'ok'; END; $$;",
        "SELECT * FROM t MATCH_RECOGNIZE (PARTITION BY id ORDER BY ts PATTERN (a+));",
    ] {
        recovers(sql);
    }
}

#[test]
fn malformed_inputs_recover_losslessly() {
    for sql in [
        "SELECT ((((((((",
        "SELECT a FROM WHERE GROUP BY",
        "WITH c AS SELECT 1 SELECT * FROM c",
        "SELECT a FROM t WHERE a BETWEEN 1",
        "SELECT a FROM t WHERE a IN (1, 2,",
        "SELECT $$unterminated",
        "SELECT \"unterminated",
    ] {
        recovers(sql);
    }
}
