//! Exhaustive CREATE TABLE / CREATE VIEW / DROP coverage (Phase 7 core DDL).
//!
//! The matrix crosses *object* (TABLE, VIEW, DROP) with *shape* (plain column list, inline column
//! constraints, out-of-line table constraints, CLUSTER BY, CLONE, CTAS, SECURE / MATERIALIZED /
//! RECURSIVE / TEMP modifiers, IF [NOT] EXISTS, qualified names, CASCADE / RESTRICT). Every case is
//! asserted to (1) parse with no errors, (2) format to valid SQL (reparses clean), (3) be
//! idempotent, and (4) preserve its meaningful tokens (formatting only changes trivia and keyword
//! casing). A few exact-string goldens pin the layout opinions.

use snow_fmt_formatter::{format, FormatOptions};
use snow_fmt_lexer::tokenize;
use snow_fmt_parser::parse;
use snow_fmt_syntax::SyntaxKind;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

/// The signature a faithful formatter must preserve: meaningful tokens, upper-cased, with the
/// synthesized `;` dropped. (Formatting only normalizes trivia and keyword/identifier casing — and
/// since we upper-case both sides here, a contextual keyword that the formatter up-cases still
/// compares equal to its lower-case input.)
fn signature(sql: &str) -> Vec<String> {
    tokenize(sql)
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

const CASES: &[&str] = &[
    // ---- CREATE TABLE: column lists & types ----
    "create table t (id int)",
    "create table t (id int, name varchar(100))",
    "create table mydb.sch.t (id number(38, 0), ts timestamp_ntz, payload variant)",
    "create table t (a int, b string, c boolean, d float, e number(10, 2))",
    // ---- inline column constraints ----
    "create table t (id int not null)",
    "create table t (id int default 0)",
    "create table t (name string default 'anon')",
    "create table t (id int not null default 0, name string)",
    "create table t (id int primary key)",
    "create table t (id int unique)",
    "create table t (a varchar(10) collate 'en-ci')",
    "create table t (a int comment 'the a column')",
    "create table t (id int not null default 0 unique comment 'pk-ish')",
    "create table t (id int autoincrement, name string)",
    // ---- out-of-line table constraints ----
    "create table t (a int, b int, primary key (a))",
    "create table t (a int, b int, primary key (a, b))",
    "create table t (a int, b int, unique (a, b))",
    "create table t (a int, b int, foreign key (b) references u (id))",
    "create table t (a int, constraint pk primary key (a))",
    "create table t (a int, b int, constraint fk foreign key (b) references u (id))",
    "create table t (a int, b int, c int, primary key (a), unique (b), foreign key (c) references u (id))",
    "create table t (a int, check (a > 0))",
    // ---- modifiers + IF NOT EXISTS + OR REPLACE ----
    "create or replace table t (a int)",
    "create temporary table t (a int)",
    "create temp table t (a int)",
    "create transient table t (a int)",
    "create volatile table t (a int)",
    "create local temporary table t (a int)",
    "create table if not exists t (a int)",
    "create or replace table if not exists t (a int)",
    // ---- CLUSTER BY ----
    "create table t (a int, b int) cluster by (a)",
    "create table t (a int, b int) cluster by (a, b)",
    "create or replace table t (id int) cluster by (id)",
    // ---- CLONE ----
    "create table t clone src",
    "create or replace table t clone mydb.sch.src",
    "create transient table t clone src",
    // ---- CTAS ----
    "create table t as select a, b from u",
    "create or replace table t as select * from u where x > 1",
    "create table t (x, y) as select a, b from s",
    "create table t as with c as (select 1 as n) select n from c",
    "create table t as select a from u union all select a from v",
    // ---- CREATE VIEW ----
    "create view v as select a from t",
    "create or replace view v as select * from t",
    "create secure view v as select a from t",
    "create materialized view mv as select a from t where a > 0",
    "create recursive view rv as select 1",
    "create or replace secure view v (a, b) as select a, b from t",
    "create view if not exists v as select a from t",
    "create view v (id, total) comment = 'a view' as select id, sum(x) from t group by id",
    "create or replace secure materialized view mv as select a from t",
    "create view v as select a from t join u on t.id = u.id where t.x > 0",
    // ---- DROP ----
    "drop table t",
    "drop table if exists t",
    "drop table if exists db.s.t",
    "drop table t cascade",
    "drop table if exists t cascade",
    "drop table if exists t restrict",
    "drop view v",
    "drop view if exists v",
    "drop view if exists db.s.v restrict",
];

#[test]
fn all_cases_parse_clean() {
    for sql in CASES {
        let errors = parse(sql).errors().to_vec();
        assert!(errors.is_empty(), "parse errors for {sql:?}: {errors:?}");
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
fn formatted_output_is_valid_sql() {
    for sql in CASES {
        let formatted = fmt(sql);
        let errors = parse(&formatted).errors().to_vec();
        assert!(
            errors.is_empty(),
            "formatted output is invalid for {sql:?}: {errors:?}\n---\n{formatted}"
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

// ---- exact-string goldens (layout opinions) ----

#[test]
fn create_table_inline_when_short() {
    assert_eq!(
        fmt("create table t (id int, name varchar(100))"),
        "CREATE TABLE t (id int, name varchar(100));\n"
    );
}

#[test]
fn inline_constraints_upcase_their_keywords() {
    assert_eq!(
        fmt("create or replace table t (id number(38,0) not null, name string default 'x', primary key (id))"),
        "CREATE OR REPLACE TABLE t (id number(38, 0) NOT NULL, name string DEFAULT 'x', PRIMARY KEY (id));\n"
    );
}

#[test]
fn out_of_line_constraints_upcase() {
    assert_eq!(
        fmt("create table t (a int, b int, constraint fk foreign key (b) references u (id))"),
        "CREATE TABLE t (a int, b int, CONSTRAINT fk FOREIGN KEY (b) REFERENCES u(id));\n"
    );
}

#[test]
fn cluster_by_upcases() {
    assert_eq!(
        fmt("create temporary table t (a int) cluster by (a)"),
        "CREATE TEMPORARY TABLE t (a int) CLUSTER BY (a);\n"
    );
}

#[test]
fn clone_upcases() {
    assert_eq!(
        fmt("create table t clone src"),
        "CREATE TABLE t CLONE src;\n"
    );
}

#[test]
fn ctas_puts_query_below() {
    assert_eq!(
        fmt("create table t as select a from u"),
        "CREATE TABLE t AS\nSELECT a\nFROM u;\n"
    );
}

#[test]
fn materialized_secure_view() {
    assert_eq!(
        fmt("create or replace secure materialized view mv as select a from t"),
        "CREATE OR REPLACE SECURE MATERIALIZED VIEW mv AS\nSELECT a\nFROM t;\n"
    );
}

#[test]
fn view_with_column_list_and_comment() {
    assert_eq!(
        fmt("create or replace secure view v (a, b) comment = 'hi' as select a, b from t"),
        "CREATE OR REPLACE SECURE VIEW v (a, b) COMMENT = 'hi' AS\nSELECT a, b\nFROM t;\n"
    );
}

#[test]
fn drop_with_cascade_restrict_upcases() {
    assert_eq!(
        fmt("drop table if exists t cascade"),
        "DROP TABLE IF EXISTS t CASCADE;\n"
    );
    assert_eq!(
        fmt("drop view if exists db.s.v restrict"),
        "DROP VIEW IF EXISTS db.s.v RESTRICT;\n"
    );
}

#[test]
fn long_column_list_explodes_one_per_line() {
    let out = format(
        "create table bigt (averylongcolumnnamehere int, anotherlongcolumnname varchar(255), yetanothercol number(38, 0), morecolumns string)",
        &FormatOptions::default(),
    );
    assert_eq!(
        out,
        "CREATE TABLE bigt (\n    \
           averylongcolumnnamehere int,\n    \
           anotherlongcolumnname varchar(255),\n    \
           yetanothercol number(38, 0),\n    \
           morecolumns string\n\
         );\n"
    );
}

// ---- a column named like a DDL word stays an identifier (verbatim, never up-cased) ----

#[test]
fn quoted_ddl_word_column_names_round_trip() {
    // `"comment"` / `"key"` are quoted identifiers, not keywords; they are emitted verbatim.
    let out = fmt("create table t (\"comment\" int, \"key\" string)");
    assert_eq!(out, "CREATE TABLE t (\"comment\" int, \"key\" string);\n");
}
