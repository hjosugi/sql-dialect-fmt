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
    let expected = "\
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
";
    assert_eq!(out, expected);
}

#[test]
fn multiple_statements_are_separated_and_terminated() {
    assert_eq!(fmt("select 1; select 2"), "SELECT 1;\n\nSELECT 2;\n");
}

#[test]
fn magic_trailing_comma_forces_the_list_to_explode() {
    // The list would fit on one line, but the author's trailing comma means "keep it exploded".
    let expected = "\
SELECT
    a,
    b,
FROM t;
";
    assert_eq!(fmt("select a, b, from t"), expected);
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
    let expected = "\
SELECT
    f(
        a,
        b,
    )
FROM t;
";
    assert_eq!(fmt("select f(a, b,) from t"), expected);
}

#[test]
fn function_arguments_stay_inline_without_a_trailing_comma() {
    assert_eq!(fmt("select f(a, b) from t"), "SELECT f(a, b)\nFROM t;\n");
}

#[test]
fn values_rows_honor_a_magic_trailing_comma() {
    let expected = "\
VALUES (
    1,
    2,
), (3, 4);
";
    assert_eq!(fmt("values (1, 2,), (3, 4)"), expected);
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
    let expected = "\
SELECT a.x, b.y
FROM a
INNER JOIN b ON a.id = b.id
LEFT JOIN c ON b.k = c.k;
";
    assert_eq!(
        fmt("select a.x, b.y from a inner join b on a.id = b.id left join c on b.k = c.k"),
        expected
    );
}

#[test]
fn in_list_honors_a_magic_trailing_comma() {
    let expected = "\
SELECT *
FROM t
WHERE x IN (
    1,
    2,
    3,
);
";
    assert_eq!(fmt("select * from t where x in (1, 2, 3,)"), expected);
}

#[test]
fn in_list_stays_inline_without_a_trailing_comma() {
    assert_eq!(
        fmt("select * from t where x in (1, 2, 3)"),
        "SELECT *\nFROM t\nWHERE x IN (1, 2, 3);\n"
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
    let expected = "\
SELECT *
FROM t
ORDER BY
    alpha,
    bravo DESC,
    charlie NULLS LAST;
";
    assert_eq!(out, expected);
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
    let expected = "\
SELECT
    CASE
        WHEN a > 10 THEN 'big'
        WHEN a > 0 THEN 'small'
        ELSE 'zero'
    END AS label
FROM t;
";
    assert_eq!(out, expected);
}

#[test]
fn simple_case_keeps_its_operand() {
    assert_eq!(
        fmt("select case status when 1 then 'a' when 2 then 'b' end from t"),
        "SELECT CASE status WHEN 1 THEN 'a' WHEN 2 THEN 'b' END\nFROM t;\n"
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
    let expected = "\
SELECT
    a, -- first
    b
FROM t;
";
    assert_eq!(fmt("select a, -- first\n b from t"), expected);
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
