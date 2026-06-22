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
fn statements_with_comments_are_preserved_verbatim() {
    let src = "select /* keep me */ a from t -- trailing note\n";
    let out = fmt(src);
    assert!(out.contains("/* keep me */"), "block comment lost: {out:?}");
    assert!(
        out.contains("-- trailing note"),
        "line comment lost: {out:?}"
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
