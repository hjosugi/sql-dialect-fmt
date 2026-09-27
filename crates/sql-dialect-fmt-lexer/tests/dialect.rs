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

/// Like [`non_trivia_for`], but keeps comments (only whitespace and newlines are dropped), so
/// comment handling can be asserted on directly.
fn significant_for(input: &str, dialect: Dialect) -> Vec<LexPair<'_>> {
    let lexed = tokenize_for_dialect(input, dialect);
    let joined: String = lexed.tokens.iter().map(|t| t.text).collect();
    assert_eq!(
        joined, input,
        "lex must round-trip for {input:?} @ {dialect:?}"
    );
    lexed
        .tokens
        .into_iter()
        .filter(|t| !matches!(t.kind, WHITESPACE | NEWLINE))
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

#[test]
fn mysql_double_quotes_are_strings_backticks_are_identifiers() {
    assert_eq!(
        non_trivia_for("\"hello\" `col`", Dialect::MySql),
        vec![(STRING, "\"hello\""), (QUOTED_IDENT, "`col`")]
    );
    let lexed = tokenize_for_dialect("\"hello\" `col`", Dialect::MySql);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
}

#[test]
fn mysql_hash_comment_at_variable_and_prefixed_string() {
    assert_eq!(
        significant_for("# note\n@user x'0A'", Dialect::MySql),
        vec![(COMMENT, "# note"), (VARIABLE, "@user"), (STRING, "x'0A'"),]
    );
    let lexed = tokenize_for_dialect("# note\n@user x'0A'", Dialect::MySql);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
}

#[test]
fn bigquery_backtick_identifier_hash_comment_and_raw_string() {
    assert_eq!(
        significant_for(
            "SELECT `proj.ds.t` # trailing\n, r'raw\\n'",
            Dialect::BigQuery
        ),
        vec![
            (IDENT, "SELECT"),
            (QUOTED_IDENT, "`proj.ds.t`"),
            (COMMENT, "# trailing"),
            (COMMA, ","),
            (STRING, "r'raw\\n'"),
        ]
    );
    let lexed = tokenize_for_dialect("SELECT `t` # c", Dialect::BigQuery);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
}

#[test]
fn transactsql_bracket_ident_hash_ident_at_variable_and_nested_comment() {
    assert_eq!(
        non_trivia_for("[my col] #temp @var", Dialect::TransactSql),
        vec![
            (QUOTED_IDENT, "[my col]"),
            (IDENT, "#temp"),
            (VARIABLE, "@var"),
        ]
    );
    // Nested block comments collapse to a single BLOCK_COMMENT token under T-SQL.
    let nested = tokenize_for_dialect("/* a /* b */ c */", Dialect::TransactSql);
    assert!(nested.errors.is_empty(), "{:?}", nested.errors);
    assert_eq!(
        significant_for("/* a /* b */ c */", Dialect::TransactSql),
        vec![(BLOCK_COMMENT, "/* a /* b */ c */")]
    );
    // A non-nesting dialect stops the block comment at the first `*/`.
    let flat = tokenize_for_dialect("/* a /* b */ c */", Dialect::Snowflake);
    assert_eq!(
        significant_for("/* a /* b */ c */", Dialect::Snowflake),
        vec![
            (BLOCK_COMMENT, "/* a /* b */"),
            (IDENT, "c"),
            (STAR, "*"),
            (SLASH, "/"),
        ]
    );
    assert!(flat.errors.is_empty(), "{:?}", flat.errors);
}

#[test]
fn oracle_double_quoted_identifier_and_colon_bind_variable() {
    assert_eq!(
        non_trivia_for("\"col\" :name n'x'", Dialect::PlSql),
        vec![
            (QUOTED_IDENT, "\"col\""),
            (VARIABLE, ":name"),
            (STRING, "n'x'"),
        ]
    );
    let lexed = tokenize_for_dialect("\"col\" :name n'x'", Dialect::PlSql);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    // `::` stays the cast operator, not a bind variable.
    assert_eq!(
        non_trivia_for("a::int", Dialect::PlSql),
        vec![(IDENT, "a"), (COLON2, "::"), (IDENT, "int")]
    );
}

#[test]
fn postgres_dollar_quoting_dollar_params_and_nested_comments() {
    assert_eq!(
        non_trivia_for("$$body$$ $1 e'a\\n'", Dialect::PostgreSql),
        vec![
            (DOLLAR_STRING, "$$body$$"),
            (VARIABLE, "$1"),
            (STRING, "e'a\\n'"),
        ]
    );
    let lexed = tokenize_for_dialect("$$body$$ $1", Dialect::PostgreSql);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    assert_eq!(
        significant_for("/* a /* b */ c */", Dialect::PostgreSql),
        vec![(BLOCK_COMMENT, "/* a /* b */ c */")]
    );
}

#[test]
fn sqlite_bracket_and_backtick_identifiers() {
    assert_eq!(
        non_trivia_for("[a b] `c d` x'0A'", Dialect::Sqlite),
        vec![
            (QUOTED_IDENT, "[a b]"),
            (QUOTED_IDENT, "`c d`"),
            (STRING, "x'0A'"),
        ]
    );
    let lexed = tokenize_for_dialect("[a b] `c d` x'0A'", Dialect::Sqlite);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
}

#[test]
fn every_dialect_lexes_a_mixed_sample_losslessly() {
    let sample =
        "SELECT \"a\", `b`, [c], 's', x'0A', @v, :n, $1, $$d$$, 1.5e3 /* c /* n */ */ # h\n-- l --";
    for dialect in Dialect::ALL {
        let lexed = tokenize_for_dialect(sample, *dialect);
        let joined: String = lexed.tokens.iter().map(|t| t.text).collect();
        assert_eq!(joined, sample, "lossless round-trip failed for {dialect:?}");
    }
}
