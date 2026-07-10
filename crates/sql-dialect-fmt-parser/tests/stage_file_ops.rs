//! Snowflake PUT / GET / LIST / REMOVE parser coverage.
//!
//! Client-file URIs and stage paths must parse losslessly into a dedicated statement, including
//! unquoted `file://` paths whose doubled slash would otherwise be consumed as a line comment.

use sql_dialect_fmt_parser::SyntaxKind;
use sql_dialect_fmt_test_support::parser::{assert_parse_clean, assert_parse_roundtrip};

const CLEAN: &[&str] = &[
    "PUT file:///tmp/data/mydata.csv @my_int_stage",
    "PUT FILE:///tmp/data/orders_*01.csv @db.schema.stage/incoming/ AUTO_COMPRESS = FALSE PARALLEL = 8 SOURCE_COMPRESSION = NONE OVERWRITE = TRUE",
    r"PUT file://C:\temp\data\mydata.csv @~ AUTO_COMPRESS=TRUE",
    r#"PUT 'file:///tmp/data/orders 001.csv' '@"my stage"' AUTO_COMPRESS = FALSE"#,
    "GET @%mytable file:///tmp/data/",
    "GET @~/myfiles FILE:///tmp/load/ PARALLEL=10 PATTERN='.*[.]csv'",
    r#"GET '@"my stage"/out/' 'file:///tmp/load data/' PATTERN = '.*[.]parquet'"#,
    "LIST @%mytable",
    "LIST @my_csv_stage/analysis/ PATTERN='.*data_0.*'",
    "REMOVE @mystage/path1/subpath2",
    "REMOVE @~/retry/ PATTERN = '.*[.]tmp$'",
];

const BROKEN: &[&str] = &[
    "PUT",
    "PUT @stage",
    "PUT file:///tmp/data/x.csv",
    "GET",
    "GET @stage",
    "LIST",
    "REMOVE PATTERN = '.*'",
];

#[test]
fn stage_file_operations_parse_cleanly_and_round_trip() {
    for sql in CLEAN {
        assert_parse_clean(sql);
    }
}

#[test]
fn malformed_stage_file_operations_recover_losslessly() {
    for sql in BROKEN {
        let parsed = assert_parse_roundtrip(sql);
        assert!(
            !parsed.errors().is_empty(),
            "expected a diagnostic for malformed statement {sql:?}"
        );
        assert!(
            parsed
                .syntax()
                .descendants()
                .any(|node| node.kind() == SyntaxKind::STAGE_FILE_STMT),
            "expected a STAGE_FILE_STMT for {sql:?}"
        );
    }
}

#[test]
fn stage_file_operations_expose_locations_options_and_uri_tokens() {
    let sql = "PUT file:///tmp/data/*.csv @db.schema.stage/incoming/ AUTO_COMPRESS = FALSE PARALLEL = 8 SOURCE_COMPRESSION = NONE OVERWRITE = TRUE";
    let parsed = assert_parse_clean(sql);
    let root = parsed.syntax();
    let statement = root
        .descendants()
        .find(|node| node.kind() == SyntaxKind::STAGE_FILE_STMT)
        .expect("a STAGE_FILE_STMT");

    assert_eq!(
        statement
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::COPY_LOCATION)
            .count(),
        2
    );
    assert!(statement
        .descendants()
        .any(|node| node.kind() == SyntaxKind::STAGE_REF));
    assert_eq!(
        statement
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::COPY_OPTION)
            .count(),
        4
    );
    assert!(statement
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == SyntaxKind::FILE_URI));
}

#[test]
fn command_words_remain_identifiers_outside_statement_position() {
    let sql = "SELECT put, get, list, remove FROM t";
    let parsed = assert_parse_clean(sql);
    for token in parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| {
            matches!(
                token.text().to_ascii_lowercase().as_str(),
                "put" | "get" | "list" | "remove"
            )
        })
    {
        assert_eq!(token.kind(), SyntaxKind::IDENT, "{}", token.text());
    }

    for sql in [
        "PUT file:///tmp/x @s",
        "GET @s file:///tmp/",
        "LIST @s",
        "REMOVE @s",
    ] {
        let parsed = assert_parse_clean(sql);
        let command = parsed
            .syntax()
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| !token.kind().is_trivia())
            .expect("a command token");
        assert_eq!(command.kind(), SyntaxKind::CONTEXTUAL_KEYWORD, "{sql}");
    }
}
