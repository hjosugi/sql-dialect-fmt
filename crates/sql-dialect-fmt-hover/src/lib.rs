//! Hover information for Snowflake SQL editor integrations.
//!
//! This crate is intentionally LSP-agnostic. LSP, Tree-sitter adapters, and CLI
//! diagnostics can all call [`hover_at`] and translate the result into their own
//! wire format.

use sql_dialect_fmt_syntax::SyntaxKind;
use std::ops::Range;

mod data;
mod scan;

use data::{
    keyword_template, language_template, property_template, type_info, StaticHover,
    ROUTINE_OPTION_STOPS,
};
use scan::{spanned_tokens, token_at, SpannedToken};

pub use data::{CREATE_PROCEDURE_DOCS, CREATE_TASK_DOCS, DATA_TYPES_DOCS};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hover {
    pub kind: HoverKind,
    pub title: String,
    pub body: String,
    pub range: Range<usize>,
    pub docs_url: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverKind {
    Keyword,
    Type,
    Procedure,
    Task,
    Language,
    Property,
}

/// Return hover information for the token at `offset`.
///
/// Offsets are byte offsets, matching LSP's UTF-8 internal representation after
/// the caller converts from line/column. Trivia currently has no hover.
pub fn hover_at(source: &str, offset: usize) -> Option<Hover> {
    let tokens = spanned_tokens(source);
    let index = token_at(&tokens, offset)?;
    let token = tokens[index].clone();

    if let Some(hover) = procedure_symbol_hover(source, &tokens, index) {
        return Some(hover);
    }
    if let Some(hover) = task_symbol_hover(source, &tokens, index) {
        return Some(hover);
    }
    if let Some(hover) = type_hover(&token) {
        return Some(hover);
    }
    if let Some(hover) = language_hover(&token) {
        return Some(hover);
    }
    if let Some(hover) = property_hover(&token) {
        return Some(hover);
    }
    keyword_hover(&token)
}

fn procedure_symbol_hover(
    source: &str,
    tokens: &[SpannedToken<'_>],
    index: usize,
) -> Option<Hover> {
    let object = object_declaration(tokens, index, "PROCEDURE")?;
    if word(tokens.get(object.keyword + 1)?, "SCOPED") {
        return None;
    }
    let name_range = procedure_name_range(tokens, object.keyword, object.end)?;
    if !name_range.contains(&index) {
        return None;
    }

    let name = compact_token_text(tokens, name_range.clone());
    let args = procedure_args(source, tokens, name_range.end, object.end)
        .unwrap_or_else(|| String::from(""));
    let returns = clause_after_keyword(
        source,
        tokens,
        object.keyword,
        object.end,
        "RETURNS",
        &[
            "LANGUAGE",
            "RUNTIME_VERSION",
            "PACKAGES",
            "IMPORTS",
            "HANDLER",
            "AS",
            "COMMENT",
            "EXECUTE",
        ],
    );
    let language = value_after_keyword(source, tokens, object.keyword, object.end, "LANGUAGE");
    let handler =
        routine_option_after_keyword(source, tokens, object.keyword, object.end, "HANDLER");
    let runtime = routine_option_after_keyword(
        source,
        tokens,
        object.keyword,
        object.end,
        "RUNTIME_VERSION",
    );
    let packages =
        routine_option_after_keyword(source, tokens, object.keyword, object.end, "PACKAGES");
    let imports =
        routine_option_after_keyword(source, tokens, object.keyword, object.end, "IMPORTS");
    let target_path =
        routine_option_after_keyword(source, tokens, object.keyword, object.end, "TARGET_PATH");

    let mut lines = vec![format!("Stored procedure `{name}`.")];
    if !args.is_empty() {
        lines.push(format!("Arguments: `{args}`."));
    }
    if let Some(returns) = returns {
        lines.push(format!("Returns: `{returns}`."));
    }
    if let Some(language) = language {
        lines.push(format!("Language: `{language}`."));
    }
    if let Some(handler) = handler {
        lines.push(format!("Handler: `{handler}`."));
    }
    if let Some(runtime) = runtime {
        lines.push(format!("Runtime: `{runtime}`."));
    }
    if let Some(packages) = packages {
        lines.push(format!("Packages: `{packages}`."));
    }
    if let Some(imports) = imports {
        lines.push(format!("Imports: `{imports}`."));
    }
    if let Some(target_path) = target_path {
        lines.push(format!("Target path: `{target_path}`."));
    }
    lines.push(String::from(
        "Snowflake resolves stored procedures by name plus argument types.",
    ));
    lines.push(String::from(
        "External-language procedures usually pair LANGUAGE with HANDLER, PACKAGES, IMPORTS, and RUNTIME_VERSION.",
    ));

    Some(Hover {
        kind: HoverKind::Procedure,
        title: format!("Stored procedure `{name}`"),
        body: lines.join("\n"),
        range: combined_range(tokens, name_range),
        docs_url: Some(CREATE_PROCEDURE_DOCS),
    })
}

fn task_symbol_hover(source: &str, tokens: &[SpannedToken<'_>], index: usize) -> Option<Hover> {
    let object = object_declaration(tokens, index, "TASK")?;
    let name_range = task_name_range(tokens, object.keyword, object.end)?;
    if !name_range.contains(&index) {
        return None;
    }

    let name = compact_token_text(tokens, name_range.clone());
    let warehouse = value_after_keyword(source, tokens, object.keyword, object.end, "WAREHOUSE")
        .or_else(|| {
            value_after_keyword(
                source,
                tokens,
                object.keyword,
                object.end,
                "USER_TASK_MANAGED_INITIAL_WAREHOUSE_SIZE",
            )
        });
    let schedule = value_after_keyword(source, tokens, object.keyword, object.end, "SCHEDULE");
    let after = clause_after_keyword(
        source,
        tokens,
        object.keyword,
        object.end,
        "AFTER",
        &["WHEN", "AS", "EXECUTE", "COMMENT", "FINALIZE"],
    );
    let when = clause_after_keyword(source, tokens, object.keyword, object.end, "WHEN", &["AS"]);

    let mut lines = vec![format!("Task `{name}`.")];
    if let Some(warehouse) = warehouse {
        lines.push(format!("Compute: `{warehouse}`."));
    }
    if let Some(schedule) = schedule {
        lines.push(format!("Schedule: `{schedule}`."));
    }
    if let Some(after) = after {
        lines.push(format!("Predecessors: `{after}`."));
    }
    if let Some(when) = when {
        lines.push(format!("Condition: `{when}`."));
    }
    lines.push(String::from(
        "Tasks run SQL on a schedule or after predecessor tasks; newly created tasks start suspended.",
    ));

    Some(Hover {
        kind: HoverKind::Task,
        title: format!("Task `{name}`"),
        body: lines.join("\n"),
        range: combined_range(tokens, name_range),
        docs_url: Some(CREATE_TASK_DOCS),
    })
}

#[derive(Clone, Copy)]
struct ObjectDeclaration {
    keyword: usize,
    end: usize,
}

fn object_declaration(
    tokens: &[SpannedToken<'_>],
    index: usize,
    object_keyword: &str,
) -> Option<ObjectDeclaration> {
    let (start, end) = statement_bounds(tokens, index);
    for keyword in (start..=index.min(end.saturating_sub(1))).rev() {
        if !word(&tokens[keyword], object_keyword) {
            continue;
        }
        if tokens[start..keyword]
            .iter()
            .any(|token| word(token, "CREATE"))
        {
            return Some(ObjectDeclaration { keyword, end });
        }
    }
    None
}

fn statement_bounds(tokens: &[SpannedToken<'_>], index: usize) -> (usize, usize) {
    let start = tokens[..index]
        .iter()
        .rposition(|token| token.kind == SyntaxKind::SEMICOLON)
        .map_or(0, |idx| idx + 1);
    let end = tokens[index..]
        .iter()
        .position(|token| token.kind == SyntaxKind::SEMICOLON)
        .map_or(tokens.len(), |relative| index + relative);
    (start, end)
}

fn procedure_name_range(
    tokens: &[SpannedToken<'_>],
    procedure_keyword: usize,
    end: usize,
) -> Option<Range<usize>> {
    let start = procedure_keyword + 1;
    let mut cursor = start;
    while cursor < end && tokens[cursor].kind != SyntaxKind::L_PAREN {
        if !is_name_part(&tokens[cursor]) {
            return None;
        }
        cursor += 1;
    }
    (start < cursor).then_some(start..cursor)
}

fn task_name_range(
    tokens: &[SpannedToken<'_>],
    task_keyword: usize,
    end: usize,
) -> Option<Range<usize>> {
    let mut start = task_keyword + 1;
    if word_seq(tokens, start, &["IF", "NOT", "EXISTS"]) {
        start += 3;
    }

    let mut cursor = start;
    while cursor < end && is_name_part(&tokens[cursor]) && !is_clause_boundary(&tokens[cursor]) {
        cursor += 1;
    }
    (start < cursor).then_some(start..cursor)
}

fn procedure_args(
    source: &str,
    tokens: &[SpannedToken<'_>],
    start: usize,
    end: usize,
) -> Option<String> {
    let open = (start..end).find(|&idx| tokens[idx].kind == SyntaxKind::L_PAREN)?;
    let close = matching_paren(tokens, open, end)?;
    let inside = token_slice(source, tokens, open + 1, close);
    Some(inside)
}

fn matching_paren(tokens: &[SpannedToken<'_>], open: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, token) in tokens.iter().enumerate().take(end).skip(open) {
        match token.kind {
            SyntaxKind::L_PAREN => depth += 1,
            SyntaxKind::R_PAREN => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn value_after_keyword(
    source: &str,
    tokens: &[SpannedToken<'_>],
    start: usize,
    end: usize,
    keyword: &str,
) -> Option<String> {
    let idx = (start..end).find(|&idx| word(&tokens[idx], keyword))?;
    let mut value_start = idx + 1;
    if value_start < end && tokens[value_start].kind == SyntaxKind::EQ {
        value_start += 1;
    }
    let mut value_end = value_start;
    while value_end < end
        && !is_clause_boundary(&tokens[value_end])
        && tokens[value_end].kind != SyntaxKind::COMMA
    {
        value_end += 1;
    }
    let value = token_slice(source, tokens, value_start, value_end);
    (!value.is_empty()).then_some(value)
}

fn routine_option_after_keyword(
    source: &str,
    tokens: &[SpannedToken<'_>],
    start: usize,
    end: usize,
    keyword: &str,
) -> Option<String> {
    clause_after_keyword(source, tokens, start, end, keyword, ROUTINE_OPTION_STOPS)
        .map(|value| value.strip_prefix("= ").unwrap_or(&value).to_string())
}

fn clause_after_keyword(
    source: &str,
    tokens: &[SpannedToken<'_>],
    start: usize,
    end: usize,
    keyword: &str,
    stops: &[&str],
) -> Option<String> {
    let idx = (start..end).find(|&idx| word(&tokens[idx], keyword))?;
    let value_start = idx + 1;
    let mut depth = 0usize;
    let mut value_end = value_start;
    while value_end < end {
        let token = &tokens[value_end];
        match token.kind {
            SyntaxKind::L_PAREN | SyntaxKind::L_BRACKET => depth += 1,
            SyntaxKind::R_PAREN | SyntaxKind::R_BRACKET => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && stops.iter().any(|stop| word(token, stop)) {
            break;
        }
        value_end += 1;
    }
    let value = token_slice(source, tokens, value_start, value_end);
    (!value.is_empty()).then_some(value)
}

fn token_slice(source: &str, tokens: &[SpannedToken<'_>], start: usize, end: usize) -> String {
    if start >= end || start >= tokens.len() {
        return String::new();
    }
    let end = end.min(tokens.len());
    compact_text(&source[tokens[start].range.start..tokens[end - 1].range.end])
}

fn compact_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compact_token_text(tokens: &[SpannedToken<'_>], range: Range<usize>) -> String {
    tokens[range]
        .iter()
        .map(|token| token.text)
        .collect::<Vec<_>>()
        .join("")
}

fn combined_range(tokens: &[SpannedToken<'_>], range: Range<usize>) -> Range<usize> {
    tokens[range.start].range.start..tokens[range.end - 1].range.end
}

fn is_name_part(token: &SpannedToken<'_>) -> bool {
    matches!(
        token.kind,
        SyntaxKind::IDENT | SyntaxKind::QUOTED_IDENT | SyntaxKind::DOT
    )
}

fn is_clause_boundary(token: &SpannedToken<'_>) -> bool {
    matches!(
        token.kind,
        SyntaxKind::SEMICOLON | SyntaxKind::R_PAREN | SyntaxKind::R_BRACKET
    ) || [
        "AS",
        "AFTER",
        "ARTIFACT_REPOSITORY",
        "CALLED",
        "COMMENT",
        "CONFIG",
        "COPY",
        "ERROR_INTEGRATION",
        "EXECUTE",
        "EXTERNAL_ACCESS_INTEGRATIONS",
        "FINALIZE",
        "HANDLER",
        "IMMUTABLE",
        "IMPORTS",
        "LANGUAGE",
        "MEMOIZABLE",
        "NULL",
        "OVERLAP_POLICY",
        "PACKAGES",
        "RETURNS",
        "RUNTIME_VERSION",
        "SCHEDULE",
        "SECRETS",
        "SECURE",
        "STRICT",
        "SUCCESS_INTEGRATION",
        "TASK_AUTO_RETRY_ATTEMPTS",
        "TARGET_PATH",
        "USER_TASK_MANAGED_INITIAL_WAREHOUSE_SIZE",
        "USER_TASK_TIMEOUT_MS",
        "VOLATILE",
        "WAREHOUSE",
        "WHEN",
    ]
    .iter()
    .any(|boundary| word(token, boundary))
}

fn word(token: &SpannedToken<'_>, expected: &str) -> bool {
    (token.kind == SyntaxKind::IDENT || token.kind.is_keyword())
        && token.text.eq_ignore_ascii_case(expected)
}

fn word_seq(tokens: &[SpannedToken<'_>], start: usize, words: &[&str]) -> bool {
    words.iter().enumerate().all(|(offset, expected)| {
        tokens
            .get(start + offset)
            .is_some_and(|t| word(t, expected))
    })
}

fn type_hover(token: &SpannedToken<'_>) -> Option<Hover> {
    let (canonical, body) = type_info(token.text)?;
    Some(Hover {
        kind: HoverKind::Type,
        title: format!("Snowflake type `{canonical}`"),
        body: body.to_string(),
        range: token.range.clone(),
        docs_url: Some(DATA_TYPES_DOCS),
    })
}

fn language_hover(token: &SpannedToken<'_>) -> Option<Hover> {
    language_template(token.text).map(|template| from_static(token.range.clone(), template))
}

fn property_hover(token: &SpannedToken<'_>) -> Option<Hover> {
    property_template(token.text).map(|template| from_static(token.range.clone(), template))
}

fn keyword_hover(token: &SpannedToken<'_>) -> Option<Hover> {
    keyword_template(token.text).map(|template| from_static(token.range.clone(), template))
}

fn from_static(range: Range<usize>, template: StaticHover) -> Hover {
    Hover {
        kind: template.kind,
        title: template.title.to_string(),
        body: template.body.to_string(),
        range,
        docs_url: template.docs_url,
    }
}
