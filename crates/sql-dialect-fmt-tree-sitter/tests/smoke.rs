use sql_dialect_fmt_syntax::{keyword_texts, BUILTIN_TYPE_WORDS};
use sql_dialect_fmt_test_fixtures::{EASY_CASES, MINIMUM_EMBEDDED_EASY_CASES};
use tree_sitter::{InputEdit, Point, Query, QueryCursor, StreamingIterator};

const GRAMMAR_JS: &str = include_str!("../../../tree-sitter-snowflake/grammar.js");

/// Every node kind the grammar can produce for a top-level statement: the coarse
/// statement families plus the lenient `statement` fallback.
const STATEMENT_NODE_KINDS: &[&str] = &[
    "select_statement",
    "insert_statement",
    "update_statement",
    "delete_statement",
    "merge_statement",
    "create_statement",
    "drop_statement",
    "alter_statement",
    "grant_statement",
    "revoke_statement",
    "copy_statement",
    "use_statement",
    "set_statement",
    "show_statement",
    "describe_statement",
    "statement",
];

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
        name: "show statement starts a flow chain",
        sql: "SHOW TABLES IN SCHEMA db.s ->> SELECT \"name\" FROM $1;",
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
    CaptureCase {
        name: "syntax keywords added to tree-sitter table",
        sql: "ALTER TABLE t ADD COLUMN c NUMBER; ROLLBACK;",
        expected: &[
            ("keyword", "ALTER"),
            ("type", "NUMBER"),
            ("keyword", "ROLLBACK"),
        ],
    },
];

fn parse(source: &str) -> tree_sitter::Tree {
    let mut parser =
        sql_dialect_fmt_tree_sitter::parser().expect("load Snowflake Tree-sitter grammar");
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
    // The structural layer: every top-level named child is a statement-kind node (or a
    // free-standing `comment`, which the grammar attaches to the root as an extra).
    for child in root.named_children(&mut root.walk()) {
        assert!(
            child.kind() == "comment" || STATEMENT_NODE_KINDS.contains(&child.kind()),
            "{name} has an unexpected top-level node `{}`: {}",
            child.kind(),
            root.to_sexp()
        );
    }
}

#[test]
fn statements_split_on_semicolons() {
    let tree = parse("SELECT 1;\nSELECT 2;\nSELECT 3");
    let root = tree.root_node();
    assert_eq!(
        root.named_child_count(),
        3,
        "expected three statements: {}",
        root.to_sexp()
    );
}

fn highlight_captures(sql: &str) -> Vec<(String, String)> {
    let language = sql_dialect_fmt_tree_sitter::language();
    let query = Query::new(&language, sql_dialect_fmt_tree_sitter::HIGHLIGHTS_QUERY)
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

fn injection_captures(sql: &str) -> Vec<(String, String)> {
    let language = sql_dialect_fmt_tree_sitter::language();
    let query = Query::new(&language, sql_dialect_fmt_tree_sitter::INJECTIONS_QUERY)
        .expect("injection query compiles");
    let tree = parse(sql);
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), sql.as_bytes());
    let capture_names = query.capture_names();
    let mut seen = Vec::new();

    matches.advance();
    while let Some(query_match) = matches.get() {
        let language = query
            .property_settings(query_match.pattern_index)
            .iter()
            .find(|property| property.key.as_ref() == "injection.language")
            .and_then(|property| property.value.as_deref())
            .expect("injection pattern sets injection.language")
            .to_string();
        for capture in query_match.captures {
            let name = capture_names[capture.index as usize];
            if name != "injection.content" {
                continue;
            }
            let text = capture
                .node
                .utf8_text(sql.as_bytes())
                .expect("capture text is valid UTF-8")
                .to_string();
            seen.push((language.clone(), text));
        }
        matches.advance();
    }

    seen
}

fn js_const_words(name: &str) -> std::collections::HashSet<String> {
    let start_marker = format!("const {name} = [");
    let start = GRAMMAR_JS
        .find(&start_marker)
        .unwrap_or_else(|| panic!("missing {name} array"))
        + start_marker.len();
    let rest = &GRAMMAR_JS[start..];
    let end = rest
        .find("];")
        .unwrap_or_else(|| panic!("unterminated {name} array"));

    rest[..end]
        .lines()
        .filter_map(|line| {
            let word = line.trim().trim_end_matches(',');
            word.strip_prefix('\'')?.strip_suffix('\'')
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn grammar_metadata_is_available() {
    let language = sql_dialect_fmt_tree_sitter::language();

    assert_eq!(language.name(), Some("snowflake"));
    assert!(language.node_kind_count() > 0);
    assert!(sql_dialect_fmt_tree_sitter::NODE_TYPES.contains("\"source_file\""));
}

#[test]
fn highlight_and_support_queries_compile() {
    let language = sql_dialect_fmt_tree_sitter::language();
    let highlights = Query::new(&language, sql_dialect_fmt_tree_sitter::HIGHLIGHTS_QUERY)
        .expect("highlight query compiles");
    Query::new(&language, sql_dialect_fmt_tree_sitter::LOCALS_QUERY)
        .expect("locals query compiles");
    Query::new(&language, sql_dialect_fmt_tree_sitter::INJECTIONS_QUERY)
        .expect("injections query compiles");
    Query::new(&language, sql_dialect_fmt_tree_sitter::FOLDS_QUERY).expect("folds query compiles");
    let indents = Query::new(&language, sql_dialect_fmt_tree_sitter::INDENTS_QUERY)
        .expect("indents query compiles");

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

    let indent_names = indents.capture_names();
    assert!(indent_names.contains(&"indent"));
    assert!(indent_names.contains(&"dedent"));
}

#[test]
fn tree_sitter_keywords_cover_syntax_keywords() {
    let words = js_const_words("KEYWORDS");
    for keyword in keyword_texts() {
        assert!(
            words.contains(keyword),
            "tree-sitter KEYWORDS is missing `{keyword}`"
        );
    }
}

#[test]
fn tree_sitter_types_match_builtin_type_words() {
    let words = js_const_words("TYPES");
    let expected: std::collections::HashSet<String> = BUILTIN_TYPE_WORDS
        .iter()
        .map(|word| word.to_ascii_lowercase())
        .collect();

    assert_eq!(words, expected);
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
fn statement_kind_nodes_classify_top_level_statements() {
    let sql = "SELECT 1; WITH x AS (SELECT 1) SELECT * FROM x; INSERT INTO t VALUES (1); \
               UPDATE t SET a = 1; DELETE FROM t; \
               MERGE INTO d USING s ON d.i = s.i WHEN MATCHED THEN UPDATE SET v = s.v; \
               CREATE TABLE t AS SELECT 1; DROP TABLE t; ALTER TABLE t SET COMMENT = 'c'; \
               GRANT SELECT ON t TO ROLE r; REVOKE SELECT ON t FROM ROLE r; \
               COPY INTO @s FROM t; USE DATABASE d; SET v = 1; SHOW TABLES; \
               DESCRIBE TABLE t; DESC TABLE t; TRUNCATE TABLE t; CALL p();";
    let tree = parse(sql);
    let root = tree.root_node();
    assert!(!root.has_error(), "{}", root.to_sexp());

    let kinds: Vec<&str> = root
        .named_children(&mut root.walk())
        .map(|child| child.kind())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "select_statement",
            "select_statement",
            "insert_statement",
            "update_statement",
            "delete_statement",
            "merge_statement",
            "create_statement",
            "drop_statement",
            "alter_statement",
            "grant_statement",
            "revoke_statement",
            "copy_statement",
            "use_statement",
            "set_statement",
            "show_statement",
            "describe_statement",
            "describe_statement",
            "statement",
            "statement",
        ],
        "{}",
        root.to_sexp()
    );
}

#[test]
fn statement_leading_keywords_stay_keywords_mid_statement() {
    // Statement-leading words appearing inside a statement (subqueries, `UPDATE ... SET`,
    // `GRANT SELECT`, ...) must stay plain `keyword` tokens of the surrounding statement.
    for (sql, expected_kind, expected_statements) in [
        ("CREATE TABLE t AS SELECT 1 FROM s;", "create_statement", 1),
        ("GRANT SELECT ON t TO ROLE r;", "grant_statement", 1),
        ("INSERT INTO t SELECT * FROM s;", "insert_statement", 1),
        ("SELECT * FROM (SELECT 1);", "select_statement", 1),
    ] {
        let tree = parse(sql);
        let root = tree.root_node();
        assert!(!root.has_error(), "{sql}: {}", root.to_sexp());
        assert_eq!(
            root.named_child_count(),
            expected_statements,
            "{sql}: {}",
            root.to_sexp()
        );
        let statement = root.named_child(0).expect("statement node");
        assert_eq!(statement.kind(), expected_kind, "{sql}: {}", root.to_sexp());
    }
}

#[test]
fn node_types_expose_statement_kinds() {
    for kind in STATEMENT_NODE_KINDS {
        assert!(
            sql_dialect_fmt_tree_sitter::NODE_TYPES.contains(&format!("\"{kind}\"")),
            "node-types.json is missing `{kind}`"
        );
    }
}

#[test]
fn expression_nodes_group_calls_and_parentheses() {
    let sql = "SELECT COUNT(*), (ABS(-2) + 1) FROM t;";
    let tree = parse(sql);
    let sexp = tree.root_node().to_sexp();

    assert!(sexp.contains("(expression"), "{sexp}");
    assert!(sexp.contains("(call_expression"), "{sexp}");
    assert!(sexp.contains("(argument_list"), "{sexp}");
    assert!(sexp.contains("(parenthesized_expression"), "{sexp}");
}

#[test]
fn unbalanced_parentheses_still_tokenize_for_mid_edit_sql() {
    for (name, sql) in [
        ("unbalanced open paren", "SELECT ("),
        ("unbalanced close paren", "SELECT )"),
    ] {
        let tree = parse(sql);
        let root = tree.root_node();
        assert_eq!(root.kind(), "source_file", "{name} root kind");
        assert_eq!(
            root.end_byte(),
            sql.len(),
            "{name} did not consume the full input"
        );
    }
}

#[test]
fn injection_query_tags_dollar_bodies_by_context() {
    let sql = "CREATE FUNCTION js() LANGUAGE JAVASCRIPT AS $$return 1;$$; \
               CREATE FUNCTION py() LANGUAGE PYTHON AS $$return 2$$; \
               CREATE FUNCTION java() LANGUAGE JAVA AS $$class C {}$$; \
               CREATE FUNCTION scala() LANGUAGE SCALA AS $$class C {}$$; \
               CREATE FUNCTION sql_fn() LANGUAGE SQL AS $$SELECT 1$$; \
               EXECUTE IMMEDIATE $$ SELECT 3 $$;";
    let captures = injection_captures(sql);
    assert_eq!(
        captures,
        vec![
            ("javascript".to_string(), "$$return 1;$$".to_string()),
            ("python".to_string(), "$$return 2$$".to_string()),
            ("java".to_string(), "$$class C {}$$".to_string()),
            ("scala".to_string(), "$$class C {}$$".to_string()),
            ("sql".to_string(), "$$SELECT 1$$".to_string()),
            ("sql".to_string(), "$$ SELECT 3 $$".to_string()),
        ]
    );
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
    let mut parser =
        sql_dialect_fmt_tree_sitter::parser().expect("load Snowflake Tree-sitter grammar");
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
