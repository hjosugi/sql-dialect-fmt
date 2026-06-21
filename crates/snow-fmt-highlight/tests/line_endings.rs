use snow_fmt_highlight::{highlight, HighlightKind};
use snow_fmt_test_support::highlight::assert_highlight_lossless;

#[test]
fn highlights_linux_windows_and_old_mac_newlines_losslessly() {
    for newline in ["\n", "\r\n", "\r"] {
        let sql =
            format!("SELECT '長芋'{newline}FROM \"畑\"{newline}WHERE name ILIKE '長芋%'{newline}");
        let highlighted = highlight(&sql);

        assert!(highlighted
            .tokens
            .iter()
            .any(|token| token.kind == HighlightKind::String && token.text == "'長芋'"));
        assert_highlight_lossless(&sql);
    }
}

#[test]
fn mixed_line_endings_keep_exact_ranges() {
    let sql = "SELECT 1\r\n-- comment\rSELECT '長芋'\nFROM t\r\n->> SELECT * FROM $1\r";
    let highlighted = highlight(sql);

    let newline_texts: Vec<_> = highlighted
        .tokens
        .iter()
        .filter(|token| {
            token.kind == HighlightKind::Whitespace
                && (token.text == "\n" || token.text == "\r\n" || token.text == "\r")
        })
        .map(|token| token.text)
        .collect();

    assert_eq!(newline_texts, vec!["\r\n", "\r", "\n", "\r\n", "\r"]);
    assert_highlight_lossless(sql);
}

#[test]
fn long_mixed_newline_highlight_is_fast_and_lossless() {
    let endings = ["\n", "\r\n", "\r"];
    let mut sql = String::new();
    for i in 0..256 {
        sql.push_str("SELECT '長芋' AS label_");
        sql.push_str(&i.to_string());
        sql.push_str(" FROM table_");
        sql.push_str(&i.to_string());
        sql.push_str(endings[i % endings.len()]);
    }

    assert_highlight_lossless(&sql);
}
