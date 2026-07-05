//! Comprehensive, careful conformance tests for the Snowflake SQL lexer.
//!
//! These are organized as:
//!   * **Property / fuzz tests** — the invariants that must hold for *any* input
//!     (losslessness, no empty tokens, never panics), checked over thousands of
//!     deterministically-generated strings.
//!   * **Targeted tests** — exact token streams for every operator boundary, every
//!     literal form, and the Snowflake-specific constructs the formatter must handle.
//!   * **Error tests** — malformed input must produce diagnostics (with correct
//!     offsets) instead of panicking.

use sql_dialect_fmt_lexer::SyntaxKind::*;
use sql_dialect_fmt_lexer::{tokenize, tokenize_with_options, BodyDelimiter, LexOptions};
use sql_dialect_fmt_syntax::keyword_kind;
use sql_dialect_fmt_test_support::lexer::{
    assert_lex_lossless as assert_lossless, assert_lexed_lossless, lex_non_trivia as lex_nt,
    lex_pairs as lex,
};

// ---- helpers ----------------------------------------------------------------

fn n_errors(input: &str) -> usize {
    tokenize(input).errors.len()
}

// ---- property / fuzz tests --------------------------------------------------

/// Tiny deterministic xorshift RNG — reproducible fuzzing with zero dependencies.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

#[test]
fn fuzz_fragments_preserve_invariants() {
    // A vocabulary of "tricky" fragments: every delimiter, escape, and operator that
    // could interact badly when juxtaposed, plus multibyte unicode.
    const FRAGMENTS: &[&str] = &[
        " ", "\t", "\n", "\r\n", "\r", "select", "FROM", "x", "_a", "a$b", "tbl$$x", "0", "1",
        "12.5", ".5", "100.", "1e10", "1e", "1e+5", "'", "''", "'s'", "\\", "\"", "\"q\"", "$$",
        "$", "$1", "$a", "::", ":", ":=", "|", "||", "|>", "->", "->>", "=>", "=", "<", "<=", "<>",
        ">", ">=", "!", "!=", "(", ")", "[", "]", "{", "}", ",", ".", ";", "@", "?", "+", "-", "*",
        "/", "%", "&", "^", "~", "--c", "//c", "/*", "*/", "/*c*/", "café", "中文", "💥", "`", "#",
    ];
    let mut rng = Rng::new(0xDEAD_BEEF_CAFE_1234);
    for _ in 0..6000 {
        let n = rng.below(40) + 1;
        let mut s = String::new();
        for _ in 0..n {
            s.push_str(FRAGMENTS[rng.below(FRAGMENTS.len())]);
        }
        assert_lossless(&s);
    }
}

#[test]
fn fuzz_random_chars_preserve_invariants() {
    const POOL: &[char] = &[
        ' ', '\t', '\n', '\r', 'a', 'b', 'Z', '_', '0', '9', '\'', '"', '\\', '$', ':', '|', '<',
        '>', '=', '!', '-', '/', '*', '.', ',', ';', '(', ')', '[', ']', '{', '}', '@', '?', '+',
        '%', '&', '^', '~', '`', '#', 'é', '中', '💥', 'Ω',
    ];
    let mut rng = Rng::new(0x0123_4567_89AB_CDEF);
    for _ in 0..4000 {
        let n = rng.below(60);
        let mut s = String::new();
        for _ in 0..n {
            s.push(POOL[rng.below(POOL.len())]);
        }
        assert_lossless(&s);
    }
}

// ---- empty / whitespace / newlines -----------------------------------------

#[test]
fn empty_input() {
    assert!(tokenize("").tokens.is_empty());
    assert!(tokenize("").errors.is_empty());
}

#[test]
fn whitespace_runs_collapse_into_one_token() {
    assert_eq!(lex("   \t  "), vec![(WHITESPACE, "   \t  ")]);
    assert_eq!(lex("  \n"), vec![(WHITESPACE, "  "), (NEWLINE, "\n")]);
}

#[test]
fn newlines_lf_crlf_cr() {
    assert_eq!(
        lex("a\r\nb"),
        vec![(IDENT, "a"), (NEWLINE, "\r\n"), (IDENT, "b")]
    );
    assert_eq!(
        lex("a\rb"),
        vec![(IDENT, "a"), (NEWLINE, "\r"), (IDENT, "b")]
    );
    assert_eq!(lex("\n\n"), vec![(NEWLINE, "\n"), (NEWLINE, "\n")]);
    assert_eq!(
        lex("x\n\ny"),
        vec![(IDENT, "x"), (NEWLINE, "\n"), (NEWLINE, "\n"), (IDENT, "y"),]
    );
}

// ---- operators & maximal munch ---------------------------------------------

#[test]
fn single_char_punctuation() {
    let cases = [
        ("(", L_PAREN),
        (")", R_PAREN),
        ("[", L_BRACKET),
        ("]", R_BRACKET),
        ("{", L_BRACE),
        ("}", R_BRACE),
        (",", COMMA),
        (".", DOT),
        (";", SEMICOLON),
        ("@", AT),
        ("?", QUESTION),
        ("+", PLUS),
        ("-", MINUS),
        ("*", STAR),
        ("/", SLASH),
        ("%", PERCENT),
        ("&", AMP),
        ("^", CARET),
        ("~", TILDE),
        ("=", EQ),
        ("<", LT),
        (">", GT),
        ("|", PIPE),
        (":", COLON),
        ("$", DOLLAR),
    ];
    for (src, kind) in cases {
        assert_eq!(lex_nt(src), vec![(kind, src)], "for {src:?}");
        assert_eq!(n_errors(src), 0, "for {src:?}");
    }
}

#[test]
fn maximal_munch_boundaries() {
    assert_eq!(lex_nt("||"), vec![(CONCAT, "||")]);
    assert_eq!(lex_nt("|>"), vec![(PIPE_GT, "|>")]);
    assert_eq!(lex_nt("|"), vec![(PIPE, "|")]);
    assert_eq!(lex_nt("|||"), vec![(CONCAT, "||"), (PIPE, "|")]);
    assert_eq!(lex_nt("::"), vec![(COLON2, "::")]);
    assert_eq!(lex_nt(":::"), vec![(COLON2, "::"), (COLON, ":")]);
    assert_eq!(lex_nt(":="), vec![(ASSIGN, ":=")]);
    assert_eq!(lex_nt("<="), vec![(LTE, "<=")]);
    assert_eq!(lex_nt("<>"), vec![(NEQ, "<>")]);
    assert_eq!(lex_nt("!="), vec![(NEQ, "!=")]);
    assert_eq!(lex_nt(">="), vec![(GTE, ">=")]);
    assert_eq!(lex_nt("=>"), vec![(FAT_ARROW, "=>")]);
    assert_eq!(lex_nt("->>"), vec![(FLOW_PIPE, "->>")]);
    assert_eq!(lex_nt("->"), vec![(ARROW, "->")]);
    assert_eq!(lex_nt("->>>"), vec![(FLOW_PIPE, "->>"), (GT, ">")]);
    // SQL has no `==`; it lexes as two separate `=` tokens.
    assert_eq!(lex_nt("=="), vec![(EQ, "="), (EQ, "=")]);
    // Comment vs operator disambiguation.
    assert_eq!(lex("--x"), vec![(COMMENT, "--x")]);
    assert!(lex_nt("--x").is_empty());
    assert_eq!(lex_nt("-x"), vec![(MINUS, "-"), (IDENT, "x")]);
    assert_eq!(lex_nt("/x"), vec![(SLASH, "/"), (IDENT, "x")]);
}

#[test]
fn full_operator_run() {
    assert_eq!(
        lex_nt("x::int || y => z ->> q -> w := v <> u != t <= s >= r"),
        vec![
            (IDENT, "x"),
            (COLON2, "::"),
            (IDENT, "int"),
            (CONCAT, "||"),
            (IDENT, "y"),
            (FAT_ARROW, "=>"),
            (IDENT, "z"),
            (FLOW_PIPE, "->>"),
            (IDENT, "q"),
            (ARROW, "->"),
            (IDENT, "w"),
            (ASSIGN, ":="),
            (IDENT, "v"),
            (NEQ, "<>"),
            (IDENT, "u"),
            (NEQ, "!="),
            (IDENT, "t"),
            (LTE, "<="),
            (IDENT, "s"),
            (GTE, ">="),
            (IDENT, "r"),
        ]
    );
}

// ---- Snowflake-specific constructs -----------------------------------------

#[test]
fn pipe_chain() {
    let sql = "SHOW TABLES\n->> SELECT \"name\" FROM $1\n->> SELECT count(*) FROM $1";
    let lexed = tokenize(sql);
    assert!(lexed.errors.is_empty());
    assert_eq!(
        lexed.tokens.iter().filter(|t| t.kind == FLOW_PIPE).count(),
        2,
        "expected two Snowflake flow pipe operators"
    );
    assert_lossless(sql);
}

#[test]
fn legacy_pipe_gt_stays_lossless() {
    let sql = "FROM orders\n|> WHERE amount > 100\n|> ORDER BY amount DESC";
    let lexed = tokenize(sql);
    assert!(lexed.errors.is_empty());
    assert_eq!(
        lexed.tokens.iter().filter(|t| t.kind == PIPE_GT).count(),
        2,
        "expected two compatibility pipe operators"
    );
    assert_lossless(sql);
}

#[test]
fn semi_structured_access() {
    assert_eq!(
        lex_nt("payload:user.name::string"),
        vec![
            (IDENT, "payload"),
            (COLON, ":"),
            (IDENT, "user"),
            (DOT, "."),
            (IDENT, "name"),
            (COLON2, "::"),
            (IDENT, "string"),
        ]
    );
}

#[test]
fn array_indexing() {
    assert_eq!(
        lex_nt("c[0]['k']"),
        vec![
            (IDENT, "c"),
            (L_BRACKET, "["),
            (INT_NUMBER, "0"),
            (R_BRACKET, "]"),
            (L_BRACKET, "["),
            (STRING, "'k'"),
            (R_BRACKET, "]"),
        ]
    );
}

#[test]
fn named_args_and_lambda() {
    assert_eq!(
        lex_nt("f(a => 1, x -> x + 1)"),
        vec![
            (IDENT, "f"),
            (L_PAREN, "("),
            (IDENT, "a"),
            (FAT_ARROW, "=>"),
            (INT_NUMBER, "1"),
            (COMMA, ","),
            (IDENT, "x"),
            (ARROW, "->"),
            (IDENT, "x"),
            (PLUS, "+"),
            (INT_NUMBER, "1"),
            (R_PAREN, ")"),
        ]
    );
}

#[test]
fn qualified_names_with_quoted_part() {
    assert_eq!(
        lex_nt(r#"db.schema."Tbl".col"#),
        vec![
            (IDENT, "db"),
            (DOT, "."),
            (IDENT, "schema"),
            (DOT, "."),
            (QUOTED_IDENT, r#""Tbl""#),
            (DOT, "."),
            (IDENT, "col"),
        ]
    );
}

#[test]
fn stage_reference() {
    assert_eq!(lex_nt("@my_stage"), vec![(AT, "@"), (IDENT, "my_stage")]);
}

// ---- literals: strings, quoted idents, dollar quotes, variables ------------

#[test]
fn string_literals() {
    assert_eq!(lex("'a''b'"), vec![(STRING, "'a''b'")]); // doubled-quote escape
    assert_eq!(lex("''"), vec![(STRING, "''")]); // empty string, complete
    assert_eq!(lex("'a\\'b'"), vec![(STRING, "'a\\'b'")]); // backslash-escaped quote
    assert_eq!(lex("'café'"), vec![(STRING, "'café'")]); // unicode body
    let with_newline = "'line1\nline2'";
    assert_eq!(lex(with_newline), vec![(STRING, with_newline)]); // newline inside string
}

#[test]
fn quoted_identifiers() {
    assert_eq!(lex(r#""Tbl""#), vec![(QUOTED_IDENT, r#""Tbl""#)]);
    assert_eq!(lex(r#""a""b""#), vec![(QUOTED_IDENT, r#""a""b""#)]); // doubled-quote escape
    assert_eq!(
        lex(r#""with space""#),
        vec![(QUOTED_IDENT, r#""with space""#)]
    );
    // A keyword inside quotes is just a quoted identifier.
    assert_eq!(lex(r#""select""#), vec![(QUOTED_IDENT, r#""select""#)]);
}

#[test]
fn dollar_quoted_bodies() {
    assert_eq!(lex("$$$$"), vec![(DOLLAR_STRING, "$$$$")]); // empty body
    assert_eq!(lex("$$a $ b$$"), vec![(DOLLAR_STRING, "$$a $ b$$")]); // single $ inside is fine
    let body = "$$\nfor (i=0;i<n;i++) {}\n$$";
    assert_eq!(lex(body), vec![(DOLLAR_STRING, body)]); // embedded code with newlines
}

#[test]
fn body_delimiters_are_table_driven_for_future_snowflake_changes() {
    const FUTURE_DELIMITERS: &[BodyDelimiter] = &[
        BodyDelimiter::symmetric("dollar-quoted body", "$$"),
        BodyDelimiter::paired("tagged procedure body", "$proc$", "$proc$"),
    ];
    let sql = "AS $proc$\nBEGIN\n  RETURN '長芋';\nEND;\n$proc$;";
    let lexed = tokenize_with_options(
        sql,
        LexOptions {
            body_delimiters: FUTURE_DELIMITERS,
            ..Default::default()
        },
    );

    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    assert_eq!(
        lexed
            .tokens
            .iter()
            .filter(|token| token.kind == DOLLAR_STRING)
            .map(|token| token.text)
            .collect::<Vec<_>>(),
        vec!["$proc$\nBEGIN\n  RETURN '長芋';\nEND;\n$proc$"]
    );
    assert_lexed_lossless(sql, &lexed);
}

#[test]
fn longest_body_delimiter_opener_wins() {
    const OVERLAPPING_DELIMITERS: &[BodyDelimiter] = &[
        BodyDelimiter::symmetric("dollar-quoted body", "$$"),
        BodyDelimiter::symmetric("long tagged body", "$$proc$$"),
    ];
    let sql = "$$proc$$BEGIN\n  RETURN $$;\nEND;$$proc$$";
    let lexed = tokenize_with_options(
        sql,
        LexOptions {
            body_delimiters: OVERLAPPING_DELIMITERS,
            ..Default::default()
        },
    );

    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    assert_eq!(lexed.tokens.len(), 1);
    assert_eq!(lexed.tokens[0].kind, DOLLAR_STRING);
    assert_eq!(lexed.tokens[0].text, sql);
    assert_lexed_lossless(sql, &lexed);
}

#[test]
fn dollar_quote_only_recognized_at_token_start() {
    // When a token *starts* at `$$`, it is a dollar-quoted body.
    assert_eq!(lex("$$y$$"), vec![(DOLLAR_STRING, "$$y$$")]);
    // But `$` is a legal identifier character, so `$$` glued to an identifier is consumed
    // as part of that identifier. Real SQL always separates `AS` and `$$` with whitespace.
    assert_eq!(lex("x$$y$$"), vec![(IDENT, "x$$y$$")]);
}

#[test]
fn variables_and_lone_dollar() {
    assert_eq!(
        lex_nt("$1 $2 $foo"),
        vec![(VARIABLE, "$1"), (VARIABLE, "$2"), (VARIABLE, "$foo")]
    );
    assert_eq!(
        lex_nt("$proc$"),
        vec![(VARIABLE, "$proc$")],
        "tagged delimiters are opt-in until Snowflake supports them"
    );
    assert_eq!(lex("$"), vec![(DOLLAR, "$")]);
    assert_eq!(lex("$ "), vec![(DOLLAR, "$"), (WHITESPACE, " ")]);
}

#[test]
fn identifiers_may_contain_dollar() {
    assert_eq!(lex("a$b"), vec![(IDENT, "a$b")]);
    assert_eq!(lex("_x$1"), vec![(IDENT, "_x$1")]);
}

// ---- numbers ----------------------------------------------------------------

#[test]
fn number_forms() {
    assert_eq!(
        lex_nt("0 1 42 12.5 .5 100. 1e10 1E10 1e+5 1e-5 1.5e-3"),
        vec![
            (INT_NUMBER, "0"),
            (INT_NUMBER, "1"),
            (INT_NUMBER, "42"),
            (FLOAT_NUMBER, "12.5"),
            (FLOAT_NUMBER, ".5"),
            (FLOAT_NUMBER, "100."),
            (FLOAT_NUMBER, "1e10"),
            (FLOAT_NUMBER, "1E10"),
            (FLOAT_NUMBER, "1e+5"),
            (FLOAT_NUMBER, "1e-5"),
            (FLOAT_NUMBER, "1.5e-3"),
        ]
    );
}

#[test]
fn number_edge_cases() {
    // `1e` has no exponent digits → INT `1` followed by IDENT `e` (the lexer backtracks).
    assert_eq!(lex_nt("1e"), vec![(INT_NUMBER, "1"), (IDENT, "e")]);
    // `1e+` backtracks past the `+` too.
    assert_eq!(
        lex_nt("1e+"),
        vec![(INT_NUMBER, "1"), (IDENT, "e"), (PLUS, "+")]
    );
    // `1..2` documents the current (lossless) split.
    assert_eq!(
        lex_nt("1..2"),
        vec![(FLOAT_NUMBER, "1."), (FLOAT_NUMBER, ".2")]
    );
}

// ---- comments ---------------------------------------------------------------

#[test]
fn comments() {
    assert_eq!(
        lex("-- hi\nx"),
        vec![(COMMENT, "-- hi"), (NEWLINE, "\n"), (IDENT, "x")]
    );
    assert_eq!(lex("// hi"), vec![(COMMENT, "// hi")]);
    assert_eq!(lex("/* a */"), vec![(BLOCK_COMMENT, "/* a */")]);
    assert_eq!(
        lex("/* a * b / c */"),
        vec![(BLOCK_COMMENT, "/* a * b / c */")]
    );
    let multiline = "/* a\nb */";
    assert_eq!(lex(multiline), vec![(BLOCK_COMMENT, multiline)]);
}

#[test]
fn block_comments_do_not_nest() {
    let s = "/* outer /* inner */ tail */";
    let toks = lex(s);
    assert_eq!(toks[0], (BLOCK_COMMENT, "/* outer /* inner */"));
    // After the (first) `*/`, the rest is ordinary tokens.
    assert_eq!(lex_nt(s), vec![(IDENT, "tail"), (STAR, "*"), (SLASH, "/")]);
}

// ---- realistic snippets -----------------------------------------------------

#[test]
fn realistic_javascript_procedure() {
    let sql = "CREATE OR REPLACE FUNCTION add(a INT, b INT)\nRETURNS INT\nLANGUAGE JAVASCRIPT\nAS $$\n  return A + B;\n$$;";
    let lexed = tokenize(sql);
    assert!(lexed.errors.is_empty(), "errors: {:?}", lexed.errors);
    assert_lossless(sql);
    let bodies: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|t| t.kind == DOLLAR_STRING)
        .collect();
    assert_eq!(bodies.len(), 1);
    assert!(bodies[0].text.contains("return A + B;"));
}

#[test]
fn valid_inputs_produce_no_errors() {
    let corpus = [
        "SELECT 1",
        "SELECT a, b, c FROM t WHERE a > 1 AND b < 2",
        "WITH cte AS (SELECT * FROM t) SELECT * FROM cte",
        "SELECT col:field::string FROM raw",
        "SHOW TABLES ->> SELECT \"name\" FROM $1",
        "FROM t |> WHERE x > 0 |> SELECT x",
        "CREATE OR REPLACE PROCEDURE p() RETURNS STRING LANGUAGE SQL AS $$ BEGIN RETURN 'ok'; END; $$",
        "SELECT $1, $2 FROM @my_stage",
        "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.v = s.v",
        "SELECT * FROM t QUALIFY row_number() OVER (PARTITION BY a ORDER BY b) = 1",
        "-- comment\n/* block */ SELECT 1 // trailing\n",
    ];
    for s in corpus {
        let lexed = tokenize(s);
        assert!(
            lexed.errors.is_empty(),
            "unexpected errors for {s:?}: {:?}",
            lexed.errors
        );
        assert_lossless(s);
    }
}

// ---- error handling ---------------------------------------------------------

#[test]
fn unterminated_string_reports_offset() {
    let lexed = tokenize("ab 'x");
    assert_eq!(lexed.errors.len(), 1);
    assert_eq!(
        lexed.errors[0].offset, 3,
        "offset should point at the opening quote"
    );
    assert!(lexed.errors[0].message.contains("string"));
    assert_lossless("ab 'x");
}

#[test]
fn unterminated_others() {
    assert_eq!(n_errors("/* x"), 1);
    assert!(tokenize("/* x").errors[0].message.contains("block comment"));
    assert_eq!(n_errors("$$ x"), 1);
    assert_eq!(n_errors("\"x"), 1);
}

#[test]
fn lex_error_span_covers_the_offending_token() {
    // An unterminated string runs from its opening quote to end of input: the error span must
    // cover that whole token, not a single character.
    let src = "ab 'unterminated";
    let lexed = tokenize(src);
    let err = &lexed.errors[0];
    let start = src.find('\'').unwrap();
    assert_eq!(err.offset, start);
    assert_eq!(err.len, src.len() - start);
    assert_eq!(&src[err.range()], "'unterminated");

    // A stray multi-byte character spans its full UTF-8 width (and stays on a char boundary).
    let lexed = tokenize("€");
    let err = &lexed.errors[0];
    assert_eq!(err.offset, 0);
    assert_eq!(err.len, "€".len());
    assert_eq!(&"€"[err.range()], "€");
}

#[test]
fn lex_error_displays_human_message_with_location() {
    let lexed = tokenize("ab\n'x");
    let err = &lexed.errors[0];
    let shown = err.to_string();
    assert!(shown.contains("unterminated string literal"), "{shown}");
    assert!(shown.contains("(byte 3)"), "{shown}");
    assert!(shown.contains("at line 2, column 1"), "{shown}");
    let _: &dyn std::error::Error = err;
}

#[test]
fn stray_characters_are_errors_not_panics() {
    assert_eq!(lex("!"), vec![(BANG, "!")]);
    assert_eq!(n_errors("!"), 1);
    assert_eq!(lex("`"), vec![(ERROR, "`")]);
    assert_eq!(n_errors("`"), 1);
    assert_eq!(lex("#"), vec![(ERROR, "#")]);
    assert_eq!(n_errors("#"), 1);
    assert_eq!(lex("€"), vec![(ERROR, "€")]);
    assert_eq!(n_errors("€"), 1);
}

// ---- cross-crate: lexer IDENT + syntax keyword_kind -------------------------

#[test]
fn keyword_reclassification_round_trips() {
    let lexed = tokenize("select From wHere qualify notakeyword");
    let mapped: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|t| t.kind == IDENT)
        .map(|t| keyword_kind(t.text))
        .collect();
    assert_eq!(
        mapped,
        vec![
            Some(SELECT_KW),
            Some(FROM_KW),
            Some(WHERE_KW),
            Some(QUALIFY_KW),
            None,
        ]
    );
}
