//! Comprehensive Databricks/Spark SQL **formatter** invariant matrix.
//!
//! Modeled on `tests/ddl.rs`: a large `CASES` array crosses (a) shared SQL that must format
//! identically under Databricks — SELECT/JOIN/CTE/window/QUALIFY/grouping-set rollups/set-ops/
//! CASE/CAST/MERGE/INSERT/UPDATE/DELETE/PIVOT/TABLESAMPLE — with (b) Databricks-specific syntax:
//! backtick identifiers (spaces, doubled-backtick escape, in select/alias/table/qualified-name
//! position), Unity Catalog 3-level names, `USING DELTA`/`USING parquet` with
//! LOCATION/TBLPROPERTIES/PARTITIONED BY/COMMENT/CLUSTER BY, `LATERAL VIEW [OUTER]` with
//! explode/posexplode, `VERSION/TIMESTAMP AS OF` time travel on refs and in joins, higher-order
//! lambdas (transform/filter/aggregate/reduce/zip_with, single- and multi-param, nested), `||`
//! concat, and `::` cast.
//!
//! Every case is asserted under the **Databricks** dialect to:
//!   1. parse with no errors,
//!   2. round-trip byte-exact losslessly (`parse(s).syntax().to_string() == s`),
//!   3. format idempotently,
//!   4. re-parse clean after formatting, and
//!   5. preserve its case-folded significant-token stream.
//!
//! A block of exact-string goldens pins the layout opinions.

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

/// Every case below is verified to parse **clean** (no diagnostics) under Databricks, so all four
/// hard invariants apply. The Delta maintenance + cache statements (`VACUUM`, `OPTIMIZE … ZORDER
/// BY`, `INSERT OVERWRITE`, `CACHE`/`UNCACHE`/`REFRESH`, `DESCRIBE HISTORY`, and the `MERGE`
/// extensions) have their own dedicated matrix in `tests/databricks_delta.rs`; a few representative
/// cases also appear here so the shared invariants cover them.
const CASES: &[&str] = &[
    // ---- shared: basic SELECT / projection / predicates ----
    "select a, b from t",
    "select a, b from t where a > 1",
    "select distinct a from t",
    "select a as x, b as y from t",
    "select t.* from t",
    "select * from t where a > 1 and b < 2 or c = 3",
    "select count(*) from t",
    "select count(distinct a) from t",
    "select a from t order by a desc nulls last",
    "select a from t limit 10 offset 5",
    // ---- shared: JOINs ----
    "select a from t join u on t.id = u.id",
    "select a from t left join u on t.id = u.id",
    "select a from t left outer join u on t.id = u.id",
    "select a from t right join u on t.id = u.id",
    "select a from t full outer join u on t.id = u.id",
    "select a from t inner join u on t.id = u.id where t.x > 0",
    "select a from t cross join u",
    "select a from t, u where t.id = u.id",
    // ---- shared: CTEs ----
    "with c as (select 1 as n) select n from c",
    "with a as (select 1), b as (select 2) select * from a, b",
    "with recursive r as (select 1 as n union all select n + 1 from r) select n from r",
    // ---- shared: window functions + QUALIFY ----
    "select sum(x) over (partition by a order by b) from t",
    "select row_number() over (order by b) from t",
    "select a from t qualify row_number() over (order by b) = 1",
    "select a from t qualify rank() over (partition by a order by b) = 1",
    "select avg(x) over (partition by a rows between unbounded preceding and current row) from t",
    // ---- shared: GROUP BY rollups ----
    "select a, count(*) from t group by a",
    "select a, count(*) from t group by 1",
    "select a, count(*) from t group by cube (a, b)",
    "select a, count(*) from t group by rollup (a, b)",
    "select a, count(*) from t group by grouping sets ((a), (b))",
    "select a, b, grouping(a) from t group by cube (a, b)",
    "select a from t group by a having count(*) > 1",
    // ---- shared: set operations ----
    "select a from t union select a from u",
    "select a from t union all select a from u",
    "select a from t intersect select a from u",
    "select a from t except select a from u",
    // ---- shared: CASE / CAST ----
    "select case when a > 0 then 1 else 0 end from t",
    "select case a when 1 then 'one' when 2 then 'two' else 'other' end from t",
    "select cast(a as int) from t",
    "select cast(a as decimal(10, 2)) from t",
    // ---- shared: subqueries ----
    "select a from t where a in (select b from u)",
    "select a from t where exists (select 1 from u where u.id = t.id)",
    "select a from (select a from u) sub",
    // ---- shared: DML ----
    "insert into t (a, b) values (1, 2)",
    "insert into t values (1, 2), (3, 4)",
    "update t set a = 1 where b = 2",
    "delete from t where a = 1",
    "merge into t using s on t.id = s.id when matched then update set t.x = s.x",
    "merge into t using s on t.id = s.id when matched then delete when not matched then insert (id) values (s.id)",
    // ---- shared: PIVOT / TABLESAMPLE ----
    "select * from t pivot (sum(x) for k in ('a', 'b'))",
    "select * from t tablesample (10 percent)",
    // ---- databricks: backtick identifiers ----
    "select `a` from t",
    "select `a b` from t",
    "select `it``s` from t",
    "select c as `my col` from t",
    "select `c1`, `c2` from `t`",
    "select `weird``col` as `out``put` from `db`.`tbl`",
    // ---- databricks: Unity Catalog 3-level names ----
    "select * from cat.sch.tbl",
    "select * from `catalog`.`schema`.`table`",
    "select cat.sch.tbl.col from cat.sch.tbl",
    "select a from main.default.events e join main.default.users u on e.uid = u.id",
    // ---- databricks: CREATE TABLE ... USING with options ----
    "create table t (id bigint) using delta",
    "create table t (id bigint, payload string) using delta",
    "create table t (id bigint) using parquet",
    "create table t (id bigint) using parquet location '/mnt/x'",
    "create table t (id bigint) using delta partitioned by (id)",
    "create table t (id bigint) using delta cluster by (id)",
    "create table t (id bigint) using delta tblproperties ('k' = 'v')",
    "create table t (id bigint) using delta location '/mnt/events' tblproperties ('delta.enableChangeDataFeed' = 'true')",
    "create or replace table t (id int) using delta",
    "create table if not exists t (id int) using delta",
    // ---- databricks: LATERAL VIEW ----
    "select * from events lateral view explode(items) t as item",
    "select * from events lateral view outer explode(items) t as item",
    "select * from events lateral view posexplode(items) t as pos, item",
    "select x.item from events lateral view explode(items) x as item where x.item > 0",
    // ---- databricks: time travel ----
    "select * from t version as of 5",
    "select * from t timestamp as of '2024-01-01'",
    "select * from t version as of 5 where a > 1",
    "select * from a join b version as of 3 on a.id = b.id",
    "select * from t1 join t2 timestamp as of '2024-01-01' on t1.id = t2.id",
    // ---- databricks: higher-order function lambdas ----
    "select transform(xs, x -> x + 1) from t",
    "select filter(xs, x -> x > 0) from t",
    "select aggregate(xs, 0, (acc, x) -> acc + x) from t",
    "select reduce(xs, 0, (acc, x) -> acc + x, acc -> acc) from t",
    "select zip_with(a, b, (x, y) -> x + y) from t",
    "select transform(xs, x -> transform(x, y -> y + 1)) from t",
    "select transform(filter(xs, x -> x > 0), y -> y * 2) from t",
    "select reduce(xs, named_struct('sum', 0), (acc, x) -> named_struct('sum', acc.sum + x)) from t",
    // ---- databricks: operators ----
    "select a || b from t",
    "select a || ' ' || b from t",
    "select a::int from t",
    "select (a + b)::double from t",
    // ---- databricks: Delta maintenance + cache statements (full matrix in databricks_delta.rs) ----
    "vacuum t retain 168 hours dry run",
    "optimize t where a > 1 zorder by (a, b)",
    "insert overwrite table t partition (dt = '2024-01-01') select a, b from s",
    "cache table t as select * from s",
    "uncache table if exists t",
    "refresh table t",
    "describe history t",
    "merge into t using s on t.id = s.id when not matched by source then delete",
    "merge into t using s on t.id = s.id when not matched then insert *",
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
fn golden_backtick_with_space_and_qualified_name() {
    assert_eq!(
        fmt("select `a b` from `catalog`.`schema`.`table`"),
        "SELECT `a b`\nFROM `catalog`.`schema`.`table`;\n"
    );
}

#[test]
fn golden_doubled_backtick_escape_is_preserved_verbatim() {
    assert_eq!(
        fmt("select `it``s` as `out``put` from t"),
        "SELECT `it``s` AS `out``put`\nFROM t;\n"
    );
}

#[test]
fn golden_unity_catalog_three_level_name() {
    assert_eq!(
        fmt("select * from main.default.events"),
        "SELECT *\nFROM main.default.events;\n"
    );
}

#[test]
fn golden_using_delta_with_all_options() {
    assert_eq!(
        fmt("create table events (id bigint, payload string) using delta location '/mnt/events' tblproperties ('delta.enableChangeDataFeed' = 'true')"),
        "CREATE TABLE events (id bigint, payload string)\n    USING delta\n    LOCATION '/mnt/events'\n    TBLPROPERTIES ('delta.enableChangeDataFeed' = 'true');\n"
    );
}

#[test]
fn golden_using_delta_partitioned_by() {
    assert_eq!(
        fmt("create table t (id bigint, dt string) using delta partitioned by (dt)"),
        "CREATE TABLE t (id bigint, dt string)\n    USING delta\n    PARTITIONED BY (dt);\n"
    );
}

#[test]
fn golden_lateral_view_on_its_own_line() {
    assert_eq!(
        fmt("select * from events lateral view explode(items) item as item_id"),
        "SELECT *\nFROM events\nLATERAL VIEW explode(items) item AS item_id;\n"
    );
}

#[test]
fn golden_lateral_view_outer_posexplode() {
    assert_eq!(
        fmt("select * from events lateral view outer posexplode(items) t as pos, item"),
        "SELECT *\nFROM events\nLATERAL VIEW OUTER posexplode(items) t AS pos, item;\n"
    );
}

#[test]
fn golden_version_as_of_time_travel() {
    assert_eq!(
        fmt("select * from events version as of 12"),
        "SELECT *\nFROM events VERSION AS OF 12;\n"
    );
}

#[test]
fn golden_higher_order_multi_param_lambda() {
    assert_eq!(
        fmt("select zip_with(a, b, (x, y) -> x + y) from events"),
        "SELECT zip_with(a, b, (x, y) -> x + y)\nFROM events;\n"
    );
}

#[test]
fn golden_qualify_clause_formats_like_snowflake() {
    // QUALIFY is a shared window-filter clause: it must be recognized (and up-cased) under
    // Databricks exactly as under Snowflake, not treated as a bare identifier.
    assert_eq!(
        fmt("select a from t qualify row_number() over (order by b) = 1"),
        "SELECT a\nFROM t\nQUALIFY row_number() OVER (ORDER BY b) = 1;\n"
    );
}
