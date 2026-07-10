//! Snowflake PUT / GET / LIST / REMOVE formatting coverage.

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::{tokenize, SyntaxKind};
use sql_dialect_fmt_parser::parse;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

fn signature(sql: &str) -> Vec<String> {
    tokenize(sql)
        .tokens
        .into_iter()
        .filter(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::SEMICOLON)
        .map(|token| token.text.to_ascii_uppercase())
        .collect()
}

const CASES: &[&str] = &[
    "put file:///tmp/data/mydata.csv @my_int_stage",
    "put file:///tmp/data/orders_*01.csv @db.schema.stage/incoming/ auto_compress = false parallel = 8 source_compression = none overwrite = true",
    r"put file://C:\temp\data\mydata.csv @~ auto_compress=true",
    r#"put 'file:///tmp/data/orders 001.csv' '@"my stage"' auto_compress = false"#,
    "get @%mytable file:///tmp/data/",
    "get @~/myfiles file:///tmp/load/ parallel=10 pattern='.*[.]csv'",
    "list @my_csv_stage/analysis/ pattern='.*data_0.*'",
    "remove @~/retry/ pattern = '.*[.]tmp$'",
];

#[test]
fn all_cases_parse_cleanly_and_preserve_tokens() {
    for sql in CASES {
        let formatted = fmt(sql);
        assert!(parse(sql).errors().is_empty(), "{sql:?}");
        assert!(
            parse(&formatted).errors().is_empty(),
            "formatted output is invalid for {sql:?}:\n{formatted}"
        );
        assert_eq!(signature(sql), signature(&formatted), "{sql:?}");
    }
}

#[test]
fn formatting_is_idempotent() {
    for sql in CASES {
        let once = fmt(sql);
        assert_eq!(once, fmt(&once), "not idempotent:\n{sql}\n---\n{once}");
    }
}

#[test]
fn stage_file_operation_golden_layouts() {
    assert_eq!(
        fmt("put file:///tmp/data/*.csv @raw.stage/incoming auto_compress=false source_compression=none parallel=8 overwrite=true"),
        "PUT file:///tmp/data/*.csv @raw.stage/incoming AUTO_COMPRESS = FALSE SOURCE_COMPRESSION = none PARALLEL = 8 OVERWRITE = TRUE;\n"
    );
    assert_eq!(
        fmt("get @~/exports/ file:///tmp/load/ parallel=4 pattern='.*[.]csv'"),
        "GET @~/exports/ file:///tmp/load/ PARALLEL = 4 PATTERN = '.*[.]csv';\n"
    );
    assert_eq!(
        fmt("list @s/path pattern='.*[.]json'"),
        "LIST @s/path PATTERN = '.*[.]json';\n"
    );
    assert_eq!(fmt("remove @%t/tmp/"), "REMOVE @%t/tmp/;\n");
}

#[test]
fn command_words_stay_identifiers_outside_stage_operations() {
    assert_eq!(
        fmt("select put, get, list, remove from t"),
        "SELECT put, get, list, remove\nFROM t;\n"
    );
}
