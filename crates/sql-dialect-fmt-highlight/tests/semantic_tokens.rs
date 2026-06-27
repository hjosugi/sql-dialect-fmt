//! Property-style matrix for the LSP semantic-token mapping.
//!
//! Mirrors the project's position×shape convention: a broad `CASES` array of real Snowflake SQL
//! exercised against four invariants:
//!
//! * resolve never panics and stays within bounds,
//! * line-tokens are well-formed (single line, positive UTF-16 length, non-overlapping),
//! * delta-encoding round-trips back to the absolute positions,
//! * every emitted token type/modifier is in the declared legend.
//!
//! Plus targeted assertions for injections, multi-line splitting, and UTF-16 columns.

use sql_dialect_fmt_highlight::{
    delta_encode, detect_injections, line_tokens, resolve_tokens, semantic_token, semantic_tokens,
    semantic_tokens_lsp, HighlightKind, InjectedLanguage, SemanticTokenModifiers,
    SemanticTokenType,
};

/// A wide spread of statement shapes and embedded-token positions.
const CASES: &[&str] = &[
    // --- bare selects / projections ---
    "SELECT 1",
    "SELECT *",
    "SELECT a, b, c FROM t",
    "select lower(name) as n from users where id = 1",
    "SELECT DISTINCT region FROM sales",
    "SELECT a FROM t WHERE x BETWEEN 1 AND 10",
    "SELECT count(*) FROM t GROUP BY region HAVING count(*) > 5",
    "SELECT a FROM t ORDER BY a DESC NULLS LAST LIMIT 10 OFFSET 5",
    "SELECT a FROM t QUALIFY row_number() OVER (PARTITION BY g ORDER BY a) = 1",
    // --- semi-structured / casts / operators ---
    "SELECT payload:customer.id::VARIANT FROM raw",
    "SELECT v['key'][0]::NUMBER FROM t",
    "SELECT a::NUMBER(38,0), b::VARCHAR FROM t",
    "SELECT col -> col2, fn(x => 1) FROM t",
    "SELECT AI_COMPLETE('claude-4-sonnet', 'summarize this') AS answer",
    "SELECT SNOWFLAKE.CORTEX.COMPLETE('claude-4-sonnet', 'summarize this') AS answer",
    "SELECT a ->> b FROM t",
    "SELECT 'long' || 'oat' AS s",
    "SELECT a |> b FROM t",
    // --- variables, binds, stages ---
    "SELECT $1, $2 FROM @my_stage/path",
    "SELECT * FROM @~/user_stage",
    "SELECT * FROM @%my_table",
    "INSERT INTO t VALUES (:bind1, :bind2)",
    "SELECT col WHERE id = ?",
    "SELECT $session_var FROM t",
    // --- strings, comments, numbers, unicode ---
    "SELECT '長芋', 3.14, 1e10, .5, 100. FROM t -- trailing\n",
    "SELECT 1 /* block\ncomment */ FROM t",
    "-- leading comment\nSELECT 1",
    "SELECT '' AS empty, 'it''s' AS escaped FROM t",
    "SELECT \"Quoted Ident\", \"with\"\"quote\" FROM \"Tbl\"",
    // --- DDL / DML / scripting ---
    "CREATE OR REPLACE TABLE t (a INT, b STRING)",
    "CREATE TEMPORARY TABLE x AS SELECT 1",
    "UPDATE t SET a = 1 WHERE id = 2",
    "DELETE FROM t WHERE a IS NULL",
    "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET a = s.a",
    "WITH cte AS (SELECT 1) SELECT * FROM cte",
    "GRANT SELECT ON TABLE t TO ROLE r",
    "CALL my_proc(1, 2)",
    "BEGIN LET x := 1; RETURN x; END",
    // --- embedded-language bodies ---
    "CREATE FUNCTION f() RETURNS STRING LANGUAGE JAVASCRIPT AS $$ return 'x'; $$",
    "CREATE FUNCTION g() RETURNS INT LANGUAGE PYTHON AS $$\ndef run():\n  return 1\n$$",
    "CREATE PROCEDURE p() RETURNS STRING LANGUAGE SQL AS $$ BEGIN RETURN 'ok'; END $$",
    "EXECUTE IMMEDIATE $$ SELECT 1 $$",
    "CREATE FUNCTION j() RETURNS INT LANGUAGE JAVA HANDLER = 'C.m' AS $$ class C {} $$",
    // --- set ops / unions / pipes ---
    "SELECT 1 UNION ALL SELECT 2 EXCEPT SELECT 3",
    "SELECT a FROM t1 JOIN t2 ON t1.id = t2.id LEFT JOIN t3 USING (k)",
    // --- multi-statement scripts ---
    "SELECT 1; SELECT 2; SELECT 3;",
    "CREATE TABLE a(x INT); INSERT INTO a VALUES (1); SELECT * FROM a;",
];

/// Rebuild `(line, start_char)` from a delta-encoded stream, the way an editor decodes it.
fn decode(deltas: &[[u32; 5]]) -> Vec<(u32, u32)> {
    let (mut line, mut col) = (0u32, 0u32);
    let mut out = Vec::with_capacity(deltas.len());
    for d in deltas {
        if d[0] == 0 {
            col += d[1];
        } else {
            line += d[0];
            col = d[1];
        }
        out.push((line, col));
    }
    out
}

#[test]
fn resolve_stays_within_bounds_and_never_panics() {
    for &sql in CASES {
        for tok in resolve_tokens(sql) {
            assert!(tok.range.start <= tok.range.end, "{sql:?}: inverted range");
            assert!(tok.range.end <= sql.len(), "{sql:?}: range past EOF");
        }
    }
}

#[test]
fn line_tokens_are_single_line_and_non_overlapping() {
    for &sql in CASES {
        let toks = line_tokens(sql);
        // Single line + positive length by construction.
        for t in &toks {
            assert!(t.length > 0, "{sql:?}: zero-length token");
        }
        // Sorted by (line, start_char) and non-overlapping within a line.
        for pair in toks.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let ordered = (a.line, a.start_char) <= (b.line, b.start_char);
            assert!(ordered, "{sql:?}: tokens out of order");
            if a.line == b.line {
                assert!(
                    a.start_char + a.length <= b.start_char,
                    "{sql:?}: overlapping tokens on line {}",
                    a.line
                );
            }
        }
    }
}

#[test]
fn delta_encoding_round_trips_to_absolute_positions() {
    for &sql in CASES {
        let lines = line_tokens(sql);
        let absolute: Vec<(u32, u32)> = lines.iter().map(|t| (t.line, t.start_char)).collect();
        let decoded = decode(&semantic_tokens_lsp(sql));
        assert_eq!(
            decoded, absolute,
            "{sql:?}: delta encoding did not round-trip"
        );
        // The wire stream carries the same length/type/modifiers as the line tokens.
        let encoded = delta_encode(&lines);
        for (enc, line) in encoded.iter().zip(&lines) {
            assert_eq!(enc[2], line.length, "{sql:?}");
            assert_eq!(enc[3], line.token_type, "{sql:?}");
            assert_eq!(enc[4], line.modifiers, "{sql:?}");
        }
    }
}

#[test]
fn every_token_type_and_modifier_is_in_the_legend() {
    let max_type = SemanticTokenType::LEGEND.len() as u32;
    let all_modifier_bits: u32 = (0..SemanticTokenModifiers::LEGEND.len())
        .map(|i| 1u32 << i)
        .sum();
    for &sql in CASES {
        for t in line_tokens(sql) {
            assert!(t.token_type < max_type, "{sql:?}: token type out of legend");
            assert_eq!(
                t.modifiers & !all_modifier_bits,
                0,
                "{sql:?}: modifier bit outside legend"
            );
        }
    }
}

#[test]
fn bundled_tokens_and_injections_are_consistent() {
    for &sql in CASES {
        let sem = semantic_tokens(sql);
        // Bundled tokens equal the standalone resolve.
        assert_eq!(sem.tokens, resolve_tokens(sql), "{sql:?}");
        assert_eq!(sem.injections, detect_injections(sql), "{sql:?}");
        // Each injection range is exactly a String (dollar-quoted) token in the stream.
        for inj in &sem.injections {
            let matched = sem.tokens.iter().any(|t| {
                t.range == inj.range
                    && t.token_type == SemanticTokenType::String
                    && sql[t.range.clone()].starts_with("$$")
            });
            assert!(
                matched,
                "{sql:?}: injection {inj:?} has no matching $$ token"
            );
        }
    }
}

#[test]
fn injection_languages_track_the_language_clause() {
    // Located by content (robust against CASES reordering); each is a single-body statement.
    let expectations: &[(&str, InjectedLanguage)] = &[
        (
            "CREATE FUNCTION f() RETURNS STRING LANGUAGE JAVASCRIPT AS $$ return 'x'; $$",
            InjectedLanguage::JavaScript,
        ),
        (
            "CREATE FUNCTION g() RETURNS INT LANGUAGE PYTHON AS $$\ndef run():\n  return 1\n$$",
            InjectedLanguage::Python,
        ),
        (
            "CREATE PROCEDURE p() RETURNS STRING LANGUAGE SQL AS $$ BEGIN RETURN 'ok'; END $$",
            InjectedLanguage::Sql,
        ),
        ("EXECUTE IMMEDIATE $$ SELECT 1 $$", InjectedLanguage::Sql),
        (
            "CREATE FUNCTION j() RETURNS INT LANGUAGE JAVA HANDLER = 'C.m' AS $$ class C {} $$",
            InjectedLanguage::Java,
        ),
    ];
    for &(sql, lang) in expectations {
        assert!(
            CASES.contains(&sql),
            "expectation drifted from CASES: {sql:?}"
        );
        let injections = detect_injections(sql);
        assert_eq!(injections.len(), 1, "{sql:?}: expected one injection");
        assert_eq!(injections[0].language, lang, "{sql:?}");
        let body = &sql[injections[0].range.clone()];
        assert!(
            body.starts_with("$$") && body.ends_with("$$"),
            "{sql:?}: body delimiters"
        );
    }
}

#[test]
fn semantic_token_mapping_agrees_with_classify() {
    // The mapping must be a total function of HighlightKind and never disagree across calls.
    use HighlightKind::*;
    let significant = [
        Keyword,
        Type,
        Identifier,
        QuotedIdentifier,
        String,
        DollarString,
        Number,
        Variable,
        Operator,
        Comment,
    ];
    for kind in significant {
        assert!(semantic_token(kind).is_some());
    }
    for kind in [Whitespace, Punctuation, Error] {
        assert!(semantic_token(kind).is_none());
    }
}

#[test]
fn cortex_and_aisql_functions_are_default_library_functions() {
    for (sql, fn_name) in [
        (
            "SELECT AI_COMPLETE('claude-4-sonnet', 'summarize this')",
            "AI_COMPLETE",
        ),
        (
            "SELECT SNOWFLAKE.CORTEX.COMPLETE('claude-4-sonnet', 'summarize this')",
            "COMPLETE",
        ),
    ] {
        let sem = semantic_tokens(sql);
        let token = sem
            .tokens
            .iter()
            .find(|token| &sql[token.range.clone()] == fn_name)
            .expect("function token");
        assert_eq!(token.token_type, SemanticTokenType::Function);
        assert!(token
            .modifiers
            .contains(SemanticTokenModifiers::DEFAULT_LIBRARY));
    }
}

#[test]
fn utf16_columns_account_for_wide_chars() {
    // 長芋 (2 chars, 1 UTF-16 unit each, 3 bytes each) sits before `FROM` on line 0.
    let sql = "SELECT '長芋' FROM t";
    let toks = line_tokens(sql);
    let from = toks
        .iter()
        .find(|t| t.token_type == SemanticTokenType::Keyword.index() && t.length == 4)
        .expect("FROM keyword token");
    // SELECT(6) + space(1) + '長芋'(4 utf16: quote+長+芋+quote) + space(1) = column 12.
    assert_eq!(from.start_char, 12, "FROM should start at UTF-16 column 12");
    assert_eq!(from.line, 0);
}

#[test]
fn multiline_dollar_body_splits_per_line() {
    let sql = "AS $$\nline1\nline2\n$$";
    let toks = line_tokens(sql);
    // The dollar body is a String token; it must be split into one piece per non-empty line.
    let string_lines: Vec<u32> = toks
        .iter()
        .filter(|t| t.token_type == SemanticTokenType::String.index())
        .map(|t| t.line)
        .collect();
    // Lines 0 ("$$"), 1 ("line1"), 2 ("line2"), 3 ("$$") all carry a piece.
    assert_eq!(string_lines, vec![0, 1, 2, 3]);
    // And still exactly one injection covering the whole body.
    assert_eq!(detect_injections(sql).len(), 1);
}

#[test]
fn empty_and_trivia_only_inputs_are_empty() {
    for sql in [
        "",
        "   ",
        "\n\n",
        "-- only a comment",
        "/* x */",
        "   ;  ;  ",
    ] {
        let lines = line_tokens(sql);
        // Comments still produce tokens; pure whitespace / punctuation do not.
        if sql.trim_start().starts_with("--") || sql.contains("/*") {
            assert!(!lines.is_empty(), "{sql:?}: comment should yield a token");
        } else {
            assert!(lines.is_empty(), "{sql:?}: should yield no tokens");
        }
        // Never panics regardless.
        let _ = semantic_tokens_lsp(sql);
        let _ = semantic_tokens(sql);
    }
}
