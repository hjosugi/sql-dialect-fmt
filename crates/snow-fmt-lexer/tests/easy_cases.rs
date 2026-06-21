//! Embedded SQL fixtures must all lex cleanly and losslessly.

use snow_fmt_test_fixtures::{EASY_CASES, MINIMUM_EMBEDDED_EASY_CASES};
use snow_fmt_test_support::lexer::assert_lex_lossless;

#[test]
fn embedded_easy_cases_lex_losslessly_without_errors() {
    assert!(EASY_CASES.len() >= MINIMUM_EMBEDDED_EASY_CASES);

    for case in EASY_CASES {
        for (label, sql) in case.sqls() {
            let lexed = assert_lex_lossless(sql);
            assert!(
                lexed.errors.is_empty(),
                "{}:{label} produced lexer errors: {:?}",
                case.name,
                lexed.errors
            );
        }
    }
}
