use sql_dialect_fmt_highlight::HighlightKind;
use sql_dialect_fmt_test_support::highlight::{
    assert_highlight_lossless, assert_interesting_highlights, interesting_highlights,
};

#[test]
fn classifies_snowflake_specific_tokens() {
    let sql = r#"SELECT payload:customer.id::VARIANT, $1, "Mixed Case"
FROM @~/stage/path
WHERE name ILIKE '長芋%' -- unicode value
->> SELECT count(*) FROM $1"#;

    assert_interesting_highlights(
        sql,
        &[
            ("SELECT", HighlightKind::Keyword),
            ("payload", HighlightKind::Identifier),
            (":", HighlightKind::Operator),
            ("customer", HighlightKind::Identifier),
            (".", HighlightKind::Punctuation),
            ("id", HighlightKind::Identifier),
            ("::", HighlightKind::Operator),
            ("VARIANT", HighlightKind::Type),
            (",", HighlightKind::Punctuation),
            ("$1", HighlightKind::Variable),
            (",", HighlightKind::Punctuation),
            ("\"Mixed Case\"", HighlightKind::QuotedIdentifier),
            ("FROM", HighlightKind::Keyword),
            ("@", HighlightKind::Operator),
            ("~", HighlightKind::Operator),
            ("/", HighlightKind::Operator),
            ("stage", HighlightKind::Identifier),
            ("/", HighlightKind::Operator),
            ("path", HighlightKind::Identifier),
            ("WHERE", HighlightKind::Keyword),
            ("name", HighlightKind::Identifier),
            ("ILIKE", HighlightKind::Keyword),
            ("'長芋%'", HighlightKind::String),
            ("-- unicode value", HighlightKind::Comment),
            ("->>", HighlightKind::Operator),
            ("SELECT", HighlightKind::Keyword),
            ("count", HighlightKind::Identifier),
            ("(", HighlightKind::Punctuation),
            ("*", HighlightKind::Operator),
            (")", HighlightKind::Punctuation),
            ("FROM", HighlightKind::Keyword),
            ("$1", HighlightKind::Variable),
        ],
    );
}

#[test]
fn scopes_are_stable_for_editor_adapters() {
    assert_eq!(HighlightKind::Keyword.scope(), "keyword.sql");
    assert_eq!(HighlightKind::Type.scope(), "support.type.sql");
    assert_eq!(
        HighlightKind::DollarString.scope(),
        "string.dollar-quoted.sql"
    );
    assert_eq!(HighlightKind::Error.scope(), "invalid.illegal.sql");
}

#[test]
fn classifies_procedure_and_task_keywords() {
    let sql = "CREATE TASK t WAREHOUSE = wh SCHEDULE = '5 MINUTES' AS CALL p(); \
CREATE PROCEDURE p() RETURNS STRING LANGUAGE SQL HANDLER = 'run';";
    let classified = interesting_highlights(sql);

    for keyword in [
        "CREATE",
        "TASK",
        "WAREHOUSE",
        "SCHEDULE",
        "AS",
        "CALL",
        "PROCEDURE",
        "RETURNS",
        "LANGUAGE",
        "SQL",
        "HANDLER",
    ] {
        assert!(
            classified.contains(&(keyword, HighlightKind::Keyword)),
            "{keyword} should be highlighted as a keyword; got {classified:?}"
        );
    }
    assert_highlight_lossless(sql);
}

#[test]
fn classifies_template_placeholders_as_parameters() {
    // SQL lifted out of a JavaScript template literal: each `${ ... }` is one placeholder token,
    // coloured like a parameter, with the surrounding SQL classified normally.
    assert_interesting_highlights(
        "SELECT ${cfg.col} FROM ${cfg.t} WHERE id = ${id}",
        &[
            ("SELECT", HighlightKind::Keyword),
            ("${cfg.col}", HighlightKind::Variable),
            ("FROM", HighlightKind::Keyword),
            ("${cfg.t}", HighlightKind::Variable),
            ("WHERE", HighlightKind::Keyword),
            ("id", HighlightKind::Identifier),
            ("=", HighlightKind::Operator),
            ("${id}", HighlightKind::Variable),
        ],
    );

    // Nested braces, a quoted `}`, and a nested template literal each stay inside one placeholder
    // token, so the trailing `AS c` keeps its ordinary SQL classification.
    let nested = interesting_highlights("SELECT ${ fn({a: 1}, '}') } AS c");
    assert_eq!(nested[0], ("SELECT", HighlightKind::Keyword));
    assert_eq!(nested[1], ("${ fn({a: 1}, '}') }", HighlightKind::Variable));
    assert_eq!(nested[2], ("AS", HighlightKind::Keyword));
    assert_eq!(nested[3], ("c", HighlightKind::Identifier));

    let template = interesting_highlights("SELECT ${ `col_${i}` }");
    assert_eq!(template[1], ("${ `col_${i}` }", HighlightKind::Variable));

    assert_highlight_lossless("SELECT ${ fn({a: 1}, '}') } AS c");
}

#[test]
fn long_highlight_input_keeps_byte_ranges() {
    let mut sql = String::new();
    for i in 0..256 {
        sql.push_str("SELECT ");
        sql.push_str(&i.to_string());
        sql.push_str("::NUMBER AS n");
        sql.push_str(&i.to_string());
        sql.push_str(" FROM table_");
        sql.push_str(&i.to_string());
        sql.push_str(" WHERE label = '長芋';\n");
    }

    assert_highlight_lossless(&sql);
}
