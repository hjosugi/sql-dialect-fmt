//! Validates the editor-facing TextMate grammar at `editors/snowflake.tmLanguage.json`.
//!
//! Two guarantees:
//!  1. The committed JSON is well-formed and structurally complete (scopeName + patterns +
//!     the repository rules the bundle references).
//!  2. Every word the grammar scopes as a keyword or type is classified the same way by the
//!     lexical highlighter, so an editor using the grammar and one using the LSP/CST agree.

use sql_dialect_fmt_highlight::{classify, HighlightKind};
use sql_dialect_fmt_syntax::{keyword_texts, SyntaxKind, BUILTIN_TYPE_WORDS};

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
        "javascript-routine",
        "python-routine",
        "java-routine",
        "scala-routine",
        "sql-routine",
        "execute-immediate",
        "as-dollar-sql",
        "sql-content",
        "comments",
        "strings",
        "dollar-quoted",
        "numbers",
        "ddl-object-name",
        "constants",
        "types",
        "scripting-types",
        "builtin-functions",
        "context-functions",
        "scripting-variables",
        "builtin-exceptions",
        "keywords-control",
        "keywords",
        "keywords-nonreserved",
        "generic-function-call",
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
fn javascript_routines_embed_the_vscode_javascript_scope() {
    let g = grammar();
    let routine = &g["repository"]["javascript-routine"];
    let patterns = routine["patterns"].as_array().expect("routine patterns");
    let embedded = patterns
        .iter()
        .find(|pattern| pattern["name"] == "meta.embedded.block.javascript.snowflake")
        .expect("embedded JavaScript body rule");

    assert_eq!(embedded["begin"], "\\$\\$");
    assert_eq!(embedded["end"], "\\$\\$");
    assert_eq!(embedded["contentName"], "source.js");
    assert!(
        embedded["patterns"]
            .as_array()
            .expect("embedded patterns")
            .iter()
            .any(|pattern| pattern["include"] == "source.js"),
        "embedded body must include VS Code's JavaScript grammar"
    );
}

#[test]
fn foreign_routines_embed_their_host_language_scopes() {
    let g = grammar();
    for (rule, embedded, source) in [
        (
            "python-routine",
            "meta.embedded.block.python.snowflake",
            "source.python",
        ),
        (
            "java-routine",
            "meta.embedded.block.java.snowflake",
            "source.java",
        ),
        (
            "scala-routine",
            "meta.embedded.block.scala.snowflake",
            "source.scala",
        ),
    ] {
        let routine = &g["repository"][rule];
        let patterns = routine["patterns"].as_array().expect("routine patterns");
        let body = patterns
            .iter()
            .find(|pattern| pattern["name"] == embedded)
            .unwrap_or_else(|| panic!("`{rule}` lacks its embedded body rule"));
        assert_eq!(body["contentName"], source, "`{rule}` contentName");
        assert!(
            body["patterns"]
                .as_array()
                .expect("embedded patterns")
                .iter()
                .any(|pattern| pattern["include"] == source),
            "`{rule}` body must include `{source}`"
        );
    }
}

#[test]
fn sql_routine_bodies_reuse_the_sql_content_rule() {
    let g = grammar();
    for rule in ["sql-routine", "execute-immediate"] {
        let patterns = g["repository"][rule]["patterns"]
            .as_array()
            .expect("routine patterns");
        assert!(
            patterns
                .iter()
                .any(|pattern| pattern["include"] == "#sql-dollar-body"),
            "`{rule}` must delegate $$ bodies to #sql-dollar-body"
        );
    }
    let body = &g["repository"]["sql-dollar-body"];
    assert_eq!(body["name"], "meta.embedded.block.sql.snowflake");
    assert!(
        body["patterns"]
            .as_array()
            .expect("body patterns")
            .iter()
            .any(|pattern| pattern["include"] == "#sql-content"),
        "$$ SQL bodies must highlight as SQL content"
    );
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

/// Snowflake Scripting control words the grammar colours as control flow but the parser does not
/// reserve (they are ordinary identifiers outside scripting statements).
const NONRESERVED_CONTROL: &[&str] = &["break", "continue", "exit", "iterate", "raise"];

#[test]
fn control_words_split_reserved_vs_scripting() {
    let g = grammar();
    let words = alternation(
        g["repository"]["keywords-control"]["match"]
            .as_str()
            .unwrap(),
    );
    assert!(
        words.len() >= 20,
        "control keyword set looks truncated ({} words)",
        words.len()
    );
    for word in &words {
        let expected = if NONRESERVED_CONTROL.contains(&word.as_str()) {
            HighlightKind::Identifier
        } else {
            HighlightKind::Keyword
        };
        assert_eq!(
            classify(SyntaxKind::IDENT, word),
            expected,
            "control word `{word}` classification drifted"
        );
    }
}

#[test]
fn constant_words_are_the_reserved_literals() {
    let g = grammar();
    let words: std::collections::HashSet<String> =
        alternation(g["repository"]["constants"]["match"].as_str().unwrap())
            .into_iter()
            .collect();
    let expected: std::collections::HashSet<String> = ["true", "false", "null"]
        .iter()
        .map(|w| w.to_string())
        .collect();
    assert_eq!(words, expected);
    for word in &words {
        assert_eq!(classify(SyntaxKind::IDENT, word), HighlightKind::Keyword);
    }
}

#[test]
fn scripting_type_words_are_reserved_keywords_rescoped_as_types() {
    // CURSOR and RESULTSET are reserved words in the parser table; the grammar deliberately gives
    // them a type scope because they are Snowflake Scripting declaration types.
    let g = grammar();
    let words: std::collections::HashSet<String> = alternation(
        g["repository"]["scripting-types"]["match"]
            .as_str()
            .unwrap(),
    )
    .into_iter()
    .collect();
    let expected: std::collections::HashSet<String> = ["cursor", "resultset"]
        .iter()
        .map(|w| w.to_string())
        .collect();
    assert_eq!(words, expected);
    for word in &words {
        assert_eq!(classify(SyntaxKind::IDENT, word), HighlightKind::Keyword);
    }
}

#[test]
fn nonreserved_keyword_words_are_not_reserved() {
    // Words the grammar colours as keywords for readability must stay plain identifiers for the
    // parser; if one is ever promoted to the reserved table this fails and the word must move to
    // the `keywords` rule.
    let g = grammar();
    let words = alternation(
        g["repository"]["keywords-nonreserved"]["match"]
            .as_str()
            .unwrap(),
    );
    assert!(!words.is_empty());
    for word in &words {
        assert_eq!(
            classify(SyntaxKind::IDENT, word),
            HighlightKind::Identifier,
            "`{word}` is reserved or a type; move it out of keywords-nonreserved"
        );
    }
}

#[test]
fn keyword_list_is_complete_against_the_keyword_table() {
    // Every reserved word must be scoped by exactly one of the keyword-ish rules: plain keywords,
    // control flow, literal constants, or the scripting declaration types.
    let g = grammar();
    let repo = &g["repository"];
    let mut words: std::collections::HashSet<String> = std::collections::HashSet::new();
    for rule in [
        "keywords",
        "keywords-control",
        "constants",
        "scripting-types",
    ] {
        words.extend(alternation(repo[rule]["match"].as_str().unwrap()));
    }

    for kw in keyword_texts() {
        assert!(
            words.contains(kw),
            "no grammar keyword rule covers reserved word `{kw}`"
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
fn type_list_matches_the_builtin_type_table() {
    let g = grammar();
    let words: std::collections::HashSet<String> =
        alternation(g["repository"]["types"]["match"].as_str().unwrap())
            .into_iter()
            .collect();
    let expected: std::collections::HashSet<String> = BUILTIN_TYPE_WORDS
        .iter()
        .map(|word| word.to_ascii_lowercase())
        .collect();

    assert_eq!(words, expected);
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
