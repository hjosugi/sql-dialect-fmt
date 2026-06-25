//! SAMPLE / TABLESAMPLE and ASOF JOIN parser conformance (Phase 4 table operators).
//!
//! Each valid case must (1) round-trip byte-for-byte and (2) parse with no diagnostics. The ASOF
//! cases additionally assert the expected node structure (`JOIN`, plus the `MATCH_CONDITION`
//! soft-keyword token). Incomplete / malformed inputs must still round-trip and never panic — the
//! parser is total.
//!
//! References:
//!   * <https://docs.snowflake.com/en/sql-reference/constructs/sample>
//!   * <https://docs.snowflake.com/en/sql-reference/constructs/asof-join>

use snow_fmt_parser::{parse, SyntaxKind};
use snow_fmt_test_support::parser::{assert_parse_clean, assert_parse_roundtrip};

/// Does the parse tree for `s` contain a token whose text is `text` (case-insensitive)? Used to
/// confirm a soft keyword (e.g. `MATCH_CONDITION`, `ASOF`) survived into the tree.
fn has_token_text(s: &str, text: &str) -> bool {
    parse(s)
        .syntax()
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .any(|t| t.text().eq_ignore_ascii_case(text))
}

fn has_kind(s: &str, kind: SyntaxKind) -> bool {
    parse(s).syntax().descendants().any(|n| n.kind() == kind)
}

const SAMPLE_CASES: &[&str] = &[
    // default method (fraction)
    "SELECT * FROM t SAMPLE (10)",
    "SELECT * FROM t TABLESAMPLE (20)",
    "SELECT * FROM t SAMPLE (0.5)",
    // explicit row-wise methods
    "SELECT * FROM t SAMPLE BERNOULLI (20)",
    "SELECT * FROM t TABLESAMPLE BERNOULLI (20)",
    "SELECT * FROM t SAMPLE ROW (15)",
    "SELECT * FROM t TABLESAMPLE ROW (15)",
    // fixed-size <n> ROWS
    "SELECT * FROM t SAMPLE (100 ROWS)",
    "SELECT * FROM t SAMPLE ROW (100 ROWS)",
    "SELECT * FROM t SAMPLE BERNOULLI (10 ROWS)",
    // block-wise methods with seed
    "SELECT * FROM t SAMPLE SYSTEM (1)",
    "SELECT * FROM t SAMPLE BLOCK (5)",
    "SELECT * FROM t SAMPLE SYSTEM (1) SEED (99)",
    "SELECT * FROM t SAMPLE SYSTEM (10) REPEATABLE (42)",
    // alias forms and derived table
    "SELECT * FROM t SAMPLE (10) AS s",
    "SELECT * FROM t SAMPLE (10) s",
    "SELECT * FROM (SELECT a FROM u) SAMPLE (5)",
    // combined with clauses / joins
    "SELECT a FROM t SAMPLE (50) WHERE a > 0",
    "SELECT * FROM t1 SAMPLE (10) JOIN t2 SAMPLE (20) ON t1.id = t2.id",
    "SELECT * FROM t1 SAMPLE (10), t2 SAMPLE (5)",
];

const ASOF_CASES: &[&str] = &[
    "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (a.t >= b.t)",
    "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (a.t <= b.t)",
    "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (a.t > b.t)",
    "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (a.t < b.t)",
    "SELECT * FROM q ASOF JOIN t MATCH_CONDITION (q.ts >= t.ts) ON q.sym = t.sym",
    "SELECT * FROM q ASOF JOIN t MATCH_CONDITION (q.ts >= t.ts) ON q.k = t.k AND q.j = t.j",
    "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (a.t <= b.t) USING (sym)",
    "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (a.t <= b.t) USING (sym, mkt)",
    "SELECT t.a, q.b FROM trades t ASOF JOIN quotes q MATCH_CONDITION (t.ts >= q.ts) ON t.sym = q.sym ORDER BY t.a",
    "SELECT * FROM a JOIN b ON a.id = b.id ASOF JOIN c MATCH_CONDITION (a.t >= c.t) ON a.k = c.k",
    "SELECT * FROM a ASOF JOIN (SELECT * FROM c) d MATCH_CONDITION (a.t >= d.t) ON a.k = d.k",
];

#[test]
fn sample_cases_parse_clean() {
    for sql in SAMPLE_CASES {
        assert_parse_clean(sql);
    }
}

#[test]
fn asof_cases_parse_clean() {
    for sql in ASOF_CASES {
        assert_parse_clean(sql);
    }
}

#[test]
fn asof_joins_form_join_nodes_with_match_condition() {
    for sql in ASOF_CASES {
        assert!(has_kind(sql, SyntaxKind::JOIN), "no JOIN node in {sql:?}");
        assert!(has_token_text(sql, "ASOF"), "ASOF token missing in {sql:?}");
        assert!(
            has_token_text(sql, "MATCH_CONDITION"),
            "MATCH_CONDITION token missing in {sql:?}"
        );
    }
}

#[test]
fn sample_attaches_to_table_ref() {
    // The SAMPLE keyword sits inside the sampled table's TABLE_REF, before the alias.
    for sql in SAMPLE_CASES {
        assert!(
            has_kind(sql, SyntaxKind::TABLE_REF),
            "no TABLE_REF node in {sql:?}"
        );
        assert!(
            has_token_text(sql, "SAMPLE") || has_token_text(sql, "TABLESAMPLE"),
            "SAMPLE token missing in {sql:?}"
        );
    }
}

/// The contextual words `asof` / `match_condition` are *not* reserved: used as ordinary
/// identifiers they stay plain `IDENT`s and the surrounding query parses cleanly.
#[test]
fn contextual_words_are_not_reserved() {
    for sql in [
        "SELECT asof, match_condition FROM t",
        "SELECT bernoulli, system, block, seed, repeatable FROM t",
        "SELECT a FROM asof",
        "SELECT match_condition FROM t WHERE match_condition > 0",
    ] {
        assert_parse_clean(sql);
    }
}

/// Incomplete / malformed sampling and ASOF inputs must still round-trip losslessly and never
/// panic (the parser is total).
#[test]
fn malformed_inputs_round_trip() {
    for sql in [
        "SELECT * FROM t SAMPLE",
        "SELECT * FROM t SAMPLE (",
        "SELECT * FROM t SAMPLE (10",
        "SELECT * FROM t SAMPLE ROW",
        "SELECT * FROM t SAMPLE ROW (",
        "SELECT * FROM t TABLESAMPLE BERNOULLI",
        "SELECT * FROM t SAMPLE SYSTEM (1) SEED",
        "SELECT * FROM a ASOF JOIN b",
        "SELECT * FROM a ASOF JOIN b MATCH_CONDITION",
        "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (",
        "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (a.t >=",
        "SELECT * FROM a ASOF JOIN b MATCH_CONDITION (a.t >= b.t) ON",
    ] {
        assert_parse_roundtrip(sql);
    }
}
