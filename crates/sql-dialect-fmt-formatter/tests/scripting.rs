//! Property-test matrix for Snowflake Scripting blocks (Phase 8).
//!
//! Snowflake Scripting is the procedural block language: `[DECLARE …] BEGIN … [EXCEPTION …] END`,
//! with `LET`/assignment, `RETURN`, `IF`/`CASE`, `FOR`/`WHILE`/`REPEAT`/`LOOP`, cursors, RESULTSETs,
//! nested blocks, and embedded SQL. This file pins the formatter's behaviour over a broad
//! position × shape matrix the same way `format.rs` does for the corpus: every case must
//!
//! * **parse clean** — no diagnostics on the input,
//! * be **idempotent** — `format(format(x)) == format(x)`,
//! * stay **output-valid** — the formatted text reparses without new errors, and
//! * be **token-preserving** — the sequence of significant tokens (case-folded, trivia and the
//!   synthesized `;` dropped) is identical before and after formatting, so layout never drops,
//!   invents, or reorders a single piece of SQL.
//!
//! A handful of exact-string goldens then nail down the indentation/keyword-casing contract.
//!
//! Syntax cross-checked against docs.snowflake.com developer-guide/snowflake-scripting:
//! blocks, variables, branch (IF/CASE), and loops (FOR/WHILE/REPEAT/LOOP).

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::tokenize;
use sql_dialect_fmt_parser::parse;
use sql_dialect_fmt_syntax::SyntaxKind;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

/// The case-folded text of every significant token: trivia and the formatter-synthesized statement
/// terminators are dropped (the formatter normalizes `;` placement), and identifiers/keywords are
/// upper-cased so the comparison ignores only the formatter's keyword casing, never its content.
fn significant_token_texts(src: &str) -> Vec<String> {
    tokenize(src)
        .tokens
        .iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

/// A broad matrix of Snowflake Scripting blocks: every declared form, in varied positions and
/// nestings. Each must be accepted, idempotent, output-valid, and token-preserving.
const CASES: &[&str] = &[
    // ---- bare blocks ----
    "BEGIN RETURN 1; END",
    "BEGIN NULL; END",
    "BEGIN LET v := 1; RETURN v; END",
    // ---- DECLARE forms: type, DEFAULT, type+DEFAULT, inferred ----
    "DECLARE x INT; BEGIN RETURN x; END",
    "DECLARE x INT DEFAULT 1; BEGIN RETURN x; END",
    "DECLARE profit NUMBER(38, 2) DEFAULT 0.0; BEGIN RETURN profit; END",
    "DECLARE a INT DEFAULT 1; b STRING; c FLOAT DEFAULT 3.5; BEGIN RETURN a; END",
    // ---- cursors and RESULTSETs in DECLARE ----
    "DECLARE c1 CURSOR FOR SELECT price FROM invoices; BEGIN OPEN c1; END",
    "DECLARE res RESULTSET; BEGIN res := (SELECT 1); RETURN TABLE(res); END",
    "DECLARE res RESULTSET DEFAULT (SELECT 1); BEGIN RETURN TABLE(res); END",
    // ---- LET: typed, typed+DEFAULT, inferred, RESULTSET ----
    "BEGIN LET cost NUMBER(38, 2) := 100.0; RETURN cost; END",
    "BEGIN LET revenue NUMBER(38, 2) DEFAULT 110.0; RETURN revenue; END",
    "BEGIN LET label := 'hi'; RETURN label; END",
    "BEGIN LET res RESULTSET := (SELECT 1); RETURN TABLE(res); END",
    // ---- assignment statements ----
    "BEGIN x := 1; y := x + 2; RETURN y; END",
    "BEGIN obj := OBJECT_CONSTRUCT('a', 1); RETURN obj; END",
    // ---- RETURN variants ----
    "BEGIN RETURN; END",
    "BEGIN RETURN 'done'; END",
    "BEGIN RETURN a + b * c; END",
    // ---- IF / ELSEIF / ELSE / END IF ----
    "BEGIN IF (x > 0) THEN RETURN 'p'; END IF; END",
    "BEGIN IF (x > 0) THEN RETURN 'p'; ELSE RETURN 'n'; END IF; END",
    "BEGIN IF (c < 0) THEN r := 'neg'; ELSEIF (c = 0) THEN r := 'zero'; ELSE r := 'pos'; END IF; END",
    // ---- counter FOR loop: bare, BY, REVERSE ----
    "BEGIN FOR i IN 1 TO 5 DO total := total + i; END FOR; END",
    "BEGIN FOR i IN 1 TO 10 BY 2 DO total := total + i; END FOR; END",
    "BEGIN FOR i IN REVERSE 1 TO 10 DO total := total + i; END FOR; END",
    // ---- cursor FOR loop ----
    "BEGIN FOR rec IN c1 DO total := total + rec.price; END FOR; END",
    "BEGIN FOR r IN (SELECT id FROM tasks) DO INSERT INTO processed (id) VALUES (r.id); END FOR; END",
    // ---- WHILE / REPEAT / LOOP ----
    "BEGIN WHILE (counter < 5) DO counter := counter + 1; END WHILE; END",
    "BEGIN REPEAT counter := counter - 1; UNTIL (counter = 0) END REPEAT; END",
    "BEGIN LOOP IF (counter = 0) THEN BREAK; END IF; counter := counter - 1; END LOOP; END",
    // ---- CASE statement (searched and simple) ----
    "BEGIN CASE WHEN x > 0 THEN y := 'p'; ELSE y := 'n'; END CASE; END",
    "BEGIN CASE grade WHEN 1 THEN y := 'a'; WHEN 2 THEN y := 'b'; ELSE y := 'c'; END CASE; END",
    // ---- embedded SQL statements inside a block ----
    "BEGIN INSERT INTO t (a, b) VALUES (1, 2); RETURN 1; END",
    "BEGIN UPDATE t SET a = 1 WHERE id = 5; END",
    "BEGIN DELETE FROM t WHERE x > 0; END",
    "BEGIN MERGE INTO tgt t USING src s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.v = s.v; END",
    "BEGIN LET n := (SELECT count(*) FROM orders); RETURN n; END",
    // ---- CALL ----
    "BEGIN CALL my_proc(1, 2); END",
    "BEGIN CALL pkg.do_thing(x => 1, y => 2); END",
    // ---- EXCEPTION handlers ----
    "BEGIN RETURN 1; EXCEPTION WHEN OTHER THEN RETURN 'err'; END",
    "BEGIN x := 1; EXCEPTION WHEN statement_error THEN ROLLBACK; RETURN 'failed'; WHEN OTHER THEN RETURN 'unknown'; END",
    // ---- nested blocks ----
    "BEGIN BEGIN LET v := 1; RETURN v; END; END",
    "DECLARE x INT; BEGIN BEGIN x := 1; EXCEPTION WHEN OTHER THEN x := -1; END; RETURN x; END",
    // ---- nested control flow ----
    "BEGIN FOR i IN 1 TO 5 DO IF (i > 2) THEN total := total + i; END IF; END FOR; RETURN total; END",
    "BEGIN WHILE (x < 10) DO FOR j IN 1 TO 3 DO x := x + j; END FOR; END WHILE; END",
    // ---- the full DECLARE + BEGIN + EXCEPTION skeleton ----
    "DECLARE total INT DEFAULT 0; BEGIN FOR i IN 1 TO 5 DO total := total + i; END FOR; RETURN total; EXCEPTION WHEN OTHER THEN RETURN -1; END",
    // ---- block wrapped in EXECUTE IMMEDIATE $$ … $$ (body verbatim) ----
    "EXECUTE IMMEDIATE $$ BEGIN RETURN 1; END $$",
    // ---- END label ----
    "BEGIN RETURN 1; END",
    // ---- mixed: LET, IF, loop, embedded SQL, return ----
    "DECLARE c INT DEFAULT 0; BEGIN LET t := 0; WHILE (c < 3) DO t := t + c; c := c + 1; END WHILE; INSERT INTO log (n) VALUES (t); RETURN t; END",
];

#[test]
fn every_case_parses_clean() {
    for &sql in CASES {
        let parsed = parse(sql);
        assert!(
            parsed.errors().is_empty(),
            "unexpected parse errors for {sql:?}: {:?}",
            parsed.errors()
        );
        // Losslessness: the CST round-trips byte-for-byte.
        assert_eq!(
            parsed.syntax().to_string(),
            sql,
            "parse tree did not round-trip for {sql:?}"
        );
    }
}

#[test]
fn every_case_is_idempotent() {
    for &sql in CASES {
        let once = fmt(sql);
        let twice = fmt(&once);
        assert_eq!(once, twice, "formatting is not idempotent for {sql:?}");
    }
}

#[test]
fn every_case_reparses_clean() {
    for &sql in CASES {
        let out = fmt(sql);
        assert!(
            parse(&out).errors().is_empty(),
            "formatting introduced parse errors for {sql:?}\n--- out ---\n{out}",
        );
    }
}

#[test]
fn every_case_preserves_significant_tokens() {
    for &sql in CASES {
        let out = fmt(sql);
        assert_eq!(
            significant_token_texts(sql),
            significant_token_texts(&out),
            "formatting changed the significant tokens for {sql:?}\n--- out ---\n{out}",
        );
    }
}

// ---- exact-string goldens: the indentation + keyword-casing contract ----

#[test]
fn declare_begin_end_lays_out_one_decl_and_stmt_per_line() {
    assert_eq!(
        fmt("declare x int default 1; result string; begin let v := x; return v; end"),
        "DECLARE\n    x int DEFAULT 1;\n    result string;\nBEGIN\n    LET v := x;\n    RETURN v;\nEND;\n"
    );
}

#[test]
fn if_elseif_else_branches_are_flush_and_bodies_indented() {
    assert_eq!(
        fmt("begin if (a) then x := 1; elseif (b) then x := 2; else x := 3; end if; end"),
        "BEGIN\n    IF (a) THEN\n        x := 1;\n    ELSEIF (b) THEN\n        x := 2;\n    ELSE\n        x := 3;\n    END IF;\nEND;\n"
    );
}

#[test]
fn counter_for_loop_keeps_its_header_on_one_line() {
    assert_eq!(
        fmt("begin for i in 1 to 10 by 2 do total := total + i; end for; end"),
        "BEGIN\n    FOR i IN 1 TO 10 BY 2 DO\n        total := total + i;\n    END FOR;\nEND;\n"
    );
}

#[test]
fn cursor_for_loop_over_a_subquery() {
    assert_eq!(
        fmt("begin for r in (select id from tasks) do insert into processed (id) values (r.id); end for; end"),
        "BEGIN\n    FOR r IN (SELECT id FROM tasks) DO\n        INSERT INTO processed (id)\n        VALUES (r.id);\n    END FOR;\nEND;\n"
    );
}

#[test]
fn while_loop_body_is_indented() {
    assert_eq!(
        fmt("begin while (c < 5) do c := c + 1; end while; end"),
        "BEGIN\n    WHILE (c < 5) DO\n        c := c + 1;\n    END WHILE;\nEND;\n"
    );
}

#[test]
fn repeat_until_keeps_until_flush_with_the_loop() {
    assert_eq!(
        fmt("begin repeat c := c - 1; until (c = 0) end repeat; end"),
        "BEGIN\n    REPEAT\n        c := c - 1;\n    UNTIL (c = 0)\n    END REPEAT;\nEND;\n"
    );
}

#[test]
fn loop_with_break_inside_an_if() {
    assert_eq!(
        fmt("begin loop if (c = 0) then break; end if; c := c - 1; end loop; end"),
        "BEGIN\n    LOOP\n        IF (c = 0) THEN\n            BREAK;\n        END IF;\n        c := c - 1;\n    END LOOP;\nEND;\n"
    );
}

#[test]
fn exception_section_lays_out_one_handler_per_line() {
    assert_eq!(
        fmt("begin x := 1; exception when statement_error then rollback; return 'failed'; when other then return 'unknown'; end"),
        "BEGIN\n    x := 1;\nEXCEPTION\n    WHEN statement_error THEN\n        ROLLBACK;\n        RETURN 'failed';\n    WHEN other THEN\n        RETURN 'unknown';\nEND;\n"
    );
}

#[test]
fn nested_block_is_indented_inside_its_parent() {
    assert_eq!(
        fmt("begin begin let v := 1; return v; end; end"),
        "BEGIN\n    BEGIN\n        LET v := 1;\n        RETURN v;\n    END;\nEND;\n"
    );
}

#[test]
fn let_with_a_scalar_subquery_is_kept_on_one_line() {
    assert_eq!(
        fmt("begin let total := (select sum(amount) from orders); return total; end"),
        "BEGIN\n    LET total := (SELECT sum(amount) FROM orders);\n    RETURN total;\nEND;\n"
    );
}

#[test]
fn execute_immediate_dollar_body_is_preserved_verbatim() {
    // The `$$ … $$` body is a single delimited token: its bytes are never reflowed.
    let src = "execute immediate $$\nbegin\n  return 1;\nend\n$$";
    let out = fmt(src);
    assert!(
        out.starts_with("EXECUTE IMMEDIATE $$"),
        "header not up-cased: {out:?}"
    );
    assert!(
        out.contains("\nbegin\n  return 1;\nend\n$$"),
        "body changed: {out:?}"
    );
    assert_eq!(fmt(&out), out, "not idempotent");
}

#[test]
fn case_statement_lays_out_each_arm() {
    assert_eq!(
        fmt("begin case when x > 0 then y := 'p'; else y := 'n'; end case; end"),
        "BEGIN\n    CASE\n        WHEN x > 0 THEN\n            y := 'p';\n        ELSE\n            y := 'n';\n    END CASE;\nEND;\n"
    );
}

#[test]
fn simple_case_statement_lays_out_each_arm() {
    assert_eq!(
        fmt("begin case grade when 1 then y := 'a'; when 2 then y := 'b'; else y := 'c'; end case; end"),
        "BEGIN\n    CASE grade\n        WHEN 1 THEN\n            y := 'a';\n        WHEN 2 THEN\n            y := 'b';\n        ELSE\n            y := 'c';\n    END CASE;\nEND;\n"
    );
}
