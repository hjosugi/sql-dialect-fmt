//! Embedded SQL fixtures are the parser's always-on recovery corpus.
//!
//! Many cases intentionally cover Snowflake statements that the parser does not
//! fully understand yet. The contract here is lossless recovery for every byte;
//! grammar-specific "no diagnostics" assertions live in the focused phase tests.

use sql_dialect_fmt_test_fixtures::{EASY_CASES, MINIMUM_EMBEDDED_EASY_CASES};
use sql_dialect_fmt_test_support::parser::assert_parse_recovers_with_context;

#[test]
fn embedded_easy_cases_parse_losslessly_with_recovery() {
    assert!(EASY_CASES.len() >= MINIMUM_EMBEDDED_EASY_CASES);

    for case in EASY_CASES {
        for (label, sql) in case.sqls() {
            assert_parse_recovers_with_context(sql, &format!("{}:{label}", case.name));
        }
    }
}
