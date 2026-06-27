//! Parser tests for Snowflake Scripting blocks (Phase 8).
//!
//! Two contracts: (1) every well-formed block parses clean and the tree round-trips byte-for-byte
//! (losslessness), and (2) the structural node kinds the formatter relies on are actually produced.
//! Plus a few resilience cases (malformed blocks recover without panicking) and the contextual
//! up-casing edge cases (a variable literally named `default`/`break` is not mistaken for a keyword).
//!
//! Syntax cross-checked against docs.snowflake.com developer-guide/snowflake-scripting.

use snow_fmt_parser::SyntaxKind;
use snow_fmt_test_support::parser::{
    assert_has_node_kind, assert_parse_clean as clean, assert_parse_recovers as recovers,
};

/// Does the parse tree contain a *token* of the given kind? (`CONTEXTUAL_KEYWORD` is a token kind,
/// so it never appears among `descendants()` — we walk `descendants_with_tokens()` instead.)
fn has_token_kind(input: &str, kind: SyntaxKind) -> bool {
    snow_fmt_parser::parse(input)
        .syntax()
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .any(|t| t.kind() == kind)
}

/// Every shape of a Snowflake Scripting block must parse clean (no diagnostics) and round-trip.
const CLEAN_CASES: &[&str] = &[
    // bare blocks
    "BEGIN RETURN 1; END",
    "BEGIN NULL; END",
    // DECLARE: type / DEFAULT / type+DEFAULT / multiple
    "DECLARE x INT; BEGIN RETURN x; END",
    "DECLARE x INT DEFAULT 1; BEGIN RETURN x; END",
    "DECLARE profit NUMBER(38, 2) DEFAULT 0.0; BEGIN RETURN profit; END",
    "DECLARE a INT DEFAULT 1; b STRING; c FLOAT; BEGIN RETURN a; END",
    // cursors / RESULTSETs
    "DECLARE c1 CURSOR FOR SELECT price FROM invoices; BEGIN OPEN c1; END",
    "DECLARE res RESULTSET; BEGIN res := (SELECT 1); RETURN TABLE(res); END",
    // LET: typed / typed+DEFAULT / inferred / RESULTSET
    "BEGIN LET cost NUMBER(38, 2) := 100.0; RETURN cost; END",
    "BEGIN LET revenue NUMBER(38, 2) DEFAULT 110.0; RETURN revenue; END",
    "BEGIN LET label := 'hi'; RETURN label; END",
    "BEGIN LET res RESULTSET := (SELECT 1); RETURN TABLE(res); END",
    // assignment
    "BEGIN x := 1; y := x + 2; RETURN y; END",
    // RETURN variants
    "BEGIN RETURN; END",
    "BEGIN RETURN a + b; END",
    // IF / ELSEIF / ELSE
    "BEGIN IF (x > 0) THEN RETURN 'p'; END IF; END",
    "BEGIN IF (c < 0) THEN r := 'neg'; ELSEIF (c = 0) THEN r := 'z'; ELSE r := 'p'; END IF; END",
    // counter FOR: bare / BY / REVERSE
    "BEGIN FOR i IN 1 TO 5 DO total := total + i; END FOR; END",
    "BEGIN FOR i IN 1 TO 10 BY 2 DO total := total + i; END FOR; END",
    "BEGIN FOR i IN REVERSE 1 TO 10 DO total := total + i; END FOR; END",
    // cursor FOR
    "BEGIN FOR rec IN c1 DO total := total + rec.price; END FOR; END",
    "BEGIN FOR r IN (SELECT id FROM tasks) DO INSERT INTO processed (id) VALUES (r.id); END FOR; END",
    // WHILE / REPEAT / LOOP
    "BEGIN WHILE (counter < 5) DO counter := counter + 1; END WHILE; END",
    "BEGIN REPEAT counter := counter - 1; UNTIL (counter = 0) END REPEAT; END",
    "BEGIN LOOP IF (counter = 0) THEN BREAK; END IF; counter := counter - 1; END LOOP; END",
    "BEGIN LOOP CONTINUE; END LOOP; END",
    // CASE statement
    "BEGIN CASE WHEN x > 0 THEN y := 'p'; ELSE y := 'n'; END CASE; END",
    "BEGIN CASE grade WHEN 1 THEN y := 'a'; ELSE y := 'c'; END CASE; END",
    // embedded SQL
    "BEGIN INSERT INTO t (a, b) VALUES (1, 2); RETURN 1; END",
    "BEGIN UPDATE t SET a = 1 WHERE id = 5; END",
    "BEGIN DELETE FROM t WHERE x > 0; END",
    "BEGIN MERGE INTO tgt t USING src s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.v = s.v; END",
    // CALL
    "BEGIN CALL my_proc(1, 2); END",
    // EXCEPTION
    "BEGIN RETURN 1; EXCEPTION WHEN OTHER THEN RETURN 'err'; END",
    "BEGIN x := 1; EXCEPTION WHEN statement_error THEN ROLLBACK; WHEN OTHER THEN RETURN 'u'; END",
    // nested blocks and control flow
    "BEGIN BEGIN LET v := 1; RETURN v; END; END",
    "BEGIN FOR i IN 1 TO 5 DO IF (i > 2) THEN total := total + i; END IF; END FOR; RETURN total; END",
    // the full skeleton
    "DECLARE total INT DEFAULT 0; BEGIN FOR i IN 1 TO 5 DO total := total + i; END FOR; RETURN total; EXCEPTION WHEN OTHER THEN RETURN -1; END",
];

#[test]
fn every_clean_case_parses_without_errors_and_roundtrips() {
    for &sql in CLEAN_CASES {
        clean(sql);
    }
}

#[test]
fn block_produces_its_structural_node_kinds() {
    assert_has_node_kind("BEGIN RETURN 1; END", SyntaxKind::BLOCK_STMT);
    assert_has_node_kind("BEGIN RETURN 1; END", SyntaxKind::STMT_LIST);
    assert_has_node_kind(
        "DECLARE x INT DEFAULT 1; BEGIN RETURN x; END",
        SyntaxKind::DECLARE_SECTION,
    );
    assert_has_node_kind(
        "DECLARE x INT DEFAULT 1; BEGIN RETURN x; END",
        SyntaxKind::DECLARE_ITEM,
    );
    assert_has_node_kind("BEGIN LET v := 1; END", SyntaxKind::LET_STMT);
    assert_has_node_kind("BEGIN x := 1; END", SyntaxKind::ASSIGN_STMT);
    assert_has_node_kind("BEGIN RETURN 1; END", SyntaxKind::RETURN_STMT);
    assert_has_node_kind("BEGIN IF (a) THEN x := 1; END IF; END", SyntaxKind::IF_STMT);
    assert_has_node_kind(
        "BEGIN CASE WHEN a THEN x := 1; ELSE x := 2; END CASE; END",
        SyntaxKind::CASE_STMT,
    );
    assert_has_node_kind(
        "BEGIN CASE WHEN a THEN x := 1; ELSE x := 2; END CASE; END",
        SyntaxKind::CASE_STMT_WHEN,
    );
    assert_has_node_kind(
        "BEGIN FOR i IN 1 TO 5 DO x := i; END FOR; END",
        SyntaxKind::LOOP_STMT,
    );
    assert_has_node_kind(
        "BEGIN WHILE (a) DO x := 1; END WHILE; END",
        SyntaxKind::LOOP_STMT,
    );
    assert_has_node_kind(
        "BEGIN REPEAT x := 1; UNTIL (a) END REPEAT; END",
        SyntaxKind::LOOP_STMT,
    );
    assert_has_node_kind("BEGIN LOOP BREAK; END LOOP; END", SyntaxKind::LOOP_STMT);
    assert_has_node_kind(
        "BEGIN RETURN 1; EXCEPTION WHEN OTHER THEN RETURN 0; END",
        SyntaxKind::EXCEPTION_SECTION,
    );
    assert_has_node_kind(
        "BEGIN RETURN 1; EXCEPTION WHEN OTHER THEN RETURN 0; END",
        SyntaxKind::EXCEPTION_WHEN,
    );
    assert_has_node_kind("BEGIN CALL p(1); END", SyntaxKind::CALL_STMT);
    // An embedded SQL *statement* inside a block reuses the full SQL machinery (a SELECT/INSERT/…
    // statement is delegated to the top-level statement parser, so it is structurally parsed).
    assert_has_node_kind("BEGIN SELECT count(*) FROM t; END", SyntaxKind::SELECT_STMT);
    assert_has_node_kind(
        "BEGIN INSERT INTO t (a) VALUES (1); END",
        SyntaxKind::INSERT_STMT,
    );
}

#[test]
fn nested_block_yields_two_block_stmts() {
    let count = snow_fmt_parser::parse("BEGIN BEGIN RETURN 1; END; END")
        .syntax()
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::BLOCK_STMT)
        .count();
    assert_eq!(count, 2, "expected an outer and an inner BLOCK_STMT");
}

#[test]
fn contextual_scripting_words_are_tagged_only_in_position() {
    // The structural words up-case as contextual keywords where the grammar expects them …
    assert!(has_token_kind(
        "BEGIN FOR i IN 1 TO 5 DO x := i; END FOR; END",
        SyntaxKind::CONTEXTUAL_KEYWORD
    ));
    assert!(has_token_kind(
        "BEGIN FOR i IN REVERSE 1 TO 5 DO x := i; END FOR; END",
        SyntaxKind::CONTEXTUAL_KEYWORD
    ));
    assert!(has_token_kind(
        "DECLARE x INT DEFAULT 1; BEGIN RETURN x; END",
        SyntaxKind::CONTEXTUAL_KEYWORD
    ));
    assert!(has_token_kind(
        "BEGIN LOOP BREAK; END LOOP; END",
        SyntaxKind::CONTEXTUAL_KEYWORD
    ));
    // … and a variable literally named `default` (an assignment target) is NOT tagged.
    assert!(!has_token_kind(
        "BEGIN default := 1; END",
        SyntaxKind::CONTEXTUAL_KEYWORD
    ));
}

#[test]
fn a_variable_named_like_a_scripting_word_is_not_a_keyword() {
    // `default`/`break` are not reserved: as an assignment target or DECLARE name they stay plain
    // identifiers, so the block must still parse clean and round-trip.
    clean("BEGIN default := 1; RETURN default; END");
    clean("DECLARE break INT DEFAULT 0; BEGIN RETURN break; END");
}

#[test]
fn malformed_blocks_recover_losslessly() {
    for sql in [
        "BEGIN RETURN 1;",           // missing END
        "DECLARE x INT BEGIN END",   // missing `;` after the decl
        "BEGIN IF (a) THEN x := 1;", // unterminated IF
        "BEGIN FOR i IN 1 TO 5 DO",  // unterminated FOR
        "BEGIN LOOP",                // unterminated LOOP
        "BEGIN WHILE (a)",           // missing DO and body
        "BEGIN EXCEPTION WHEN",      // truncated handler
    ] {
        recovers(sql);
    }
}
