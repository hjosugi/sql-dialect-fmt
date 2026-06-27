//! Exhaustive SAMPLE / TABLESAMPLE and ASOF JOIN coverage (Phase 4 table operators).
//!
//! The matrix crosses *operator shape* with *position*:
//!   * SAMPLE/TABLESAMPLE: every sampling method (BERNOULLI / ROW / SYSTEM / BLOCK / default),
//!     fraction vs. fixed `<n> ROWS`, SEED / REPEATABLE, tight vs. spaced parens, on a base table,
//!     on a derived table, before/with an alias, and combined with WHERE / JOIN / set ops.
//!   * ASOF JOIN: MATCH_CONDITION with each comparison operator (`>=` `<=` `>` `<`), ON (single and
//!     multi-equality), USING, no join condition, and aliased / chained / ordered queries.
//!
//! Every case is asserted to (1) parse with no errors, (2) format to valid SQL, (3) be idempotent
//! (`format(format(x)) == format(x)`), and (4) preserve its meaningful tokens (formatting only
//! changes trivia and keyword casing). A few exact-string goldens pin the canonical layout.
//!
//! References:
//!   * <https://docs.snowflake.com/en/sql-reference/constructs/sample>
//!   * <https://docs.snowflake.com/en/sql-reference/constructs/asof-join>

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::{tokenize, SyntaxKind};
use sql_dialect_fmt_parser::parse;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

/// The signature a faithful formatter must preserve: meaningful tokens, upper-cased, with the
/// synthesized `;` dropped. (Formatting may only touch trivia and keyword casing.)
fn signature(sql: &str) -> Vec<String> {
    tokenize(sql)
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

const CASES: &[&str] = &[
    // ---- SAMPLE: default method, fraction ----
    "select * from t sample (10)",
    "select * from t sample (0.1)",
    "select * from t sample (50.5)",
    "select * from t tablesample (20)",
    "select * from t tablesample (100)",
    // ---- SAMPLE: BERNOULLI / ROW (the row-wise methods), fraction ----
    "select * from t sample bernoulli (20)",
    "select * from t tablesample bernoulli (20)",
    "select * from t sample row (15)",
    "select * from t tablesample row (15)",
    // ---- SAMPLE: fixed-size `<n> ROWS` (BERNOULLI/ROW only) ----
    "select * from t sample (100 rows)",
    "select * from t sample row (100 rows)",
    "select * from t tablesample row (1000 rows)",
    "select * from t sample bernoulli (10 rows)",
    "select * from t sample row (1000000 rows)",
    // ---- SAMPLE: SYSTEM / BLOCK with SEED / REPEATABLE ----
    "select * from t sample system (1)",
    "select * from t sample block (5)",
    "select * from t sample system (1) seed (99)",
    "select * from t sample system (10) repeatable (42)",
    "select * from t tablesample block (3) seed (0)",
    // ---- SAMPLE: tight parens normalize to canonical spacing ----
    "select * from t sample(10)",
    "select * from t sample bernoulli(20)",
    "select * from t sample row(50 rows)",
    // ---- SAMPLE: with alias (AS and implicit), on a derived table ----
    "select * from t sample (10) as s",
    "select * from t sample (10) s",
    "select s.* from t sample bernoulli (5) as s",
    "select * from (select a from u) sample (5)",
    "select * from (select a from u where a > 0) sample (5) as d",
    // ---- SAMPLE: combined with other clauses ----
    "select a from t sample (50) where a > 0",
    "select a, count(*) from t sample (25) group by a",
    "select * from t sample (10) order by 1 limit 5",
    "select * from t1 sample (10) join t2 sample (20) on t1.id = t2.id",
    "select * from t1 sample (10), t2 sample (5)",
    "select * from t sample (10) union all select * from u sample (20)",
    // ---- ASOF JOIN: MATCH_CONDITION operators ----
    "select * from a asof join b match_condition (a.t >= b.t)",
    "select * from a asof join b match_condition (a.t <= b.t)",
    "select * from a asof join b match_condition (a.t > b.t)",
    "select * from a asof join b match_condition (a.t < b.t)",
    // ---- ASOF JOIN: ON (single + multi-equality) and USING ----
    "select * from q asof join t match_condition (q.ts >= t.ts) on q.sym = t.sym",
    "select * from q asof join t match_condition (q.ts >= t.ts) on q.k = t.k and q.j = t.j",
    "select * from a asof join b match_condition (a.t <= b.t) using (sym)",
    "select * from a asof join b match_condition (a.t <= b.t) using (sym, mkt)",
    // ---- ASOF JOIN: aliases, qualified columns, ordering ----
    "select t.a, q.b from trades t asof join quotes q match_condition (t.ts >= q.ts) on t.sym = q.sym",
    "select t.stock, q.price from trades as t asof join quotes as q match_condition (t.tt >= q.qt) on t.s = q.s order by t.stock",
    // ---- ASOF JOIN: chained with an ordinary join, and with a derived right side ----
    "select * from a join b on a.id = b.id asof join c match_condition (a.t >= c.t) on a.k = c.k",
    "select * from a asof join (select * from c) d match_condition (a.t >= d.t) on a.k = d.k",
    // ---- SAMPLE + ASOF together ----
    "select * from a sample (10) asof join b match_condition (a.t >= b.t) on a.k = b.k",
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

// ---- exact-string goldens (canonical layout) ----

#[test]
fn sample_default_golden() {
    assert_eq!(
        fmt("select * from t sample (10)"),
        "SELECT *\nFROM t SAMPLE (10);\n"
    );
}

#[test]
fn sample_bernoulli_golden() {
    // The method word is a plain identifier (not reserved) so it keeps its case and hugs the paren.
    assert_eq!(
        fmt("select * from t tablesample bernoulli(25) repeatable(99)"),
        "SELECT *\nFROM t TABLESAMPLE bernoulli(25) repeatable(99);\n",
    );
}

#[test]
fn sample_row_rows_golden() {
    // `ROW` is the reserved keyword `ROW_KW`, so it up-cases and takes a space before `(`.
    assert_eq!(
        fmt("select * from t sample row (100 rows)"),
        "SELECT *\nFROM t SAMPLE ROW (100 ROWS);\n",
    );
}

#[test]
fn sample_on_derived_table_golden() {
    assert_eq!(
        fmt("select * from (select a from u) sample (5)"),
        "SELECT *\nFROM (\n    SELECT a\n    FROM u\n) SAMPLE (5);\n",
    );
}

#[test]
fn asof_join_with_on_golden() {
    assert_eq!(
        fmt("select * from q asof join t match_condition (q.ts >= t.ts) on q.sym = t.sym"),
        "SELECT *\nFROM q\nASOF JOIN t MATCH_CONDITION (q.ts >= t.ts) ON q.sym = t.sym;\n",
    );
}

#[test]
fn asof_join_without_on_golden() {
    assert_eq!(
        fmt("select * from a asof join b match_condition (a.t >= b.t)"),
        "SELECT *\nFROM a\nASOF JOIN b MATCH_CONDITION (a.t >= b.t);\n",
    );
}

#[test]
fn asof_join_using_golden() {
    assert_eq!(
        fmt("select * from a asof join b match_condition (a.t <= b.t) using (sym, mkt)"),
        "SELECT *\nFROM a\nASOF JOIN b MATCH_CONDITION (a.t <= b.t) USING (sym, mkt);\n",
    );
}

/// Outside the `ASOF JOIN` / sampling positions, `asof`, `bernoulli`, `system`, `seed`,
/// `match_condition`, `repeatable`, `block` are not reserved — they stay ordinary identifiers and
/// are not up-cased. (`SAMPLE` / `TABLESAMPLE` *are* reserved, so they are not in this list.)
#[test]
fn sampling_and_asof_words_are_not_reserved() {
    assert_eq!(
        fmt("select asof, system, bernoulli, seed, match_condition, repeatable, block from t"),
        "SELECT asof, system, bernoulli, seed, match_condition, repeatable, block\nFROM t;\n",
    );
}
