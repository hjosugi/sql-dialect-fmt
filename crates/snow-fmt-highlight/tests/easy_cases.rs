use snow_fmt_test_fixtures::{EASY_CASES, MINIMUM_EMBEDDED_EASY_CASES};
use snow_fmt_test_support::highlight::assert_highlight_lossless_with_context;

#[test]
fn embedded_easy_cases_highlight_losslessly() {
    assert!(
        EASY_CASES.len() >= MINIMUM_EMBEDDED_EASY_CASES,
        "highlight regression suite should keep broad easy fixture coverage"
    );

    for case in EASY_CASES {
        for (label, sql) in case.sqls() {
            assert_highlight_lossless_with_context(sql, &format!("{}:{label}", case.name));
        }
    }
}
