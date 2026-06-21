use snow_fmt_test_fixtures::{EASY_CASES, MINIMUM_EMBEDDED_EASY_CASES};
use tree_sitter::{InputEdit, Point, Query, QueryCursor, StreamingIterator};

struct SqlCase {
    name: &'static str,
    sql: &'static str,
}

struct CaptureCase {
    name: &'static str,
    sql: &'static str,
    expected: &'static [(&'static str, &'static str)],
}

const PARSE_CASES: &[SqlCase] = &[
    SqlCase {
        name: "flow pipe and semistructured access",
        sql: r#"WITH rollup AS (
  SELECT payload:customer.id::VARIANT AS customer_id,
         ARRAY_CONSTRUCT(1, 2, 3) AS values
  FROM @~/stage/path
  WHERE payload:name ILIKE '長芋%'
)
SELECT * FROM rollup
->> SELECT customer_id FROM $1 WHERE values[0] >= 1;
"#,
    },
    SqlCase {
        name: "legacy pipe token kept for compatibility",
        sql: "SELECT * FROM events |> WHERE payload:type = 'click' |> LIMIT 10;",
    },
    SqlCase {
        name: "copy into stage with file format options",
        sql: r#"COPY INTO @~/exports
FROM (SELECT id, payload FROM raw.events)
FILE_FORMAT = (TYPE = CSV FIELD_OPTIONALLY_ENCLOSED_BY = '"');
"#,
    },
    SqlCase {
        name: "merge with update and insert arms",
        sql: r#"MERGE INTO dst USING src ON dst.id = src.id
WHEN MATCHED THEN UPDATE SET v = src.v
WHEN NOT MATCHED THEN INSERT (id, v) VALUES (src.id, src.v);
"#,
    },
    SqlCase {
        name: "task calls procedure",
        sql: "CREATE TASK refresh_task WAREHOUSE = wh SCHEDULE = '1 MINUTE' AS CALL refresh_proc();",
    },
    SqlCase {
        name: "dynamic table",
        sql: "CREATE DYNAMIC TABLE dt TARGET_LAG = '1 minute' WAREHOUSE = wh AS SELECT * FROM src;",
    },
    SqlCase {
        name: "snowflake scripting block",
        sql: r#"DECLARE x NUMBER;
BEGIN
  LET x := 1;
  RETURN x;
EXCEPTION
  WHEN OTHER THEN RETURN NULL;
END;
"#,
    },
    SqlCase {
        name: "embedded javascript body",
        sql: r#"CREATE PROCEDURE p()
RETURNS STRING
LANGUAGE JAVASCRIPT
AS $$
return "ok";
$$;
"#,
    },
    SqlCase {
        name: "match recognize surface",
        sql: r#"SELECT * FROM events
MATCH_RECOGNIZE (
  PARTITION BY user_id
  ORDER BY ts
  PATTERN (start_event follow_event*)
  DEFINE follow_event AS event_type = 'follow'
);
"#,
    },
    SqlCase {
        name: "asof join",
        sql: r#"SELECT * FROM quotes ASOF JOIN trades
MATCH_CONDITION(quotes.ts >= trades.ts)
ON quotes.symbol = trades.symbol;
"#,
    },
    SqlCase {
        name: "structured and vector types",
        sql: "CREATE TABLE typed (v VECTOR(FLOAT, 3), o OBJECT(city VARCHAR), m MAP(VARCHAR, NUMBER));",
    },
    SqlCase {
        name: "quoted identifiers and unicode string",
        sql: r#"SELECT "顧客"."名前", '長芋' FROM "schema"."table";"#,
    },
    SqlCase {
        name: "comments and mixed operators",
        sql: "/* block */\nSELECT a => b, c -> d, e::NUMBER -- line\nFROM t // slash line\n;",
    },
];

const LINE_ENDING_CASES: &[SqlCase] = &[
    SqlCase {
        name: "lf",
        sql: "SELECT 1;\nSELECT 2;\n",
    },
    SqlCase {
        name: "crlf",
        sql: "SELECT 1;\r\nSELECT 2;\r\n",
    },
    SqlCase {
        name: "cr",
        sql: "SELECT 1;\rSELECT 2;\r",
    },
    SqlCase {
        name: "mixed",
        sql: "SELECT 1;\r\n-- mixed\rSELECT 2;\n",
    },
];

const CAPTURE_CASES: &[CaptureCase] = &[
    CaptureCase {
        name: "core snowflake tokens",
        sql: "SELECT $1::NUMBER FROM @~/stage/path ->> SELECT '長芋' FROM $1;",
        expected: &[
            ("keyword", "SELECT"),
            ("variable.parameter", "$1"),
            ("operator", "::"),
            ("type", "NUMBER"),
            ("string.special", "@~/stage/path"),
            ("operator", "->>"),
            ("string", "'長芋'"),
        ],
    },
    CaptureCase {
        name: "comments quoted identifiers and compatibility pipe",
        sql: "/* ok */ SELECT \"name\" FROM t |> WHERE \"name\" ILIKE 'a%';",
        expected: &[
            ("comment", "/* ok */"),
            ("keyword", "SELECT"),
            ("variable.member", "\"name\""),
            ("operator", "|>"),
            ("keyword", "ILIKE"),
            ("string", "'a%'"),
        ],
    },
    CaptureCase {
        name: "embedded dollar body",
        sql: "CREATE PROCEDURE p() LANGUAGE JAVASCRIPT AS $$return \"ok\";$$;",
        expected: &[
            ("keyword", "CREATE"),
            ("keyword", "JAVASCRIPT"),
            ("string.special", "$$return \"ok\";$$"),
        ],
    },
];

fn parse(source: &str) -> tree_sitter::Tree {
    let mut parser = snow_fmt_tree_sitter::parser().expect("load Snowflake Tree-sitter grammar");
    parser.parse(source, None).expect("parse source")
}

fn assert_parse_ok(case: &SqlCase) {
    assert_parse_ok_named(case.name, case.sql);
}

fn assert_parse_ok_named(name: &str, sql: &str) {
    let tree = parse(sql);
    let root = tree.root_node();
    assert_eq!(root.kind(), "source_file", "{name} root kind");
    assert_eq!(
        root.end_byte(),
        sql.len(),
        "{name} did not consume the full input"
    );
    assert!(
        !root.has_error(),
        "{name} contains Tree-sitter errors: {}",
        root.to_sexp()
    );
}

fn highlight_captures(sql: &str) -> Vec<(String, String)> {
    let language = snow_fmt_tree_sitter::language();
    let query = Query::new(&language, snow_fmt_tree_sitter::HIGHLIGHTS_QUERY)
        .expect("highlight query compiles");
    let tree = parse(sql);
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, tree.root_node(), sql.as_bytes());
    let capture_names = query.capture_names();
    let mut seen = Vec::new();

    captures.advance();
    while let Some((query_match, capture_index)) = captures.get() {
        let capture = query_match.captures[*capture_index];
        let name = capture_names[capture.index as usize].to_string();
        let text = capture
            .node
            .utf8_text(sql.as_bytes())
            .expect("capture text is valid UTF-8")
            .to_string();
        seen.push((name, text));
        captures.advance();
    }

    seen
}

#[test]
fn grammar_metadata_is_available() {
    let language = snow_fmt_tree_sitter::language();

    assert_eq!(language.name(), Some("snowflake"));
    assert!(language.node_kind_count() > 0);
    assert!(snow_fmt_tree_sitter::NODE_TYPES.contains("\"source_file\""));
}

#[test]
fn highlight_and_support_queries_compile() {
    let language = snow_fmt_tree_sitter::language();
    let highlights = Query::new(&language, snow_fmt_tree_sitter::HIGHLIGHTS_QUERY)
        .expect("highlight query compiles");
    Query::new(&language, snow_fmt_tree_sitter::LOCALS_QUERY).expect("locals query compiles");
    Query::new(&language, snow_fmt_tree_sitter::INJECTIONS_QUERY)
        .expect("injections query compiles");

    let names = highlights.capture_names();
    for required in [
        "keyword",
        "type",
        "string",
        "string.special",
        "operator",
        "punctuation.delimiter",
        "comment",
        "variable.member",
        "variable.parameter",
    ] {
        assert!(
            names.contains(&required),
            "missing highlight capture {required}"
        );
    }
}

#[test]
fn javascript_udf_body_is_injected_as_javascript() {
    // The injection query correlates the LANGUAGE clause with the `$$ … $$` body so editors
    // highlight it with the right grammar.
    let language = snow_fmt_tree_sitter::language();
    let query = Query::new(&language, snow_fmt_tree_sitter::INJECTIONS_QUERY)
        .expect("injections query compiles");
    let sql = "CREATE PROCEDURE p() LANGUAGE JAVASCRIPT AS $$return 1;$$;";
    let tree = parse(sql);
    let names = query.capture_names();

    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, tree.root_node(), sql.as_bytes());
    let mut injected_language = None;
    let mut injected_content = None;
    captures.advance();
    while let Some((query_match, capture_index)) = captures.get() {
        let capture = query_match.captures[*capture_index];
        let text = capture
            .node
            .utf8_text(sql.as_bytes())
            .expect("utf8")
            .to_string();
        match names[capture.index as usize] {
            "injection.language" => injected_language = Some(text),
            "injection.content" => injected_content = Some(text),
            _ => {}
        }
        captures.advance();
    }

    assert_eq!(injected_language.as_deref(), Some("JAVASCRIPT"));
    // The captured content node is the whole dollar string; the `#offset!` directive (applied by
    // the highlighting host) trims the `$$` delimiters.
    assert_eq!(injected_content.as_deref(), Some("$$return 1;$$"));
}

#[test]
fn non_javascript_body_is_not_injected_as_javascript() {
    let language = snow_fmt_tree_sitter::language();
    let query = Query::new(&language, snow_fmt_tree_sitter::INJECTIONS_QUERY)
        .expect("injections query compiles");
    // A statement with no LANGUAGE clause yields no injection (the query requires language_clause).
    let sql = "SELECT $$ raw $$;";
    let tree = parse(sql);
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, tree.root_node(), sql.as_bytes());
    captures.advance();
    assert!(
        captures.get().is_none(),
        "no injection without a LANGUAGE clause"
    );
}

#[test]
fn highlight_query_captures_expected_tokens() {
    for case in CAPTURE_CASES {
        let seen = highlight_captures(case.sql);
        for expected in case.expected {
            assert!(
                seen.iter()
                    .any(|(capture, text)| capture == expected.0 && text == expected.1),
                "{} missing highlight capture {expected:?}; saw {seen:?}",
                case.name
            );
        }
    }
}

#[test]
fn parses_curated_snowflake_constructs_without_errors() {
    for case in PARSE_CASES {
        assert_parse_ok(case);
    }
}

#[test]
fn parses_embedded_easy_fixtures() {
    assert!(EASY_CASES.len() >= MINIMUM_EMBEDDED_EASY_CASES);
    for case in EASY_CASES {
        for (profile, sql) in case.sqls() {
            assert_parse_ok_named(&format!("{} {profile}", case.name), sql);
        }
    }
}

#[test]
fn accepts_common_line_endings() {
    for case in LINE_ENDING_CASES {
        assert_parse_ok(case);
    }
}

#[test]
fn supports_incremental_reparse_for_editor_use() {
    let mut parser = snow_fmt_tree_sitter::parser().expect("load Snowflake Tree-sitter grammar");
    let original = "SELECT 1;\n";
    let edited = "SELECT 10;\n";
    let mut tree = parser.parse(original, None).expect("parse original");

    tree.edit(&InputEdit {
        start_byte: 8,
        old_end_byte: 8,
        new_end_byte: 9,
        start_position: Point::new(0, 8),
        old_end_position: Point::new(0, 8),
        new_end_position: Point::new(0, 9),
    });

    let updated = parser.parse(edited, Some(&tree)).expect("parse edited");
    let root = updated.root_node();
    assert_eq!(root.end_byte(), edited.len());
    assert!(
        !root.has_error(),
        "incremental parse failed: {}",
        root.to_sexp()
    );
}
