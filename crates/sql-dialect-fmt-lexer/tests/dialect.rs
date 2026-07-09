//! Dialect-divergent lexing: backtick identifiers are a Databricks-only quote form, and a backtick
//! under Snowflake keeps its current (error) behavior unchanged.

use sql_dialect_fmt_lexer::{tokenize, tokenize_for_dialect, Dialect, SyntaxKind::*};

type LexPair<'a> = (sql_dialect_fmt_lexer::SyntaxKind, &'a str);

fn non_trivia_for(input: &str, dialect: Dialect) -> Vec<LexPair<'_>> {
    let lexed = tokenize_for_dialect(input, dialect);
    // Lossless: the concatenation of token texts must equal the input in every dialect.
    let joined: String = lexed.tokens.iter().map(|t| t.text).collect();
    assert_eq!(
        joined, input,
        "lex must round-trip for {input:?} @ {dialect:?}"
    );
    lexed
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia())
        .map(|t| (t.kind, t.text))
        .collect()
}

#[test]
fn backtick_identifier_is_quoted_ident_under_databricks() {
    // A plain backtick-quoted identifier lexes to one QUOTED_IDENT token (backticks included).
    assert_eq!(
        non_trivia_for("`col`", Dialect::Databricks),
        vec![(QUOTED_IDENT, "`col`")]
    );
    let lexed = tokenize_for_dialect("`col`", Dialect::Databricks);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
}

#[test]
fn backtick_identifier_in_select_under_databricks() {
    assert_eq!(
        non_trivia_for("SELECT `a b`, `c` FROM `my tbl`", Dialect::Databricks),
        vec![
            (IDENT, "SELECT"),
            (QUOTED_IDENT, "`a b`"),
            (COMMA, ","),
            (QUOTED_IDENT, "`c`"),
            (IDENT, "FROM"),
            (QUOTED_IDENT, "`my tbl`"),
        ]
    );
}

#[test]
fn doubled_backtick_escape_under_databricks() {
    // `` `` `` is an escaped backtick, so the identifier does not close there.
    assert_eq!(
        non_trivia_for("`a``b`", Dialect::Databricks),
        vec![(QUOTED_IDENT, "`a``b`")]
    );
    // Two adjacent identifiers separated by whitespace still produce two tokens.
    assert_eq!(
        non_trivia_for("`x` `y`", Dialect::Databricks),
        vec![(QUOTED_IDENT, "`x`"), (QUOTED_IDENT, "`y`")]
    );
}

#[test]
fn unterminated_backtick_identifier_records_error_but_stays_lossless() {
    let lexed = tokenize_for_dialect("`oops", Dialect::Databricks);
    let joined: String = lexed.tokens.iter().map(|t| t.text).collect();
    assert_eq!(joined, "`oops");
    assert_eq!(lexed.tokens.len(), 1);
    assert_eq!(lexed.tokens[0].kind, QUOTED_IDENT);
    assert!(
        !lexed.errors.is_empty(),
        "should record an unterminated error"
    );
}

#[test]
fn snowflake_backtick_behavior_is_unchanged() {
    // Under Snowflake (the default), a backtick is NOT an identifier quote — it remains an
    // unexpected character producing an ERROR token, exactly as before this change. Both the
    // default `tokenize` and the explicit Snowflake dialect must agree.
    let expected = vec![(ERROR, "`"), (IDENT, "col"), (ERROR, "`")];
    assert_eq!(non_trivia_for("`col`", Dialect::Snowflake), expected);

    let default_pairs: Vec<LexPair<'_>> = tokenize("`col`")
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia())
        .map(|t| (t.kind, t.text))
        .collect();
    assert_eq!(default_pairs, expected);

    let lexed = tokenize("`col`");
    assert!(
        !lexed.errors.is_empty(),
        "Snowflake should still flag a backtick as an error"
    );
}

#[test]
fn databricks_null_safe_equality_is_one_operator() {
    let lexed = tokenize_for_dialect("a <=> b", Dialect::Databricks);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    assert_eq!(
        non_trivia_for("a <=> b", Dialect::Databricks),
        vec![(IDENT, "a"), (NULL_SAFE_EQ, "<=>"), (IDENT, "b")]
    );

    assert!(
        non_trivia_for("a <=> b", Dialect::Snowflake)
            .iter()
            .all(|(kind, _)| *kind != NULL_SAFE_EQ),
        "Snowflake must not tokenize <=> as a Databricks null-safe-equality operator"
    );
}

#[test]
fn databricks_prefixed_strings_are_single_string_tokens() {
    let lexed = tokenize_for_dialect("r'raw\\n' x'0A0B'", Dialect::Databricks);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    assert_eq!(
        non_trivia_for("r'raw\\n' x'0A0B'", Dialect::Databricks),
        vec![(STRING, "r'raw\\n'"), (STRING, "x'0A0B'")]
    );
}

#[test]
fn double_slash_comments_remain_snowflake_only() {
    let snowflake = tokenize_for_dialect("// comment\nselect 1", Dialect::Snowflake);
    assert!(
        snowflake
            .tokens
            .iter()
            .any(|token| token.kind == COMMENT && token.text == "// comment"),
        "Snowflake should produce a COMMENT token for // line comments"
    );

    assert!(
        non_trivia_for("// comment\nselect 1", Dialect::Databricks)
            .iter()
            .all(|(kind, _)| *kind != COMMENT),
        "Databricks must not treat // as a line comment"
    );
}
