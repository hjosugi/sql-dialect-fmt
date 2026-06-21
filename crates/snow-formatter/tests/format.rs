//! Golden output, option, and safety-fallback tests for the SQL formatter.
//!
//! The exact-string goldens pin down the formatter's opinions (clause stacking, list wrapping,
//! keyword casing). The property tests in `corpus.rs` cover idempotency and token preservation
//! across a wider input set.

use snow_formatter::{format, format_with, FormatOptions, KeywordCase};

/// Assert that `input` formats to exactly `expected`.
#[track_caller]
fn assert_format(input: &str, expected: &str) {
    assert_eq!(format(input), expected, "\n--- input ---\n{input}\n");
}

// ---- clause stacking & list layout ----

#[test]
fn single_select_stays_on_one_line() {
    assert_format("select 1", "SELECT 1;\n");
}

#[test]
fn clauses_each_start_a_new_line() {
    assert_format(
        "SELECT a, b AS x, c alias FROM db.sch.t",
        "SELECT a, b AS x, c alias\nFROM db.sch.t;\n",
    );
}

#[test]
fn distinct_and_boolean_where() {
    assert_format(
        "select distinct a, b from t where a > 1 and b <= 2 or not c",
        "SELECT DISTINCT a, b\nFROM t\nWHERE a > 1 AND b <= 2 OR NOT c;\n",
    );
}

#[test]
fn aggregate_query_with_many_clauses() {
    assert_format(
        "select count(*), sum(x) from t group by a, b having count(*) > 1 order by a desc nulls last limit 10",
        "SELECT count(*), sum(x)\n\
         FROM t\n\
         GROUP BY a, b\n\
         HAVING count(*) > 1\n\
         ORDER BY a DESC NULLS LAST\n\
         LIMIT 10;\n",
    );
}

#[test]
fn long_select_list_explodes_one_per_line() {
    assert_format(
        "select averyverylongcolumnnamehere, anotherlongcolumnnamehere, yetanotherlongcolumn, andonemore from sometable",
        "SELECT\n  \
           averyverylongcolumnnamehere,\n  \
           anotherlongcolumnnamehere,\n  \
           yetanotherlongcolumn,\n  \
           andonemore\n\
         FROM sometable;\n",
    );
}

#[test]
fn joins_each_on_their_own_line() {
    assert_format(
        "select a from t1 join t2 on t1.id = t2.id left outer join t3 on t2.x = t3.x",
        "SELECT a\nFROM t1\nJOIN t2 ON t1.id = t2.id\nLEFT OUTER JOIN t3 ON t2.x = t3.x;\n",
    );
}

#[test]
fn cte_and_union() {
    assert_format(
        "with c as (select 1 as n) select n from c",
        "WITH c AS (SELECT 1 AS n)\nSELECT n\nFROM c;\n",
    );
    assert_format(
        "select 1 union all select 2",
        "SELECT 1\nUNION ALL\nSELECT 2;\n",
    );
}

#[test]
fn derived_table_subquery_breaks() {
    assert_format(
        "select * from (select a from t) sub",
        "SELECT *\nFROM (\n  SELECT a\n  FROM t\n) sub;\n",
    );
}

#[test]
fn case_window_and_semistructured() {
    assert_format(
        "select case when a then 1 when b then 2 else 3 end from t",
        "SELECT CASE WHEN a THEN 1 WHEN b THEN 2 ELSE 3 END\nFROM t;\n",
    );
    assert_format(
        "select row_number() over (partition by a order by b desc) as rn from t",
        "SELECT row_number() OVER (PARTITION BY a ORDER BY b DESC) AS rn\nFROM t;\n",
    );
    assert_format(
        "select a::int, cast(b as varchar(10)), payload:items[0].name::string from raw",
        "SELECT a::int, CAST(b AS varchar(10)), payload:items[0].name::string\nFROM raw;\n",
    );
}

#[test]
fn predicates_in_between_is() {
    assert_format(
        "select x from t where id in (1,2,3) and y between 1 and 10 and z is not null",
        "SELECT x\nFROM t\nWHERE id IN (1, 2, 3) AND y BETWEEN 1 AND 10 AND z IS NOT NULL;\n",
    );
}

#[test]
fn values_statement() {
    assert_format("values (1, 'a'), (2, 'b')", "VALUES (1, 'a'), (2, 'b');\n");
}

#[test]
fn multiple_statements_each_terminated() {
    assert_format("select 1; select 2", "SELECT 1;\nSELECT 2;\n");
}

// ---- options ----

#[test]
fn keyword_case_lower() {
    let opts = FormatOptions {
        keyword_case: KeywordCase::Lower,
        ..FormatOptions::default()
    };
    assert_eq!(
        format_with("SELECT a FROM t WHERE a AND b", &opts),
        "select a\nfrom t\nwhere a and b;\n",
    );
}

#[test]
fn line_width_controls_wrapping() {
    let narrow = FormatOptions {
        line_width: 12,
        ..FormatOptions::default()
    };
    assert_eq!(
        format_with("SELECT aaa, bbb, ccc FROM t", &narrow),
        "SELECT\n  aaa,\n  bbb,\n  ccc\nFROM t;\n",
    );
    let wide = FormatOptions {
        line_width: 80,
        ..FormatOptions::default()
    };
    assert_eq!(
        format_with("SELECT aaa, bbb, ccc FROM t", &wide),
        "SELECT aaa, bbb, ccc\nFROM t;\n",
    );
}

#[test]
fn indent_width_is_configurable() {
    let four = FormatOptions {
        line_width: 12,
        indent_width: 4,
        ..FormatOptions::default()
    };
    assert_eq!(
        format_with("SELECT aaa, bbb, ccc FROM t", &four),
        "SELECT\n    aaa,\n    bbb,\n    ccc\nFROM t;\n",
    );
}

// ---- comments ----

#[test]
fn trailing_line_comment_after_statement() {
    assert_format("select 1 -- trailing\n", "SELECT 1; -- trailing\n");
    assert_format("select a from t -- end\n", "SELECT a\nFROM t; -- end\n");
}

#[test]
fn leading_comment_above_statement() {
    assert_format(
        "/* lead */ select a from t",
        "/* lead */\nSELECT a\nFROM t;\n",
    );
    assert_format("-- file head\nselect 1", "-- file head\nSELECT 1;\n");
}

#[test]
fn comment_above_first_select_item() {
    assert_format(
        "select\n  -- the id\n  id,\n  name\nfrom t",
        "SELECT\n  -- the id\n  id,\n  name\nFROM t;\n",
    );
}

#[test]
fn trailing_comment_on_a_select_item() {
    assert_format(
        "select a, -- comment on a\n b from t",
        "SELECT\n  a, -- comment on a\n  b\nFROM t;\n",
    );
}

#[test]
fn trailing_comment_after_select_list_stays_inline() {
    assert_format(
        "select a -- after a\nfrom t",
        "SELECT a -- after a\nFROM t;\n",
    );
    assert_format(
        "select a, b -- after list\nfrom t",
        "SELECT a, b -- after list\nFROM t;\n",
    );
}

#[test]
fn comment_between_clauses_leads_the_next_clause() {
    assert_format(
        "select a\n-- before from\nfrom t",
        "SELECT a\n-- before from\nFROM t;\n",
    );
}

#[test]
fn trailing_comment_inside_a_where_predicate() {
    assert_format(
        "select a from t where x = 1 -- pred\n and y = 2",
        "SELECT a\nFROM t\nWHERE x = 1 -- pred\n  AND y = 2;\n",
    );
}

#[test]
fn own_line_comment_after_a_statement() {
    assert_format(
        "select 1;\n-- trailing file comment\n",
        "SELECT 1;\n-- trailing file comment\n",
    );
}

// ---- conservative safety fallbacks ----

#[test]
fn broken_input_is_returned_unchanged() {
    for input in ["select from", "select )( @ #", "1 +"] {
        assert_eq!(format(input), input, "broken input must be untouched");
    }
}

#[test]
fn empty_and_whitespace_inputs() {
    assert_eq!(format(""), "");
    // Whitespace-only input has no statements; it collapses to empty.
    assert_eq!(format("   \n  "), "");
}
