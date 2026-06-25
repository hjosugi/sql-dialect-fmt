//! Exhaustive Phase 4 advanced-query coverage: MATCH_RECOGNIZE, hierarchical queries
//! (`START WITH` / `CONNECT BY`), Time Travel (`AT` / `BEFORE`), and the change-tracking
//! `CHANGES` clause.
//!
//! Every case below is asserted to (1) parse with no errors, (2) format to valid SQL, (3) be
//! idempotent (`format(format(x)) == format(x)`), and (4) preserve its meaningful tokens —
//! formatting only changes trivia and keyword casing, never the token *sequence*. The matrix
//! deliberately crosses *construct* (MATCH_RECOGNIZE / CONNECT BY / Time Travel / CHANGES) with
//! *shape* (every body sub-clause, every PER-MATCH and SKIP variant, every Time-Travel parameter
//! form, table aliases, joins, derived tables, CTEs) so regressions surface here.
//!
//! It mirrors `tests/subqueries.rs`: a position×shape `CASES` array plus the four property tests,
//! a structural-coverage test that proves each case takes its dedicated grammar path (rather than
//! silently falling back to verbatim), and a few exact-string goldens for the canonical layouts.

use snow_fmt_formatter::{format, FormatOptions};
use snow_fmt_lexer::tokenize;
use snow_fmt_parser::parse;
use snow_fmt_syntax::SyntaxKind;
use snow_fmt_test_support::parser::has_node_kind;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

/// Significant lexer token kinds: drop trivia and the statement terminators the formatter
/// synthesizes. A faithful formatter never changes this sequence.
fn significant_kinds(src: &str) -> Vec<SyntaxKind> {
    tokenize(src)
        .tokens
        .into_iter()
        .map(|t| t.kind)
        .filter(|k| !k.is_trivia() && *k != SyntaxKind::SEMICOLON)
        .collect()
}

/// The case-folded text of every significant token. Catches a dropped/added/reordered token even
/// when two tokens share a lexer kind (e.g. two identifiers), while tolerating the keyword
/// up-casing that formatting is allowed to do.
fn significant_text(src: &str) -> Vec<String> {
    tokenize(src)
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

const CASES: &[&str] = &[
    // ---- MATCH_RECOGNIZE: body sub-clauses, every combination ----
    // minimal: just PATTERN + DEFINE
    "select * from t match_recognize(pattern(a b+) define b as b.v > 0)",
    // + PARTITION BY + ORDER BY
    "select * from t match_recognize(partition by id order by ts pattern(a b+) define b as b.v > 0)",
    // multi-key PARTITION BY / ORDER BY
    "select * from t match_recognize(partition by region, company order by d, seq pattern(a+) define a as a.v > 0)",
    // + MEASURES (single)
    "select * from t match_recognize(order by ts measures last(price) as fp pattern(a+) define a as true)",
    // + MEASURES (multiple, comma-separated)
    "select * from t match_recognize(order by ts measures match_number() as mn, last(price) as fp, classifier() as cl pattern(a+) define a as true)",
    // ONE ROW PER MATCH
    "select * from t match_recognize(order by ts measures last(c) as lc one row per match pattern(a b+) define b as c > 0)",
    // ALL ROWS PER MATCH (bare)
    "select * from t match_recognize(order by ts all rows per match pattern(a b+) define b as c > 0)",
    // ALL ROWS PER MATCH SHOW EMPTY MATCHES
    "select * from t match_recognize(partition by a order by b measures match_number() as mn all rows per match show empty matches pattern(overavg*) define overavg as price > avg(price))",
    // ALL ROWS PER MATCH OMIT EMPTY MATCHES
    "select * from t match_recognize(partition by a order by b all rows per match omit empty matches pattern(overavg*) define overavg as price > avg(price))",
    // ALL ROWS PER MATCH WITH UNMATCHED ROWS
    "select * from t match_recognize(order by x measures match_number() as mn, classifier() as cl all rows per match with unmatched rows pattern(any_row up+) define any_row as true, up as price > lag(price))",
    // AFTER MATCH SKIP PAST LAST ROW
    "select * from t match_recognize(order by b one row per match after match skip past last row pattern(strt down+ up+) define down as price < lag(price), up as price > lag(price))",
    // AFTER MATCH SKIP TO NEXT ROW
    "select * from t match_recognize(order by b measures last(c) as lc one row per match after match skip to next row pattern(a b+) define b as c > 0)",
    // AFTER MATCH SKIP TO LAST <symbol>
    "select * from t match_recognize(order by b after match skip to last up pattern(strt up+) define up as price > lag(price))",
    // AFTER MATCH SKIP TO FIRST <symbol>
    "select * from t match_recognize(order by b after match skip to first up pattern(strt up+) define up as price > lag(price))",
    // PATTERN with alternation, grouping, and bounded quantifiers (regex-like body, kept verbatim)
    "select * from t match_recognize(order by b pattern(strt (down | up)+ fin{1,3}) define down as price < lag(price), up as price > lag(price), fin as price = 0)",
    // PATTERN with anchors and reluctant quantifiers
    "select * from t match_recognize(order by b pattern(^ a* b?? c+ $) define a as true, b as true, c as true)",
    // SUBSET clause
    "select * from t match_recognize(order by b measures sum(s.v) as sv pattern(a b+) subset s = (a, b) define b as b.v > 0)",
    // the full V-shape example (every clause present)
    "select * from stock_price_history match_recognize(partition by company order by price_date measures match_number() as match_number, first(price_date) as start_date, last(price_date) as end_date one row per match after match skip to last row_up pattern(row_before row_down+ row_up+) define row_down as price < lag(price), row_up as price > lag(price)) order by company, match_number",
    // MATCH_RECOGNIZE with an output alias
    "select * from t match_recognize(order by b pattern(a+) define a as a.v > 0) mr",
    // MATCH_RECOGNIZE on a derived table
    "select * from (select * from sales where company = 'abcd') match_recognize(order by d measures classifier() as cl all rows per match pattern(a up+) define up as price > lag(price))",
    // MATCH_RECOGNIZE feeding a join
    "select * from t match_recognize(order by b pattern(a+) define a as a.v > 0) mr join dim d on mr.k = d.k",
    // ---- CONNECT BY / START WITH: hierarchical queries ----
    "select id, manager_id, title from employees start with title = 'president' connect by manager_id = prior employee_id order by id",
    "select id, manager_id from t connect by prior id = manager_id start with manager_id is null",
    "select sys_connect_by_path(title, ' -> '), id from employees start with id = 1 connect by parent_id = prior id",
    "select id from t start with parent_id is null connect by prior id = parent_id and prior region = region",
    "select * from t connect by prior id = parent_id",
    // WHERE is applied before the hierarchical walk, so it precedes START WITH / CONNECT BY.
    "select id from t where active = true start with id = 1 connect by parent_id = prior id",
    "select id, level from t start with parent_id is null connect by prior id = parent_id group by id, level",
    // CONNECT BY inside a derived table
    "select * from (select id from t start with parent_id is null connect by prior id = parent_id) h",
    // ---- Time Travel: AT / BEFORE, every parameter form ----
    "select * from t at(timestamp => '2024-06-05 17:50:00'::timestamp_ltz)",
    "select * from t at(offset => -60*5) as t where t.flag = 'valid'",
    "select * from t before(statement => '8e5d0ca9-005e-44e6-b858-a8f5b37c5726') as oldt",
    "select * from t at(stream => 's1')",
    "select * from my_table at(offset => -120) h",
    // Time Travel on both sides of a join
    "select * from t1 at(timestamp => '2024-06-05'::timestamp_ltz) h join t2 at(timestamp => '2024-06-05'::timestamp_ltz) t on h.c1 = t.c1",
    // Time Travel with an explicit AS alias then a WHERE
    "select a, b from orders before (timestamp => '2024-01-01'::timestamp_ntz) as o where o.amt > 0",
    // ---- CHANGES: change-tracking ----
    "select * from t changes(information => default) at(timestamp => 'x')",
    "select * from t changes(information => append_only) before(offset => -60)",
    "select c1 from t changes(information => default) at(offset => -60) end(offset => -1)",
    "select * from t changes(information => append_only) before(offset => -60) end(timestamp => 'y')",
    // ---- Mixed / monster cases ----
    // CTE feeding a MATCH_RECOGNIZE
    "with src as (select * from raw where v is not null) select * from src match_recognize(order by ts measures match_number() as mn one row per match pattern(a b+) define b as b.v > a.v)",
    // hierarchical query over a Time-Travel table
    "select id from t at(offset => -3600) start with parent_id is null connect by prior id = parent_id",
    // MATCH_RECOGNIZE whose DEFINE predicates use windowed aggregates and navigation functions
    "select * from t match_recognize(partition by sym order by ts measures first(ts) as s, last(ts) as e all rows per match pattern(a x+ b) define x as price > avg(price) over (rows between unbounded preceding and current row), b as price < first(price))",
];

#[test]
fn all_cases_parse_clean() {
    for sql in CASES {
        let errors = parse(sql).errors().to_vec();
        assert!(errors.is_empty(), "parse errors for {sql:?}: {errors:?}");
    }
}

#[test]
fn parse_tree_round_trips() {
    // Losslessness: the parsed tree must reproduce the source byte-for-byte.
    for sql in CASES {
        let text = parse(sql).syntax().to_string();
        assert_eq!(&text, sql, "parse tree did not round-trip for {sql:?}");
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
            significant_kinds(sql),
            significant_kinds(&formatted),
            "token-kind sequence changed:\n{sql}\n---\n{formatted}"
        );
        assert_eq!(
            significant_text(sql),
            significant_text(&formatted),
            "token text (case-folded) changed:\n{sql}\n---\n{formatted}"
        );
    }
}

/// Every case must actually take its intended grammar path — i.e. produce the dedicated node —
/// rather than silently falling back to a verbatim ERROR/word run. This guards against a
/// regression where a clause stops being recognized but the case still "passes" the property
/// tests by round-tripping its raw text.
#[test]
fn each_construct_builds_its_node() {
    use SyntaxKind::*;
    let expectations: &[(SyntaxKind, &str)] = &[
        (MATCH_RECOGNIZE, "match_recognize"),
        (CONNECT_BY_CLAUSE, "connect by"),
        (START_WITH_CLAUSE, "start with"),
    ];
    for sql in CASES {
        let lower = sql.to_ascii_lowercase();
        for (kind, needle) in expectations {
            if lower.contains(needle) {
                assert!(
                    has_node_kind(sql, *kind),
                    "expected a {kind:?} node for {sql:?}"
                );
            }
        }
    }
    // MATCH_RECOGNIZE body sub-clauses appear across the matrix; assert the dedicated nodes exist.
    assert!(has_node_kind(CASES[0], PATTERN_CLAUSE));
    assert!(has_node_kind(CASES[0], DEFINE_CLAUSE));
    assert!(has_node_kind(CASES[3], MEASURES_CLAUSE));
    assert!(has_node_kind(CASES[16], SUBSET_CLAUSE)); // the SUBSET case
}

// ---- exact-string goldens for the canonical layouts ----

#[test]
fn match_recognize_golden_layout() {
    let out = fmt("select * from t match_recognize(partition by a order by b \
         measures match_number() as mn, first(price) as fp one row per match \
         after match skip past last row pattern(strt down+ up+) \
         define down as price < prev(price), up as price > prev(price))");
    assert_eq!(
        out,
        "SELECT *\n\
         FROM t MATCH_RECOGNIZE (\n\
         \x20   PARTITION BY a\n\
         \x20   ORDER BY b\n\
         \x20   MEASURES match_number() AS mn, first(price) AS fp\n\
         \x20   ONE ROW PER MATCH\n\
         \x20   AFTER MATCH SKIP PAST LAST ROW\n\
         \x20   PATTERN (strt down+ up+)\n\
         \x20   DEFINE down AS price < prev(price), up AS price > prev(price)\n\
         );\n"
    );
}

#[test]
fn connect_by_golden_layout() {
    let out = fmt("select id, name from emp start with manager_id is null \
         connect by prior id = manager_id");
    assert_eq!(
        out,
        "SELECT id, name\n\
         FROM emp\n\
         START WITH manager_id IS NULL\n\
         CONNECT BY PRIOR id = manager_id;\n"
    );
}

#[test]
fn time_travel_golden_layout() {
    assert_eq!(
        fmt("select * from t at(offset => -60*5) as t where t.flag = 'valid'"),
        "SELECT *\nFROM t AT (OFFSET => -60 * 5) AS t\nWHERE t.flag = 'valid';\n"
    );
    assert_eq!(
        fmt("select * from orders before (statement => 'abc') o"),
        "SELECT *\nFROM orders BEFORE (statement => 'abc') o;\n"
    );
}

#[test]
fn changes_golden_layout() {
    assert_eq!(
        fmt("select * from t changes(information => default) at(timestamp => 'x')"),
        "SELECT *\nFROM t CHANGES (information => default) AT (timestamp => 'x');\n"
    );
}

/// `at` / `before` / `changes` / `measures` / `pattern` / `define` are *soft* keywords: up-cased
/// only where the grammar recognizes them as a clause, and ordinary identifiers everywhere else.
/// (`connect` / `start` are genuinely reserved, so they are deliberately not in this list.)
#[test]
fn contextual_keywords_stay_identifiers_outside_their_clause() {
    assert_eq!(
        fmt("select at, before, changes, measures, pattern, define from t"),
        "SELECT at, before, changes, measures, pattern, define\nFROM t;\n"
    );
}
