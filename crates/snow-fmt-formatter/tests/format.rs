//! Behavioural tests for the SQL formatter.
//!
//! Beyond a handful of golden expectations, the important invariants are exercised over the whole
//! embedded corpus:
//! * **Idempotency** — `format(format(x)) == format(x)`. A formatter that isn't a fixed point on
//!   its own output is a bug factory.
//! * **Content preservation** — the sequence of significant tokens (everything but trivia and the
//!   synthesized statement terminators) is unchanged, so formatting never drops or invents SQL.
//! * **No new parse errors** — formatting clean input yields clean output.

use snow_fmt_formatter::{format, FormatOptions};
use snow_fmt_lexer::tokenize;
use snow_fmt_parser::parse;
use snow_fmt_syntax::SyntaxKind;
use snow_fmt_test_fixtures::EASY_CASES;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

/// Significant token kinds: drop trivia and the statement terminators the formatter synthesizes.
fn significant_kinds(src: &str) -> Vec<SyntaxKind> {
    tokenize(src)
        .tokens
        .iter()
        .map(|t| t.kind)
        .filter(|k| !k.is_trivia() && *k != SyntaxKind::SEMICOLON)
        .collect()
}

/// The (whitespace-trimmed) text of every comment token in `src`.
fn comment_texts(src: &str) -> Vec<String> {
    tokenize(src)
        .tokens
        .iter()
        .filter(|t| t.kind.is_comment())
        .map(|t| t.text.trim_end().to_string())
        .collect()
}

#[test]
fn formats_a_basic_select() {
    assert_eq!(fmt("select a,b from t"), "SELECT a, b\nFROM t;\n");
}

#[test]
fn upcases_keywords_and_normalizes_spacing() {
    assert_eq!(
        fmt("select   x  from   t  where x=1 and y<>2"),
        "SELECT x\nFROM t\nWHERE x = 1 AND y <> 2;\n"
    );
}

#[test]
fn keeps_qualified_names_and_calls_tight() {
    assert_eq!(
        fmt("select count(*), t.a, x::int from s.t"),
        "SELECT count(*), t.a, x::int\nFROM s.t;\n"
    );
}

#[test]
fn distinct_is_part_of_the_header() {
    assert_eq!(
        fmt("select distinct a from t"),
        "SELECT DISTINCT a\nFROM t;\n"
    );
}

#[test]
fn long_select_list_breaks_one_item_per_line() {
    let src = "select alpha, bravo, charlie, delta, echo, foxtrot, golf, hotel from t";
    let out = format(
        src,
        &FormatOptions {
            line_width: 40,
            ..FormatOptions::default()
        },
    );
    insta::assert_snapshot!(out, @"
    SELECT
        alpha,
        bravo,
        charlie,
        delta,
        echo,
        foxtrot,
        golf,
        hotel
    FROM t;
    ");
}

#[test]
fn multiple_statements_are_separated_and_terminated() {
    assert_eq!(fmt("select 1; select 2"), "SELECT 1;\n\nSELECT 2;\n");
}

#[test]
fn magic_trailing_comma_forces_the_list_to_explode() {
    // The list would fit on one line, but the author's trailing comma means "keep it exploded".
    insta::assert_snapshot!(fmt("select a, b, from t"), @"
    SELECT
        a,
        b,
    FROM t;
    ");
}

#[test]
fn magic_trailing_comma_explodes_even_a_single_item() {
    assert_eq!(fmt("select a, from t"), "SELECT\n    a,\nFROM t;\n");
}

#[test]
fn no_trailing_comma_stays_inline_when_it_fits() {
    assert_eq!(fmt("select a, b from t"), "SELECT a, b\nFROM t;\n");
}

#[test]
fn function_arguments_honor_a_magic_trailing_comma() {
    // The trailing comma after `b` explodes the argument list, which in turn forces the SELECT
    // list to break (a multiline item can't sit inline).
    insta::assert_snapshot!(fmt("select f(a, b,) from t"), @"
    SELECT
        f(
            a,
            b,
        )
    FROM t;
    ");
}

#[test]
fn function_arguments_stay_inline_without_a_trailing_comma() {
    assert_eq!(fmt("select f(a, b) from t"), "SELECT f(a, b)\nFROM t;\n");
}

#[test]
fn values_rows_honor_a_magic_trailing_comma() {
    insta::assert_snapshot!(fmt("values (1, 2,), (3, 4)"), @"
    VALUES
        (
            1,
            2,
        ),
        (3, 4);
    ");
}

#[test]
fn aggregate_distinct_quantifier_is_kept() {
    assert_eq!(
        fmt("select count(distinct x) from t"),
        "SELECT count(DISTINCT x)\nFROM t;\n"
    );
    assert_eq!(
        fmt("select listagg(distinct x, ',') from t"),
        "SELECT listagg(DISTINCT x, ',')\nFROM t;\n"
    );
}

#[test]
fn empty_argument_list_stays_tight() {
    assert_eq!(
        fmt("select current_timestamp() from t"),
        "SELECT current_timestamp()\nFROM t;\n"
    );
}

#[test]
fn joins_each_go_on_their_own_line() {
    insta::assert_snapshot!(
        fmt("select a.x, b.y from a inner join b on a.id = b.id left join c on b.k = c.k"),
        @"
    SELECT a.x, b.y
    FROM a
    INNER JOIN b ON a.id = b.id
    LEFT JOIN c ON b.k = c.k;
    ",
    );
}

#[test]
fn in_list_honors_a_magic_trailing_comma() {
    insta::assert_snapshot!(fmt("select * from t where x in (1, 2, 3,)"), @"
    SELECT *
    FROM t
    WHERE x IN (
        1,
        2,
        3,
    );
    ");
}

#[test]
fn in_list_stays_inline_without_a_trailing_comma() {
    assert_eq!(
        fmt("select * from t where x in (1, 2, 3)"),
        "SELECT *\nFROM t\nWHERE x IN (1, 2, 3);\n"
    );
}

#[test]
fn hierarchical_query_puts_start_with_and_connect_by_on_their_own_lines() {
    insta::assert_snapshot!(
        fmt("select id, name from emp start with manager_id is null \
             connect by prior id = manager_id"),
        @"
    SELECT id, name
    FROM emp
    START WITH manager_id IS NULL
    CONNECT BY PRIOR id = manager_id;
    "
    );
}

#[test]
fn in_subquery_stays_inline() {
    assert_eq!(
        fmt("select * from t where x in (select id from s)"),
        "SELECT *\nFROM t\nWHERE x IN (SELECT id FROM s);\n"
    );
}

#[test]
fn order_by_items_wrap_when_they_do_not_fit() {
    let out = format(
        "select * from t order by alpha, bravo desc, charlie nulls last",
        &FormatOptions {
            line_width: 30,
            ..FormatOptions::default()
        },
    );
    insta::assert_snapshot!(out, @"
    SELECT *
    FROM t
    ORDER BY
        alpha,
        bravo DESC,
        charlie NULLS LAST;
    ");
}

#[test]
fn short_case_stays_on_one_line() {
    assert_eq!(
        fmt("select case when a then 1 else 2 end from t"),
        "SELECT CASE WHEN a THEN 1 ELSE 2 END\nFROM t;\n"
    );
}

#[test]
fn long_case_breaks_one_arm_per_line() {
    let out = format(
        "select case when a > 10 then 'big' when a > 0 then 'small' else 'zero' end as label from t",
        &FormatOptions {
            line_width: 40,
            ..FormatOptions::default()
        },
    );
    insta::assert_snapshot!(out, @"
    SELECT
        CASE
            WHEN a > 10 THEN 'big'
            WHEN a > 0 THEN 'small'
            ELSE 'zero'
        END AS label
    FROM t;
    ");
}

#[test]
fn simple_case_keeps_its_operand() {
    assert_eq!(
        fmt("select case status when 1 then 'a' when 2 then 'b' end from t"),
        "SELECT CASE status WHEN 1 THEN 'a' WHEN 2 THEN 'b' END\nFROM t;\n"
    );
}

#[test]
fn cte_bodies_are_indented_and_one_per_line() {
    insta::assert_snapshot!(
        fmt("with a as (select x from t), b as (select y from u) select * from a"),
        @"
    WITH a AS (
        SELECT x
        FROM t
    ),
    b AS (
        SELECT y
        FROM u
    )
    SELECT *
    FROM a;
    ",
    );
}

#[test]
fn short_cte_stays_inline() {
    assert_eq!(
        fmt("with recursive r as (select 1) select * from r"),
        "WITH RECURSIVE r AS (SELECT 1)\nSELECT *\nFROM r;\n"
    );
}

#[test]
fn derived_table_subquery_is_indented() {
    insta::assert_snapshot!(
        fmt("select * from (select id from users where active) u"),
        @"
    SELECT *
    FROM (
        SELECT id
        FROM users
        WHERE active
    ) u;
    ",
    );
}

#[test]
fn set_operations_put_each_query_and_operator_on_its_own_line() {
    insta::assert_snapshot!(fmt("select a from t union all select a from u"), @"
    SELECT a
    FROM t
    UNION ALL
    SELECT a
    FROM u;
    ");
}

#[test]
fn chained_set_operations_flatten() {
    assert_eq!(
        fmt("select 1 union select 2 except select 3"),
        "SELECT 1\nUNION\nSELECT 2\nEXCEPT\nSELECT 3;\n"
    );
}

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
fn update_set_and_where_each_on_their_own_line() {
    assert_eq!(
        fmt("update t set a = 1, b = a + 2 where id = 5"),
        "UPDATE t\nSET a = 1, b = a + 2\nWHERE id = 5;\n"
    );
}

#[test]
fn delete_where_goes_below() {
    assert_eq!(
        fmt("delete from t where x > 0"),
        "DELETE FROM t\nWHERE x > 0;\n"
    );
}

#[test]
fn merge_clauses_each_go_on_their_own_line() {
    insta::assert_snapshot!(
        fmt("merge into target t using source s on t.id = s.id when matched then update set t.v = s.v when not matched then insert (id, v) values (s.id, s.v)"),
        @"
    MERGE INTO target t
    USING source s
    ON t.id = s.id
    WHEN MATCHED THEN UPDATE SET t.v = s.v
    WHEN NOT MATCHED THEN INSERT (id, v) VALUES (s.id, s.v);
    ",
    );
}

#[test]
fn create_view_puts_the_query_after_as() {
    assert_eq!(
        fmt("create or replace view v as select a, b from t"),
        "CREATE OR REPLACE VIEW v AS\nSELECT a, b\nFROM t;\n"
    );
}

#[test]
fn create_table_as_select_is_a_ctas() {
    assert_eq!(
        fmt("create table t as select a from u"),
        "CREATE TABLE t AS\nSELECT a\nFROM u;\n"
    );
}

#[test]
fn create_table_column_defs_wrap_one_per_line() {
    let out = format(
        "create table t (id int, name varchar(100) not null)",
        &FormatOptions {
            line_width: 30,
            ..FormatOptions::default()
        },
    );
    insta::assert_snapshot!(out, @"
    CREATE TABLE t (
        id int,
        name varchar(100) NOT NULL
    );
    ");
}

#[test]
fn drop_statement_is_inline() {
    assert_eq!(
        fmt("drop table if exists db.s.t"),
        "DROP TABLE IF EXISTS db.s.t;\n"
    );
}

#[test]
fn within_group_aggregate_is_kept() {
    assert_eq!(
        fmt("select listagg(x, ',') within group (order by x) from t"),
        "SELECT listagg(x, ',') WITHIN GROUP (ORDER BY x)\nFROM t;\n"
    );
}

#[test]
fn pivot_and_unpivot_are_kept() {
    assert_eq!(
        fmt("select * from t pivot (sum(amount) for month in ('jan', 'feb')) as p"),
        "SELECT *\nFROM t PIVOT (sum(amount) FOR month IN ('jan', 'feb')) AS p;\n"
    );
    assert_eq!(
        fmt("select * from sales unpivot (amount for quarter in (q1, q2))"),
        "SELECT *\nFROM sales UNPIVOT (amount FOR quarter IN (q1, q2));\n"
    );
}

#[test]
fn procedure_header_is_structured_and_body_is_verbatim() {
    let src = "create or replace procedure p(x int) returns int language sql as $$\nbegin\n  return x;\nend\n$$";
    let out = fmt(src);
    // Header reflowed/up-cased, the delimited body preserved verbatim.
    assert!(
        out.starts_with("CREATE OR REPLACE PROCEDURE p (x int) RETURNS int LANGUAGE SQL AS $$"),
        "header not structured: {out:?}"
    );
    assert!(
        out.contains("\nbegin\n  return x;\nend\n$$"),
        "body changed: {out:?}"
    );
    assert_eq!(fmt(&out), out, "not idempotent");
}

#[test]
fn unquoted_scripting_body_passes_through_unchanged() {
    // No delimited body → parse error → returned unchanged (never mis-split on inner `;`).
    let src = "create procedure p() returns string language sql as begin return 'x'; end";
    assert_eq!(fmt(src), src);
}

#[test]
fn session_set_and_execute_immediate_format_inline() {
    assert_eq!(
        fmt("set target_table = 'MART.X'"),
        "SET target_table = 'MART.X';\n"
    );
    assert_eq!(
        fmt("execute immediate 'insert into t values (1)' using (x)"),
        "EXECUTE IMMEDIATE 'insert into t values (1)' USING (x);\n"
    );
}

#[test]
fn grouping_sets_and_cube_are_kept() {
    assert_eq!(
        fmt("select a, count(*) from t group by grouping sets ((a, b), (c), ())"),
        "SELECT a, count(*)\nFROM t\nGROUP BY GROUPING SETS ((a, b), (c), ());\n"
    );
    assert_eq!(
        fmt("select a from t group by cube(a, b)"),
        "SELECT a\nFROM t\nGROUP BY cube(a, b);\n"
    );
}

#[test]
fn named_arguments_are_kept() {
    assert_eq!(
        fmt("select object_construct('a', 1, b => 2) from t"),
        "SELECT object_construct('a', 1, b => 2)\nFROM t;\n"
    );
}

#[test]
fn lateral_flatten_and_table_function_format() {
    assert_eq!(
        fmt("select f.value from t, lateral flatten(input => t.items) f"),
        "SELECT f.value\nFROM t, LATERAL FLATTEN(INPUT => t.items) f;\n"
    );
    assert_eq!(
        fmt("select * from table(flatten(input => parse_json(x)))"),
        "SELECT *\nFROM TABLE(FLATTEN(INPUT => parse_json(x)));\n"
    );
}

#[test]
fn is_distinct_from_is_kept() {
    assert_eq!(
        fmt("select * from t where a is distinct from b and c is not distinct from d"),
        "SELECT *\nFROM t\nWHERE a IS DISTINCT FROM b AND c IS NOT DISTINCT FROM d;\n"
    );
}

#[test]
fn semi_structured_path_keys_keep_their_case() {
    // `order` is a keyword but here it is a case-sensitive JSON key — it must not be up-cased.
    assert_eq!(
        fmt("select payload:order:status::string from t"),
        "SELECT payload:order:status::string\nFROM t;\n"
    );
}

#[test]
fn from_values_is_a_table_source() {
    assert_eq!(
        fmt("select c1 from values (1, 'a'), (2, 'b') as t(c1, c2)"),
        "SELECT c1\nFROM VALUES (1, 'a'), (2, 'b') AS t (c1, c2);\n"
    );
}

#[test]
fn tablesample_is_kept() {
    assert_eq!(
        fmt("select * from t tablesample bernoulli(25) repeatable(99)"),
        "SELECT *\nFROM t TABLESAMPLE bernoulli(25) repeatable(99);\n"
    );
}

#[test]
fn pivot_value_aliases_are_kept() {
    assert_eq!(
        fmt("select * from sales pivot (sum(amt) for m in (1 as jan, 2 as feb)) p"),
        "SELECT *\nFROM sales PIVOT (sum(amt) FOR m IN (1 AS jan, 2 AS feb)) p;\n"
    );
}

#[test]
fn copy_into_puts_from_and_options_on_their_own_lines() {
    insta::assert_snapshot!(
        fmt("copy into raw.orders from @raw.stage/orders/ file_format = (type = json) on_error = continue"),
        @"
    COPY INTO raw.orders
    FROM @raw.stage/orders/
    file_format = (type = json)
    on_error = continue;
    ",
    );
}

#[test]
fn asof_join_and_match_condition_are_kept() {
    assert_eq!(
        fmt("select * from q asof join t match_condition (q.ts >= t.ts) on q.sym = t.sym"),
        "SELECT *\nFROM q\nASOF JOIN t MATCH_CONDITION (q.ts >= t.ts) ON q.sym = t.sym;\n"
    );
}

#[test]
fn match_recognize_lays_out_one_clause_per_line() {
    insta::assert_snapshot!(
        fmt("select * from t match_recognize(partition by a order by b \
             measures match_number() as mn, first(price) as fp one row per match \
             after match skip past last row pattern(strt down+ up+) \
             define down as price < prev(price), up as price > prev(price))"),
        @"
    SELECT *
    FROM t MATCH_RECOGNIZE (
        PARTITION BY a
        ORDER BY b
        MEASURES match_number() AS mn, first(price) AS fp
        ONE ROW PER MATCH
        AFTER MATCH SKIP PAST LAST ROW
        PATTERN (strt down+ up+)
        DEFINE down AS price < prev(price), up AS price > prev(price)
    );
    "
    );
}

#[test]
fn changes_clause_attaches_to_its_table() {
    assert_eq!(
        fmt("select * from t changes(information => default) at(timestamp => 'x')"),
        "SELECT *\nFROM t CHANGES (information => default) AT (timestamp => 'x');\n"
    );
}

#[test]
fn keywords_used_as_function_names_are_callable() {
    // FIRST/LAST/LEFT are reserved words elsewhere but here name functions: keep them lower-case
    // and hugging their parens, like any other call.
    assert_eq!(
        fmt("select first(price), last(price), left(s, 2) from t"),
        "SELECT first(price), last(price), left(s, 2)\nFROM t;\n"
    );
}

#[test]
fn multi_table_insert_first_puts_each_branch_on_its_own_line() {
    insta::assert_snapshot!(
        fmt("insert first when sev >= 9 then into high else into low select sev from events"),
        @"
    INSERT FIRST
    WHEN sev >= 9 THEN INTO high
    ELSE
    INTO low
    SELECT sev
    FROM events;
    ",
    );
}

#[test]
fn time_travel_at_before_is_kept() {
    assert_eq!(
        fmt("select * from orders before (statement => 'abc') o"),
        "SELECT *\nFROM orders BEFORE (statement => 'abc') o;\n"
    );
}

#[test]
fn contextual_keywords_stay_identifiers_outside_their_clause() {
    // `at`/`before`/`asof`/`grouping`/`sets` are soft keywords: up-cased only where the grammar
    // recognizes them as a clause, and ordinary (lowercase) identifiers everywhere else.
    assert_eq!(
        fmt("select asof, at, before, grouping, sets from t"),
        "SELECT asof, at, before, grouping, sets\nFROM t;\n"
    );
}

#[test]
fn group_by_all_stays_inline() {
    assert_eq!(
        fmt("select a from t group by all"),
        "SELECT a\nFROM t\nGROUP BY ALL;\n"
    );
}

#[test]
fn empty_input_formats_to_empty() {
    assert_eq!(fmt(""), "");
    assert_eq!(fmt("   \n\t "), "");
}

#[test]
fn unary_minus_does_not_get_a_trailing_space() {
    assert_eq!(
        fmt("select -1, a - b from t"),
        "SELECT -1, a - b\nFROM t;\n"
    );
}

#[test]
fn statements_with_comments_keep_them() {
    let src = "select /* keep me */ a from t -- trailing note\n";
    let out = fmt(src);
    assert!(out.contains("/* keep me */"), "block comment lost: {out:?}");
    assert!(
        out.contains("-- trailing note"),
        "line comment lost: {out:?}"
    );
}

#[test]
fn leading_comment_sits_on_its_own_line() {
    let out = fmt("-- header\nselect a from t");
    assert!(
        out.starts_with("-- header\n"),
        "leading comment misplaced: {out:?}"
    );
    assert!(out.contains("FROM t;"), "{out:?}");
}

#[test]
fn banner_comment_does_not_explode_the_select_list() {
    // A statement-level leading comment is hoisted above the header group, so the list stays inline.
    assert_eq!(
        fmt("-- header\nselect a, b, c from t"),
        "-- header\nSELECT a, b, c\nFROM t;\n"
    );
}

#[test]
fn trailing_line_comment_attaches_to_its_column() {
    // A `--` comment after a column (even after the comma) trails that column's line, and forces
    // the list to break so the comment ends its line.
    insta::assert_snapshot!(fmt("select a, -- first\n b from t"), @"
    SELECT
        a, -- first
        b
    FROM t;
    ");
}

#[test]
fn inline_block_comment_stays_inline() {
    assert_eq!(
        fmt("select a /* note */ + b from t"),
        "SELECT a /* note */ + b\nFROM t;\n"
    );
}

#[test]
fn comment_only_input_is_not_dropped() {
    let out = fmt("-- just a note\n");
    assert!(
        out.contains("-- just a note"),
        "comment-only input lost: {out:?}"
    );
}

#[test]
fn comments_are_never_dropped_on_the_embedded_corpus() {
    for case in EASY_CASES {
        for (label, sql) in case.sqls() {
            let out = fmt(sql);
            for comment in comment_texts(sql) {
                assert!(
                    out.contains(&comment),
                    "comment {comment:?} dropped for {}/{label}\n--- out ---\n{out}",
                    case.name
                );
            }
        }
    }
}

#[test]
fn is_idempotent_on_the_embedded_corpus() {
    for case in EASY_CASES {
        for (label, sql) in case.sqls() {
            let once = fmt(sql);
            let twice = fmt(&once);
            assert_eq!(
                once, twice,
                "formatting is not idempotent for {}/{label}",
                case.name
            );
        }
    }
}

#[test]
fn preserves_significant_tokens_on_the_embedded_corpus() {
    for case in EASY_CASES {
        for (label, sql) in case.sqls() {
            let out = fmt(sql);
            let name = case.name;
            assert_eq!(
                significant_kinds(sql),
                significant_kinds(&out),
                "formatting changed the significant tokens for {name}/{label}\n--- in ---\n{sql}\n--- out ---\n{out}",
            );
        }
    }
}

#[test]
fn clean_input_stays_clean_after_formatting() {
    for case in EASY_CASES {
        for (label, sql) in case.sqls() {
            if !parse(sql).errors().is_empty() {
                continue; // only assert about inputs the parser already accepts
            }
            let out = fmt(sql);
            let name = case.name;
            assert!(
                parse(&out).errors().is_empty(),
                "formatting introduced parse errors for {name}/{label}\n--- out ---\n{out}",
            );
        }
    }
}
