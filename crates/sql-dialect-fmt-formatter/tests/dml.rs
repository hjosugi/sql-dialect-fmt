//! Exhaustive DML coverage (Phase 6): INSERT (single-table VALUES / SELECT, OVERWRITE, multi-table
//! ALL/FIRST WHEN), UPDATE (... SET ... FROM ... WHERE), DELETE (... USING ... WHERE), and MERGE
//! (INTO ... USING ... ON ... WHEN [NOT] MATCHED [AND ...] THEN UPDATE/DELETE/INSERT).
//!
//! The matrix crosses *statement* with *shape* (column lists, multi-row VALUES, sub-queries / CTEs
//! as a source, FROM/USING joins, multiple WHEN branches, AND-guarded MERGE branches, OVERWRITE).
//! Every case is asserted to (1) parse with no errors, (2) format to valid SQL (reparses clean),
//! (3) be idempotent, and (4) preserve its meaningful tokens (formatting only changes trivia and
//! keyword casing). A handful of exact-string goldens pin the layout opinions.

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::tokenize;
use sql_dialect_fmt_parser::parse;
use sql_dialect_fmt_syntax::SyntaxKind;

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
    // ---- INSERT ... VALUES (single-table) ----
    "insert into t values (1)",
    "insert into t values (1, 2)",
    "insert into t (a, b) values (1, 2)",
    "insert into t (a, b) values (1, 2), (3, 4)",
    "insert into mydb.sch.t (a) values (default)",
    "insert into t values (1, 'x', true, null)",
    "insert into t (a, b) values (a + 1, b * 2)",
    "insert into t (id) values (1), (2), (3), (4)",
    // ---- INSERT ... SELECT (single-table) ----
    "insert into t select * from u",
    "insert into t select a, b from u",
    "insert into t (a, b) select a, b from u where a > 0",
    "insert into t select a from u union all select a from v",
    "insert into t with c as (select 1 as n) select n from c",
    "insert into t select a from u join v on u.id = v.id",
    // ---- INSERT OVERWRITE ----
    "insert overwrite into t (a) values (1)",
    "insert overwrite into t select * from u",
    "insert overwrite into mydb.sch.t (a, b) values (1, 2)",
    // ---- multi-table INSERT ALL (unconditional) ----
    "insert all into a into b (x) select c1, c2 from src",
    "insert all into t1 values (1) into t2 values (2) select 1, 2",
    "insert all into t1 (a) values (c1) into t2 (b) values (c2) select c1, c2 from src",
    "insert overwrite all into t1 into t2 select a, b from src",
    // ---- multi-table INSERT ALL/FIRST (conditional) ----
    "insert all when c1 > 0 then into t1 when c1 < 0 then into t2 select c1 from src",
    "insert first when sev >= 9 then into high else into low select sev from events",
    "insert first when a > 1 then into t1 when a > 2 then into t2 else into t3 select a from s",
    "insert all when c1 > 0 then into t1 values (c1) else into t2 values (c2) select c1, c2 from src",
    // ---- UPDATE ----
    "update t set a = 1",
    "update t set a = 1, b = a + 2",
    "update t set a = 1 where id = 5",
    "update t set a = 1, b = a + 2 where id = 5",
    "update t set a = s.x from s where t.id = s.id",
    "update t set a = 1, b = a + 2 from s where id = 5",
    "update mydb.sch.t set a = null where a is not null",
    "update t set a = (select max(x) from u) where id = 1",
    "update t set a = s.x, b = s.y from s where t.id = s.id and t.flag = true",
    // ---- DELETE ----
    "delete from t",
    "delete from t where x > 0",
    "delete from mydb.sch.t where a is null",
    "delete from t using u where t.id = u.id",
    "delete from t using u, v where t.id = u.id and t.k = v.k",
    "delete from t using (select id from u where x > 0) u where t.id = u.id",
    // ---- MERGE ----
    "merge into t using s on t.id = s.id when matched then delete",
    "merge into tgt t using src s on t.id = s.id when matched then update set t.v = s.v",
    "merge into t using s on t.id = s.id when not matched then insert (id) values (s.id)",
    "merge into t using s on t.id = s.id when not matched then insert (id, v) values (s.id, s.v)",
    "merge into tgt t using src s on t.id = s.id when matched then update set t.v = s.v when not matched then insert (id) values (s.id)",
    "merge into t using s on t.id = s.id when matched and s.del = true then delete when matched then update set t.v = s.v when not matched then insert (id) values (s.id)",
    "merge into t using (select * from s) s on t.id = s.id when matched then delete",
    "merge into t using (with c as (select 1 id) select * from c) s on t.id = s.id when matched then delete",
    "merge into t using s on t.id = s.id and t.k = s.k when matched then update set t.v = s.v",
    "merge into target t using source s on t.id = s.id when matched then update set t.v = s.v when not matched then insert (id, v) values (s.id, s.v)",
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
fn insert_values_go_below_the_header() {
    assert_eq!(
        fmt("insert into t (a, b) values (1, 2), (3, 4)"),
        "INSERT INTO t (a, b)\nVALUES (1, 2), (3, 4);\n"
    );
}

#[test]
fn insert_select_puts_the_query_below() {
    assert_eq!(
        fmt("insert into t select a, b from u"),
        "INSERT INTO t\nSELECT a, b\nFROM u;\n"
    );
}

#[test]
fn insert_overwrite_keeps_the_keyword_and_breaks_below() {
    assert_eq!(
        fmt("insert overwrite into t (a) values (1)"),
        "INSERT OVERWRITE INTO t (a)\nVALUES (1);\n"
    );
}

#[test]
fn multi_table_insert_all_stacks_each_target() {
    assert_eq!(
        fmt("insert all into a into b (x) select c1, c2 from src"),
        "INSERT ALL\nINTO a\nINTO b (x)\nSELECT c1, c2\nFROM src;\n"
    );
}

#[test]
fn conditional_insert_first_puts_each_branch_on_its_own_line() {
    assert_eq!(
        fmt("insert first when sev >= 9 then into high else into low select sev from events"),
        "INSERT FIRST\nWHEN sev >= 9 THEN INTO high\nELSE\nINTO low\nSELECT sev\nFROM events;\n"
    );
}

#[test]
fn update_set_from_and_where_each_on_their_own_line() {
    assert_eq!(
        fmt("update t set a = 1, b = a + 2 from s where id = 5"),
        "UPDATE t\nSET a = 1, b = a + 2\nFROM s\nWHERE id = 5;\n"
    );
}

#[test]
fn delete_using_rides_the_header_and_where_goes_below() {
    assert_eq!(
        fmt("delete from t using u, v where t.id = u.id and t.k = v.k"),
        "DELETE FROM t USING u, v\nWHERE t.id = u.id AND t.k = v.k;\n"
    );
}

#[test]
fn merge_clauses_each_go_on_their_own_line() {
    assert_eq!(
        fmt("merge into target t using source s on t.id = s.id when matched then update set t.v = s.v when not matched then insert (id, v) values (s.id, s.v)"),
        "MERGE INTO target t\nUSING source s\nON t.id = s.id\n\
         WHEN MATCHED THEN UPDATE SET t.v = s.v\n\
         WHEN NOT MATCHED THEN INSERT (id, v) VALUES (s.id, s.v);\n"
    );
}

#[test]
fn merge_with_and_guarded_branches_keeps_the_condition_inline() {
    assert_eq!(
        fmt("merge into t using s on t.id = s.id when matched and s.del = true then delete when matched then update set t.v = s.v when not matched then insert (id) values (s.id)"),
        "MERGE INTO t\nUSING s\nON t.id = s.id\n\
         WHEN MATCHED AND s.del = TRUE THEN DELETE\n\
         WHEN MATCHED THEN UPDATE SET t.v = s.v\n\
         WHEN NOT MATCHED THEN INSERT (id) VALUES (s.id);\n"
    );
}

#[test]
fn merge_subquery_source_expands_inline_with_a_trailing_alias() {
    // A parenthesized SELECT source explodes inside the `USING (...)`, the alias staying after `)`.
    assert_eq!(
        fmt("merge into t using (select * from s) s on t.id = s.id when matched then delete"),
        "MERGE INTO t\nUSING (\n    SELECT *\n    FROM s\n) s\nON t.id = s.id\nWHEN MATCHED THEN DELETE;\n"
    );
}
