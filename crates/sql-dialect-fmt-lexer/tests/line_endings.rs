//! Cross-platform line ending tests.
//!
//! sql-dialect-fmt is lossless: it must preserve Linux LF, Windows CRLF, old-Mac CR, and mixed files.

use sql_dialect_fmt_lexer::SyntaxKind::*;
use sql_dialect_fmt_test_support::lexer::{assert_lex_lossless, lex_pairs};

#[test]
fn newline_tokens_cover_linux_windows_and_old_mac() {
    let cases = [
        ("linux LF", "SELECT 1\nSELECT 2", "\n"),
        ("windows CRLF", "SELECT 1\r\nSELECT 2", "\r\n"),
        ("old mac CR", "SELECT 1\rSELECT 2", "\r"),
    ];

    for (name, sql, newline) in cases {
        let tokens = lex_pairs(sql);
        assert!(
            tokens.contains(&(NEWLINE, newline)),
            "missing newline token for {name}"
        );
        assert_lex_lossless(sql);
    }
}

#[test]
fn mixed_line_endings_are_preserved_in_order() {
    let sql = "SELECT 1\r\n-- windows comment\rSELECT 2\n// linux comment\r\nSELECT 3\r";
    let newlines: Vec<_> = lex_pairs(sql)
        .into_iter()
        .filter_map(|(kind, text)| (kind == NEWLINE).then_some(text))
        .collect();

    assert_eq!(newlines, vec!["\r\n", "\r", "\n", "\r\n", "\r"]);
    assert_lex_lossless(sql);
}

#[test]
fn line_comments_stop_before_every_newline_flavor() {
    let cases = [
        ("-- x\nSELECT", vec![(COMMENT, "-- x"), (NEWLINE, "\n")]),
        ("-- x\r\nSELECT", vec![(COMMENT, "-- x"), (NEWLINE, "\r\n")]),
        ("-- x\rSELECT", vec![(COMMENT, "-- x"), (NEWLINE, "\r")]),
        ("// x\r\nSELECT", vec![(COMMENT, "// x"), (NEWLINE, "\r\n")]),
    ];

    for (sql, expected_prefix) in cases {
        let tokens = lex_pairs(sql);
        assert_eq!(&tokens[..expected_prefix.len()], expected_prefix.as_slice());
        assert_lex_lossless(sql);
    }
}

#[test]
fn multiline_strings_comments_and_dollar_bodies_keep_original_newlines() {
    let sql = concat!(
        "SELECT 'a\r\nb\rc\n' AS s,\r\n",
        "/* block\r\ncomment\rstill\none token */\r",
        "$$\r\nreturn '長芋';\r$$\n"
    );
    let tokens = lex_pairs(sql);

    assert!(tokens
        .iter()
        .any(|token| token.0 == STRING && token.1 == "'a\r\nb\rc\n'"));
    assert!(tokens
        .iter()
        .any(|token| token.0 == BLOCK_COMMENT && token.1.contains("\r\ncomment\rstill\n")));
    assert!(tokens
        .iter()
        .any(|token| token.0 == DOLLAR_STRING && token.1.contains("\r\nreturn '長芋';\r")));
    assert_lex_lossless(sql);
}

#[test]
fn long_mixed_newline_file_is_lossless() {
    let endings = ["\n", "\r\n", "\r"];
    let mut sql = String::new();
    for i in 0..300 {
        sql.push_str("SELECT ");
        sql.push_str(&i.to_string());
        sql.push_str(" AS n");
        sql.push_str(&i.to_string());
        sql.push_str(endings[i % endings.len()]);
    }

    let lexed = assert_lex_lossless(&sql);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
}
