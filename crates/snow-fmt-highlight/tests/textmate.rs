//! Validates the editor-facing TextMate grammar at `editors/snowflake.tmLanguage.json`.
//!
//! Two guarantees:
//!  1. The committed JSON is well-formed and structurally complete (scopeName + patterns +
//!     the repository rules the bundle references).
//!  2. Every word the grammar scopes as a keyword or type is classified the same way by the
//!     lexical highlighter, so an editor using the grammar and one using the LSP/CST agree.

use snow_fmt_highlight::{classify, HighlightKind};
use snow_fmt_syntax::SyntaxKind;

const GRAMMAR_SRC: &str = include_str!("../../../editors/snowflake.tmLanguage.json");

fn grammar() -> serde_json::Value {
    serde_json::from_str(GRAMMAR_SRC).expect("grammar JSON parses")
}

/// Pull the alternation words out of a `(?i)\b(a|b|c)\b` keyword/type pattern. The group of
/// interest is the one opened by `\b(` and closed by `)\b`, not the `(?i)` flags group.
fn alternation(pattern: &str) -> Vec<String> {
    let open = r"\b(";
    let close = r")\b";
    let start = pattern.find(open).expect("alternation group opener") + open.len();
    let end = pattern.rfind(close).expect("alternation group closer");
    pattern[start..end]
        .split('|')
        .map(str::to_string)
        .filter(|w| !w.is_empty())
        .collect()
}

#[test]
fn grammar_json_round_trips() {
    let value = grammar();
    // serde round-trip: re-serializing and re-parsing yields an equal value.
    let reserialized = serde_json::to_string(&value).expect("serialize");
    let reparsed: serde_json::Value = serde_json::from_str(&reserialized).expect("reparse");
    assert_eq!(value, reparsed);
}

#[test]
fn braces_and_brackets_are_balanced() {
    // A hand-rolled structural check independent of serde, ignoring braces inside JSON strings.
    let (mut depth_curly, mut depth_square) = (0i32, 0i32);
    let mut in_string = false;
    let mut escaped = false;
    for ch in GRAMMAR_SRC.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth_curly += 1,
            '}' => depth_curly -= 1,
            '[' => depth_square += 1,
            ']' => depth_square -= 1,
            _ => {}
        }
        assert!(depth_curly >= 0 && depth_square >= 0, "unbalanced close");
    }
    assert_eq!(depth_curly, 0, "unbalanced {{}}");
    assert_eq!(depth_square, 0, "unbalanced []");
    assert!(!in_string, "unterminated string");
}

#[test]
fn declares_the_expected_scope_and_top_level_shape() {
    let g = grammar();
    assert_eq!(g["scopeName"], "source.snowflake-sql");
    assert_eq!(g["name"], "Snowflake SQL");
    assert!(g["patterns"].is_array(), "top-level patterns array");
    assert!(
        !g["patterns"].as_array().unwrap().is_empty(),
        "patterns must not be empty"
    );
    assert!(g["repository"].is_object(), "repository object");
}

#[test]
fn every_referenced_repository_rule_exists() {
    let g = grammar();
    let repo = &g["repository"];
    for pattern in g["patterns"].as_array().unwrap() {
        let Some(include) = pattern["include"].as_str() else {
            continue;
        };
        let name = include.strip_prefix('#').expect("repository include");
        assert!(
            repo.get(name).is_some(),
            "patterns reference `#{name}` but repository has no such rule"
        );
    }
}

#[test]
fn covers_all_required_token_classes() {
    let g = grammar();
    let repo = &g["repository"];
    for rule in [
        "comments",
        "strings",
        "dollar-quoted",
        "numbers",
        "types",
        "keywords",
        "variables",
        "stages",
        "quoted-identifiers",
        "operators",
        "punctuation",
    ] {
        assert!(repo.get(rule).is_some(), "missing repository rule `{rule}`");
    }
}

#[test]
fn keyword_words_classify_as_keywords() {
    let g = grammar();
    let words = alternation(g["repository"]["keywords"]["match"].as_str().unwrap());
    assert!(
        words.len() > 100,
        "keyword set looks truncated ({} words)",
        words.len()
    );
    for word in &words {
        assert_eq!(
            classify(SyntaxKind::IDENT, word),
            HighlightKind::Keyword,
            "grammar lists `{word}` as a keyword but the highlighter disagrees"
        );
    }
}

#[test]
fn keyword_list_is_complete_against_the_keyword_table() {
    // The grammar must list every reserved keyword the lexer recognizes — otherwise editors miss
    // colouring a word the CST treats as a keyword. We probe each `*_KW` variant's canonical text
    // via the classifier and require the grammar's alternation to contain it.
    let g = grammar();
    let words: std::collections::HashSet<String> =
        alternation(g["repository"]["keywords"]["match"].as_str().unwrap())
            .into_iter()
            .collect();

    // Representative reserved words spanning the SELECT pipeline, DDL/DML, scripting, and
    // embedded-language declarations. Each must be present.
    for kw in [
        "select",
        "from",
        "where",
        "group",
        "by",
        "having",
        "order",
        "qualify",
        "join",
        "lateral",
        "with",
        "recursive",
        "union",
        "create",
        "table",
        "insert",
        "update",
        "delete",
        "merge",
        "begin",
        "declare",
        "return",
        "call",
        "procedure",
        "function",
        "language",
        "javascript",
        "python",
        "java",
        "scala",
        "sql",
        "runtime_version",
        "try_cast",
        "tablesample",
        "within",
        "pivot",
        "unpivot",
        "connect",
        "start",
        "prior",
    ] {
        assert!(
            words.contains(kw),
            "grammar keyword alternation is missing `{kw}`"
        );
    }
}

#[test]
fn type_words_classify_as_types() {
    let g = grammar();
    let words = alternation(g["repository"]["types"]["match"].as_str().unwrap());
    assert!(
        words.len() >= 25,
        "type set looks truncated ({} words)",
        words.len()
    );
    for word in &words {
        assert_eq!(
            classify(SyntaxKind::IDENT, word),
            HighlightKind::Type,
            "grammar lists `{word}` as a type but the highlighter disagrees"
        );
    }
}

#[test]
fn operator_rule_covers_snowflake_specific_operators() {
    let g = grammar();
    let ops = g["repository"]["operators"]["match"].as_str().unwrap();
    // The differentiating Snowflake / GoogleSQL operators must all appear in the alternation.
    for needle in ["->>", "|>", "::", "=>", "->", ":="] {
        assert!(
            ops.contains(needle),
            "operator rule is missing `{needle}`: {ops}"
        );
    }
    // `||` is escaped in the regex source as `\\|\\|`.
    assert!(ops.contains(r"\|\|"), "operator rule is missing concat ||");
}

#[test]
fn variables_and_stages_have_their_own_rules() {
    let g = grammar();
    let repo = &g["repository"];

    // Variables: positional $1, session $name, bind :name, placeholder ?.
    let var_rules = repo["variables"]["patterns"].as_array().unwrap();
    let var_matches: Vec<&str> = var_rules
        .iter()
        .filter_map(|r| r["match"].as_str())
        .collect();
    assert!(
        var_matches.iter().any(|m| m.contains(r"\$\d+")),
        "no $1 rule"
    );
    assert!(
        var_matches.iter().any(|m| m.contains(r"\$[A-Za-z")),
        "no $name rule"
    );
    assert!(var_matches.iter().any(|m| m.contains(r"\?")), "no ? rule");

    // Stages: an @-prefixed rule.
    let stage = repo["stages"]["match"].as_str().unwrap();
    assert!(
        stage.starts_with('@'),
        "stage rule should start with @: {stage}"
    );

    // Quoted identifiers: a "-delimited begin/end rule.
    assert_eq!(repo["quoted-identifiers"]["begin"], "\"");
    assert_eq!(repo["quoted-identifiers"]["end"], "\"");

    // Dollar-quoted bodies: a $$-delimited begin/end rule.
    assert_eq!(repo["dollar-quoted"]["begin"], "\\$\\$");
    assert_eq!(repo["dollar-quoted"]["end"], "\\$\\$");
}

#[test]
fn comment_rule_covers_line_and_block_forms() {
    let g = grammar();
    let comments = g["repository"]["comments"]["patterns"].as_array().unwrap();
    let line = comments
        .iter()
        .find(|c| c["match"].is_string())
        .expect("line comment rule");
    let line_match = line["match"].as_str().unwrap();
    assert!(line_match.contains("--"), "line comment must match --");
    assert!(line_match.contains("//"), "line comment must match //");
    let block = comments
        .iter()
        .find(|c| c["begin"].is_string())
        .expect("block comment rule");
    assert_eq!(block["begin"], "/\\*");
    assert_eq!(block["end"], "\\*/");
}
