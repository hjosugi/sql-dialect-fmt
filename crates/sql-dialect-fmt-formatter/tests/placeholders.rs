//! `${ ... }` template-substitution placeholders: SQL embedded in a host language.
//!
//! SQL is routinely written inside a JavaScript template literal (`\`SELECT ${cfg.col} ...\``) or
//! run through a tool that substitutes `${var}` (Databricks / Spark / dbt). The formatter treats a
//! placeholder as one atomic name/value token: the statement parses, formats like ordinary SQL, and
//! the placeholder text — including nested braces, object literals, quoted `}`, and nested template
//! literals — round-trips verbatim. Every case must (1) parse with no errors, (2) reparse clean
//! after formatting, (3) be idempotent, and (4) preserve its meaningful tokens.

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::{tokenize, SyntaxKind};
use sql_dialect_fmt_parser::parse;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

/// Meaningful tokens the formatter must preserve (upper-cased, minus the synthesized `;`). A
/// placeholder is one token, so its inner `${ ... }` text must survive formatting unchanged.
fn signature(sql: &str) -> Vec<String> {
    tokenize(sql)
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

const CASES: &[&str] = &[
    // ---- placeholders in select-item / table / predicate positions ----
    "select ${cfg.col}, b from ${cfg.t} where id = ${id}",
    "select a from t where n > ${min} and n < ${max}",
    "update t set v = ${val} where id = ${id}",
    "insert into ${cfg.t} (a, b) values (${x}, ${y})",
    "select ${cols} from t group by ${cols}",
    // ---- placeholders as (parts of) dotted names ----
    "select ${schema}.${table} from t",
    "select col from ${db}.${schema}.t",
    "select t.${col} from t",
    // ---- nested structure: object literals, arrays, calls ----
    "select ${ fn({a: 1, b: [2, 3]}) } as c from t",
    "select ${ obj.method({x: {y: {z: 1}}}) } from t",
    // ---- nested structure: quoted `}` and nested template literals must not end the placeholder ----
    "select * from t where s = ${ a || '}' || b }",
    "select ${ `col_${i}` } from t",
    "select ${ `${prefix}_${suffix}` } from t",
    // ---- Databricks / Spark / dbt style substitution ----
    "select * from ${env:CATALOG}.sales where dt = ${var:run_date}",
    // ---- a placeholder inside a string literal stays ordinary string content ----
    "select * from t where dt = '${env:RUN_DATE}'",
];

#[test]
fn all_cases_parse_clean() {
    for sql in CASES {
        let errors = parse(sql).errors().to_vec();
        assert!(errors.is_empty(), "parse errors for {sql:?}: {errors:?}");
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
fn formatted_output_reparses_clean() {
    for sql in CASES {
        let formatted = fmt(sql);
        let errors = parse(&formatted).errors().to_vec();
        assert!(
            errors.is_empty(),
            "formatted output has parse errors for {sql:?}:\n{formatted}\n{errors:?}"
        );
    }
}

#[test]
fn meaningful_tokens_are_preserved() {
    for sql in CASES {
        assert_eq!(
            signature(sql),
            signature(&fmt(sql)),
            "token signature changed for {sql:?}"
        );
    }
}

#[test]
fn placeholder_lexes_as_one_token() {
    // The whole `${ ... }` — braces, strings, and all — is a single PLACEHOLDER token; the inner
    // content never leaks into separate `$`, `{`, `}` tokens that would derail parsing.
    for src in [
        "${id}",
        "${cfg.table}",
        "${ fn({a: 1, b: [2, 3]}) }",
        "${ a || '}' || b }",
        "${ `col_${i}` }",
        "${env:RUN_DATE}",
    ] {
        let tokens: Vec<_> = tokenize(src)
            .tokens
            .into_iter()
            .filter(|t| !t.kind.is_trivia())
            .collect();
        assert_eq!(
            tokens.len(),
            1,
            "expected one token for {src:?}: {tokens:?}"
        );
        assert_eq!(tokens[0].kind, SyntaxKind::PLACEHOLDER, "for {src:?}");
        assert_eq!(tokens[0].text, src, "placeholder text must be verbatim");
    }
}

#[test]
fn canonical_layout_is_stable() {
    // A representative JS-embedded statement lays out like ordinary SQL with the placeholders intact.
    assert_eq!(
        fmt("select ${cfg.col},b from ${cfg.t} where id=${id}"),
        "SELECT ${cfg.col}, b\nFROM ${cfg.t}\nWHERE id = ${id};\n",
    );
    // Nested braces survive verbatim; a `}` inside the object literal does not split the statement.
    assert_eq!(
        fmt("select ${ fn({a: 1}) } as c from t"),
        "SELECT ${ fn({a: 1}) } AS c\nFROM t;\n",
    );
}
