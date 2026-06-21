//! Property tests for the formatter over a corpus of real SQL.
//!
//! These encode the formatter's load-bearing guarantees, independent of any one expected string:
//!
//! * **Idempotency** — `format(format(x)) == format(x)` for *every* input, formatted or fallback.
//! * **Token preservation** — formatting only changes trivia and keyword casing; the sequence of
//!   meaningful tokens (modulo case and the synthesized statement terminator) is unchanged. This
//!   is what proves the formatter never drops, adds, or reorders SQL.
//! * **Validity & well-formedness** — formatting a clean, comment-free input yields clean SQL that
//!   ends in a single newline and has no trailing whitespace.

use snow_fmt_lexer::tokenize;
use snow_fmt_parser::parse;
use snow_fmt_test_fixtures::EASY_CASES;
use snow_formatter::format;

/// A curated set exercising every construct the lowering handles, as a focused complement to the
/// shared fixture corpus.
const CURATED: &[&str] = &[
    "select 1",
    "SELECT a, b AS x, c alias FROM db.sch.t",
    "select distinct a, b from t where a > 1 and b <= 2 or not c",
    "select count(*), sum(x), f(a,b) from t group by a, b having count(*) > 1 order by a desc nulls last limit 10 offset 5",
    "select averyverylongcolumnnamehere, anotherlongcolumnnamehere, yetanotherlongcolumn, andonemore from sometable",
    "select a from t1 join t2 on t1.id = t2.id left outer join t3 on t2.x = t3.x",
    "select a from x, y, z",
    "with c as (select 1 as n), d as (select 2 as m) select n, m from c, d",
    "select 1 union all select 2 except select 3",
    "select case x when 1 then 'a' when 2 then 'b' else 'c' end from t",
    "select case when a then 1 else 0 end",
    "select a::int, cast(b as varchar(10)), payload:items[0].name::string from raw",
    "select x from t where id in (1,2,3) and y between 1 and 10 and z is not null",
    "select x from t where exists (select 1 from u where u.id = t.id)",
    "select sum(x) over (partition by a, b order by c rows between unbounded preceding and current row) from t",
    "select * from (select a from t) sub",
    "values (1, 'a'), (2, 'b'), (3, 'c')",
    "select 1; select 2; select 3",
    // comments
    "select 1 -- trailing\n",
    "/* lead */ select a from t",
    "select\n  -- the id\n  id,\n  name\nfrom t",
    "select a, -- on a\n b from t",
    "select a -- after list\nfrom t",
    "select a\n-- before from\nfrom t",
    "select a from t where x = 1 -- pred\n and y = 2",
    "select 1;\n-- trailing file comment\n",
    "select a from t -- end\n",
];

/// Meaningful tokens, upper-cased and with statement terminators dropped — the canonical form a
/// faithful formatter must preserve.
fn signature(sql: &str) -> Vec<String> {
    tokenize(sql)
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != snow_fmt_lexer::SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

/// The sorted multiset of comment texts in `sql` — a faithful formatter must keep them all.
fn comments_of(sql: &str) -> Vec<String> {
    let mut v: Vec<String> = tokenize(sql)
        .tokens
        .into_iter()
        .filter(|t| t.kind.is_comment())
        .map(|t| t.text.to_string())
        .collect();
    v.sort();
    v
}

fn every_sql() -> impl Iterator<Item = String> {
    let fixtures = EASY_CASES
        .iter()
        .flat_map(|case| case.sqls().map(|(_, sql)| sql.to_string()));
    let curated = CURATED.iter().map(|s| s.to_string());
    fixtures.chain(curated)
}

#[test]
fn formatting_is_idempotent() {
    for sql in every_sql() {
        let once = format(&sql);
        let twice = format(&once);
        assert_eq!(
            once, twice,
            "not idempotent for:\n{sql}\n--- once ---\n{once}"
        );
    }
}

#[test]
fn formatting_preserves_meaningful_tokens() {
    for sql in every_sql() {
        let formatted = format(&sql);
        assert_eq!(
            signature(&sql),
            signature(&formatted),
            "token sequence changed for:\n{sql}\n--- formatted ---\n{formatted}"
        );
    }
}

#[test]
fn formatting_preserves_comments() {
    for sql in every_sql() {
        let formatted = format(&sql);
        assert_eq!(
            comments_of(&sql),
            comments_of(&formatted),
            "a comment was dropped or altered for:\n{sql}\n--- formatted ---\n{formatted}"
        );
    }
}

#[test]
fn clean_inputs_yield_clean_well_formed_sql() {
    for sql in every_sql() {
        if !parse(&sql).errors().is_empty() {
            continue; // broken input is returned verbatim; nothing to assert
        }
        let formatted = format(&sql);
        assert!(
            parse(&formatted).errors().is_empty(),
            "formatted output is not valid SQL for:\n{sql}\n--- formatted ---\n{formatted}"
        );
        // When the formatter actually reformatted (not a verbatim safety-net passthrough), the
        // output is well-formed: ends in exactly one newline, no trailing whitespace on any line.
        if formatted != sql && !formatted.is_empty() {
            assert!(
                formatted.ends_with('\n') && !formatted.ends_with("\n\n"),
                "output must end with exactly one newline:\n{formatted:?}"
            );
            for line in formatted.lines() {
                assert!(
                    !line.ends_with(' '),
                    "line has trailing whitespace: {line:?}\nin:\n{formatted}"
                );
            }
        }
    }
}
