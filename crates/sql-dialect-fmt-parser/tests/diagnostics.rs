//! Diagnostic quality: human-readable messages and accurate token spans.
//!
//! These pin the *content* of representative parse errors and the byte range each one covers, so a
//! regression that reverts to debug `SyntaxKind::INTO_KW` text or to single-character spans is
//! caught. The parser never fails, so every input here still round-trips.

use sql_dialect_fmt_parser::parse;
use sql_dialect_fmt_test_support::parser::assert_parse_roundtrip;

/// The single error for `sql`, asserting the source still round-trips.
fn only_error(sql: &str) -> sql_dialect_fmt_parser::ParseError {
    assert_parse_roundtrip(sql);
    let parse = parse(sql);
    let errors = parse.errors();
    assert_eq!(
        errors.len(),
        1,
        "expected exactly one error for {sql:?}: {errors:?}"
    );
    errors[0].clone()
}

#[test]
fn expect_message_uses_human_keyword_name() {
    // `MERGE <name>` without `INTO` must read "expected INTO", not "expected SyntaxKind::INTO_KW".
    let err = parse("MERGE tgt USING src ON a = b")
        .errors()
        .iter()
        .find(|e| e.message.contains("INTO"))
        .cloned()
        .expect("an INTO diagnostic");
    assert_eq!(err.message, "expected INTO");
    assert!(!err.message.contains("SyntaxKind"));
    assert!(!err.message.contains("_KW"));
}

#[test]
fn expect_message_quotes_punctuation() {
    // A window/grouping construct missing its `(` should name `'('`, quoted.
    let parse = parse("SELECT * FROM t MATCH_RECOGNIZE");
    let msg = &parse
        .errors()
        .iter()
        .find(|e| e.message.contains("'('"))
        .expect("a '(' diagnostic")
        .message;
    assert!(msg.contains("'('"), "{msg:?}");
    assert!(!msg.contains("L_PAREN"), "{msg:?}");
}

#[test]
fn span_covers_the_whole_offending_token() {
    // The first error of `( where )` fires at the `where` token: the span must cover all 5 of its
    // bytes, not a single character.
    let sql = "( where )";
    let parse = parse(sql);
    let err = &parse.errors()[0];
    let start = sql.find("where").unwrap();
    assert_eq!(err.offset, start);
    assert_eq!(err.len, "where".len());
    assert_eq!(&sql[err.range()], "where");
}

#[test]
fn span_for_multibyte_token_is_byte_accurate() {
    // A multi-byte quoted-identifier name where a query is expected: the span must cover the full
    // byte length of the token (a 3-byte char inside two 1-byte quotes = 5 bytes) and stay on char
    // boundaries, so slicing the source by the range never panics.
    let sql = "WITH c AS \"芋\" SELECT 1";
    let err = parse(sql)
        .errors()
        .iter()
        .find(|e| e.message.contains("query"))
        .cloned()
        .expect("a query diagnostic");
    let start = sql.find('"').unwrap();
    assert_eq!(err.offset, start);
    assert_eq!(err.len, "\"芋\"".len());
    assert_eq!(&sql[err.range()], "\"芋\"");
}

#[test]
fn error_at_eof_is_zero_width_at_source_end() {
    // An error with nothing left to point at lands at the end of the source with zero length, so
    // the diagnostic still has a location and can never index out of range.
    let sql = "SELECT a FROM";
    let err = only_error(sql);
    assert!(err.message.contains("table reference"), "{:?}", err.message);
    assert_eq!(err.offset, sql.len());
    assert_eq!(err.len, 0);
    assert_eq!(err.range(), sql.len()..sql.len());
}

#[test]
fn parse_error_displays_human_message_with_location() {
    // The `Display` impl (std::error::Error) renders the message plus its byte offset.
    let err = only_error("SELECT a FROM");
    let shown = err.to_string();
    assert!(shown.starts_with("expected a table reference"), "{shown}");
    assert!(shown.contains("at byte 13"), "{shown}");
    // It is a real std::error::Error.
    let _: &dyn std::error::Error = &err;
}

#[test]
fn unbalanced_parens_at_eof_never_panic() {
    // A run of `(` exhausts the input mid-expression, so recovery reaches end of input while the
    // grammar still expects `)`/an expression. The now-total `advance` must not panic at EOF; the
    // input still round-trips and records diagnostics.
    let sql = "SELECT ((((((((";
    assert_parse_roundtrip(sql);
    assert!(!parse(sql).errors().is_empty());
}

#[test]
fn truncated_statements_recover_without_panic() {
    // A spread of statements truncated right at a token boundary (the worst case for the EOF path
    // of `advance`). Each must round-trip losslessly and never panic.
    for sql in [
        "MERGE",
        "INSERT INTO",
        "SELECT a FROM t WHERE",
        "CREATE TABLE t (",
        "UPDATE t SET",
        "GRANT SELECT ON",
    ] {
        assert_parse_roundtrip(sql);
    }
}
