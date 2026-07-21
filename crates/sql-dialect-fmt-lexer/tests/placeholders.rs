//! `${ ... }` template-substitution placeholders lex to one atomic token.
//!
//! SQL is frequently written inside a host-language string (a JavaScript template literal,
//! `${cfg.table}`) or run through a variable-substituting tool (Databricks / Spark / dbt). A
//! placeholder is kept as a single `PLACEHOLDER` token — braces, strings, and all — so the
//! surrounding statement still lexes losslessly. The body is balanced with a context stack, so
//! nested braces, a `}` inside a string, and a nested template literal never terminate it early.

use sql_dialect_fmt_lexer::{tokenize, tokenize_for_dialect, Dialect, SyntaxKind::*};

type LexPair<'a> = (sql_dialect_fmt_lexer::SyntaxKind, &'a str);

fn non_trivia(input: &str) -> Vec<LexPair<'_>> {
    let lexed = tokenize(input);
    let joined: String = lexed.tokens.iter().map(|t| t.text).collect();
    assert_eq!(joined, input, "lex must round-trip for {input:?}");
    lexed
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia())
        .map(|t| (t.kind, t.text))
        .collect()
}

#[test]
fn simple_placeholder_is_one_token() {
    assert_eq!(tokenize("${id}").errors, []);
    assert_eq!(non_trivia("${id}"), vec![(PLACEHOLDER, "${id}")]);
    assert_eq!(
        non_trivia("${cfg.table}"),
        vec![(PLACEHOLDER, "${cfg.table}")]
    );
    // A `:` inside the body (Databricks / Spark `${env:VAR}`) stays part of the placeholder.
    assert_eq!(
        non_trivia("${env:RUN_DATE}"),
        vec![(PLACEHOLDER, "${env:RUN_DATE}")]
    );
}

#[test]
fn placeholder_sits_between_ordinary_tokens() {
    assert_eq!(
        non_trivia("a=${x}"),
        vec![(IDENT, "a"), (EQ, "="), (PLACEHOLDER, "${x}")]
    );
    assert_eq!(
        non_trivia("${schema}.${table}"),
        vec![
            (PLACEHOLDER, "${schema}"),
            (DOT, "."),
            (PLACEHOLDER, "${table}"),
        ]
    );
}

#[test]
fn nested_braces_and_arrays_stay_in_one_token() {
    let src = "${ fn({a: 1, b: [2, 3]}) }";
    assert_eq!(tokenize(src).errors, []);
    assert_eq!(non_trivia(src), vec![(PLACEHOLDER, src)]);

    let deep = "${ obj.method({x: {y: {z: 1}}}) }";
    assert_eq!(tokenize(deep).errors, []);
    assert_eq!(non_trivia(deep), vec![(PLACEHOLDER, deep)]);
}

#[test]
fn brace_inside_string_does_not_terminate_placeholder() {
    // The `}` lives inside a single-quoted string, so the placeholder runs to the real closing `}`.
    let src = "${ a || '}' || b }";
    assert_eq!(tokenize(src).errors, []);
    assert_eq!(non_trivia(src), vec![(PLACEHOLDER, src)]);

    // A double-quoted string with an escaped quote and a brace, likewise opaque.
    let dq = "${ f(\"a}b\\\"c\") }";
    assert_eq!(tokenize(dq).errors, []);
    assert_eq!(non_trivia(dq), vec![(PLACEHOLDER, dq)]);
}

#[test]
fn nested_template_literal_is_consumed() {
    let src = "${ `col_${i}` }";
    assert_eq!(tokenize(src).errors, []);
    assert_eq!(non_trivia(src), vec![(PLACEHOLDER, src)]);

    let two = "${ `${prefix}_${suffix}` }";
    assert_eq!(tokenize(two).errors, []);
    assert_eq!(non_trivia(two), vec![(PLACEHOLDER, two)]);
}

#[test]
fn unterminated_placeholder_is_reported_but_lossless() {
    let lexed = tokenize("select ${ oops");
    // Lossless: the token text still concatenates back to the input.
    let joined: String = lexed.tokens.iter().map(|t| t.text).collect();
    assert_eq!(joined, "select ${ oops");
    // The trailing `${ oops` is captured as one placeholder token spanning to end of input.
    let last = lexed.tokens.last().unwrap();
    assert_eq!(last.kind, PLACEHOLDER);
    assert_eq!(last.text, "${ oops");
    assert!(
        lexed
            .errors
            .iter()
            .any(|e| e.message.contains("placeholder")),
        "expected an unterminated-placeholder diagnostic: {:?}",
        lexed.errors
    );
}

#[test]
fn placeholders_are_recognized_across_dialects() {
    for dialect in [Dialect::Snowflake, Dialect::Databricks] {
        let lexed = tokenize_for_dialect("${cfg.t}", dialect);
        let kinds: Vec<_> = lexed
            .tokens
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| (t.kind, t.text))
            .collect();
        assert_eq!(
            kinds,
            vec![(PLACEHOLDER, "${cfg.t}")],
            "placeholder must lex under {dialect:?}"
        );
        assert!(lexed.errors.is_empty(), "no errors under {dialect:?}");
    }
}

#[test]
fn dollar_forms_that_are_not_placeholders_are_unchanged() {
    // `$$...$$`, `$name`, and `$1` keep their existing kinds; only `${` opens a placeholder.
    assert_eq!(non_trivia("$$body$$"), vec![(DOLLAR_STRING, "$$body$$")]);
    assert_eq!(non_trivia("$name"), vec![(VARIABLE, "$name")]);
    assert_eq!(non_trivia("$1"), vec![(VARIABLE, "$1")]);
}
