//! Spec-driven hover lookup over the generated tables in [`crate::generated`]:
//! keyword/clause features from `spec/seed/features.json` and function
//! signatures from `spec/seed/functions.json`.

use std::ops::Range;

use sql_dialect_fmt_syntax::SyntaxKind;

use crate::generated::{FeatureDoc, FunctionDoc, FEATURES, FUNCTIONS};
use crate::scan::SpannedToken;
use crate::{combined_range, word, Hover, HoverKind};

/// Words allowed between phrase words without breaking the match, so
/// `CREATE OR REPLACE TABLE` still reads as `CREATE TABLE` and `IS NOT NULL`
/// as `IS NULL`.
const PHRASE_FILLERS: &[&str] = &[
    "OR",
    "REPLACE",
    "IF",
    "NOT",
    "EXISTS",
    "TEMP",
    "TEMPORARY",
    "TRANSIENT",
    "SECURE",
    "RECURSIVE",
    "OVERWRITE",
];
const MAX_FILLER_SKIP: usize = 3;

/// Hover for a function-table entry: the hovered word (or dotted chain such as
/// `SNOWFLAKE.CORTEX.SENTIMENT`) followed by `(`, or a bare context function
/// like `CURRENT_TIMESTAMP` that Snowflake accepts without parentheses.
pub(crate) fn function_call_hover(tokens: &[SpannedToken<'_>], index: usize) -> Option<Hover> {
    let token = &tokens[index];
    if !is_word_token(token) {
        return None;
    }
    let mut start = index;
    while start >= 2
        && tokens[start - 1].kind == SyntaxKind::DOT
        && is_word_token(&tokens[start - 2])
    {
        start -= 2;
    }
    let mut end = index;
    while end + 2 < tokens.len()
        && tokens[end + 1].kind == SyntaxKind::DOT
        && is_word_token(&tokens[end + 2])
    {
        end += 2;
    }
    let called = tokens
        .get(end + 1)
        .is_some_and(|next| next.kind == SyntaxKind::L_PAREN);
    let function = if start == end {
        FUNCTIONS
            .iter()
            .find(|f| !f.name.contains('.') && token.text.eq_ignore_ascii_case(f.name))?
    } else {
        let name = tokens[start..=end]
            .iter()
            .map(|part| part.text)
            .collect::<String>()
            .to_ascii_uppercase();
        FUNCTIONS.iter().find(|f| f.name == name)?
    };
    let parenless_ok = start == end && function.parenless;
    if !called && !parenless_ok {
        return None;
    }
    Some(hover_for_function(
        function,
        tokens[start].range.start..tokens[end].range.end,
    ))
}

/// Hover for the longest feature phrase covering the token at `index`, e.g.
/// `BY` inside `GROUP BY` or `JOIN` inside `LEFT OUTER JOIN`.
pub(crate) fn feature_hover(tokens: &[SpannedToken<'_>], index: usize) -> Option<Hover> {
    let mut best: Option<(usize, &FeatureDoc, Range<usize>)> = None;
    for feature in FEATURES {
        for phrase in feature.phrases {
            let Some(span) = phrase_span(tokens, index, phrase) else {
                continue;
            };
            if best.as_ref().is_none_or(|(len, ..)| phrase.len() > *len) {
                best = Some((phrase.len(), feature, span));
            }
        }
    }
    let (_, feature, span) = best?;
    Some(hover_for_feature(feature, combined_range(tokens, span)))
}

fn hover_for_function(function: &FunctionDoc, range: Range<usize>) -> Hover {
    let mut lines = vec![
        format!("`{}`", function.signature),
        format!("Returns `{}`.", function.returns),
        function.summary.to_string(),
    ];
    if function.status != "GA" {
        lines.push(format!("Snowflake status: {}.", function.status));
    }
    let noun = if function.category == "table" {
        "table function"
    } else {
        "function"
    };
    Hover {
        kind: HoverKind::Function,
        title: format!("Snowflake {noun} `{}`", function.name),
        body: lines.join("\n"),
        range,
        docs_url: Some(function.docs_url),
    }
}

fn hover_for_feature(feature: &FeatureDoc, range: Range<usize>) -> Hover {
    let mut lines = vec![format!("`{}`", feature.syntax)];
    if let Some(notes) = feature.notes {
        lines.push(notes.to_string());
    }
    if feature.status != "GA" {
        lines.push(format!("Snowflake status: {}.", feature.status));
    }
    match feature.coverage {
        "partial" => lines.push(String::from(
            "sql-dialect-fmt parses this partially; some forms fall back to the original text.",
        )),
        "todo" => lines.push(String::from(
            "sql-dialect-fmt does not parse this yet; formatting falls back to the original text.",
        )),
        _ => {}
    }
    Hover {
        kind: HoverKind::Feature,
        title: feature.name.to_string(),
        body: lines.join("\n"),
        range,
        docs_url: Some(feature.docs_url),
    }
}

/// Token index span covered when `phrase` matches around `index`, allowing
/// filler words between the phrase words.
fn phrase_span(tokens: &[SpannedToken<'_>], index: usize, phrase: &[&str]) -> Option<Range<usize>> {
    for anchor in 0..phrase.len() {
        if !word(&tokens[index], phrase[anchor]) {
            continue;
        }
        let Some(start) = match_backward(tokens, index, &phrase[..anchor]) else {
            continue;
        };
        let Some(end) = match_forward(tokens, index, &phrase[anchor + 1..]) else {
            continue;
        };
        return Some(start..end + 1);
    }
    None
}

fn match_backward(tokens: &[SpannedToken<'_>], from: usize, words: &[&str]) -> Option<usize> {
    let mut start = from;
    for expected in words.iter().rev() {
        let mut cursor = start.checked_sub(1)?;
        let mut skipped = 0;
        while skipped < MAX_FILLER_SKIP && is_filler(&tokens[cursor]) {
            cursor = cursor.checked_sub(1)?;
            skipped += 1;
        }
        if !word(&tokens[cursor], expected) {
            return None;
        }
        start = cursor;
    }
    Some(start)
}

fn match_forward(tokens: &[SpannedToken<'_>], from: usize, words: &[&str]) -> Option<usize> {
    let mut end = from;
    for expected in words {
        let mut cursor = end + 1;
        let mut skipped = 0;
        while skipped < MAX_FILLER_SKIP && tokens.get(cursor).is_some_and(is_filler) {
            cursor += 1;
            skipped += 1;
        }
        if !word(tokens.get(cursor)?, expected) {
            return None;
        }
        end = cursor;
    }
    Some(end)
}

fn is_filler(token: &SpannedToken<'_>) -> bool {
    PHRASE_FILLERS.iter().any(|filler| word(token, filler))
}

fn is_word_token(token: &SpannedToken<'_>) -> bool {
    token.kind == SyntaxKind::IDENT || token.kind.is_keyword()
}
