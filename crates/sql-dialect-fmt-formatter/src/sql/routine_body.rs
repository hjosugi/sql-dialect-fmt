//! Embedded routine body formatting for `CREATE FUNCTION` / `CREATE PROCEDURE`.
//!
//! SQL lowering owns the routine header layout. This module owns delimiter handling and the
//! language-specific formatters for the body token, always returning `None` when a body cannot be
//! formatted safely so the caller can keep the original token verbatim.

#[cfg(feature = "embedded-javascript")]
use biome_formatter::{IndentStyle, IndentWidth, LineWidth};
#[cfg(feature = "embedded-javascript")]
use biome_js_formatter::{context::JsFormatOptions, format_range as format_js_range};
#[cfg(feature = "embedded-javascript")]
use biome_js_parser::{parse as parse_js, JsParserOptions};
#[cfg(feature = "embedded-javascript")]
use biome_js_syntax::{JsFileSource, TextRange, TextSize};
#[cfg(feature = "embedded-python")]
use ruff_formatter::{
    IndentStyle as PyIndentStyle, IndentWidth as PyIndentWidth, LineWidth as PyLineWidth,
};
#[cfg(feature = "embedded-python")]
use ruff_python_formatter::{format_module_source, PyFormatOptions};
use sql_dialect_fmt_syntax::{SyntaxKind::*, SyntaxNode, SyntaxToken};

use crate::doc::{print, PrintOptions};

use super::{lower_source, Ctx};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RoutineBodyLanguage {
    Sql,
    Javascript,
    Python,
    Java,
    Scala,
    Other,
}

pub(super) fn is_create_routine(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter(|el| !el.kind().is_trivia())
        .any(|el| matches!(el.kind(), PROCEDURE_KW | FUNCTION_KW))
}

pub(super) fn routine_body_language(node: &SyntaxNode) -> Option<RoutineBodyLanguage> {
    let clause = node
        .children()
        .find(|child| child.kind() == ROUTINE_LANGUAGE_CLAUSE)?;
    let mut after_language = false;
    for token in clause
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
    {
        if after_language {
            return Some(
                if token.kind() == SQL_KW || token.text().eq_ignore_ascii_case("sql") {
                    RoutineBodyLanguage::Sql
                } else if token.kind() == JAVASCRIPT_KW
                    || token.text().eq_ignore_ascii_case("javascript")
                {
                    RoutineBodyLanguage::Javascript
                } else if token.kind() == PYTHON_KW || token.text().eq_ignore_ascii_case("python") {
                    RoutineBodyLanguage::Python
                } else if token.kind() == JAVA_KW || token.text().eq_ignore_ascii_case("java") {
                    RoutineBodyLanguage::Java
                } else if token.kind() == SCALA_KW || token.text().eq_ignore_ascii_case("scala") {
                    RoutineBodyLanguage::Scala
                } else {
                    RoutineBodyLanguage::Other
                },
            );
        }
        after_language = token.kind() == LANGUAGE_KW;
    }
    None
}

pub(super) fn is_routine_header_word(token: &SyntaxToken) -> bool {
    if !matches!(token.kind(), IDENT | CONTEXTUAL_KEYWORD) {
        return false;
    }
    ROUTINE_HEADER_WORDS
        .binary_search(&token.text().to_ascii_lowercase().as_str())
        .is_ok()
}

pub(super) fn format_embedded_body_token(
    text: &str,
    language: RoutineBodyLanguage,
    ctx: Ctx,
) -> Option<String> {
    match language {
        RoutineBodyLanguage::Sql => format_embedded_sql_body_token(text, ctx),
        RoutineBodyLanguage::Javascript => format_embedded_javascript_body_token(text, ctx),
        RoutineBodyLanguage::Python => format_embedded_python_body_token(text, ctx),
        RoutineBodyLanguage::Java | RoutineBodyLanguage::Scala => {
            format_embedded_brace_language_body_token(text, ctx)
        }
        RoutineBodyLanguage::Other => None,
    }
}

const ROUTINE_HEADER_WORDS: &[&str] = &[
    "artifact_repository",
    "called",
    "caller",
    "copy",
    "external_access_integrations",
    "handler",
    "immutable",
    "imports",
    "input",
    "memoizable",
    "null",
    "owner",
    "packages",
    "restricted",
    "runtime_version",
    "secrets",
    "strict",
    "target_path",
    "user",
    "volatile",
];

fn body_token_content(text: &str) -> Option<String> {
    if let Some(body) = text
        .strip_prefix("$$")
        .and_then(|body| body.strip_suffix("$$"))
    {
        return Some(body.to_string());
    }
    decode_single_quoted_string(text)
}

#[cfg(any(
    feature = "embedded-javascript",
    feature = "embedded-python",
    feature = "embedded-brace-formatters"
))]
fn render_body_token(original: &str, formatted: &str) -> Option<String> {
    if original.starts_with("$$") {
        Some(format!("$$\n{formatted}\n$$"))
    } else if original.starts_with('\'') {
        Some(format!(
            "'\n{}\n'",
            encode_single_quoted_string_body(formatted)
        ))
    } else {
        None
    }
}

fn decode_single_quoted_string(text: &str) -> Option<String> {
    let inner = text.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            if chars.peek() == Some(&'\'') {
                chars.next();
                out.push('\'');
            } else {
                return None;
            }
        } else if ch == '\\' {
            // Keep backslash escapes literal. If they are required to make the embedded source
            // parse, the language formatter will reject and the original token stays verbatim.
            out.push(ch);
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

fn encode_single_quoted_string_body(text: &str) -> String {
    text.replace('\'', "''")
}

#[cfg(feature = "embedded-javascript")]
fn format_embedded_javascript_body_token(text: &str, ctx: Ctx) -> Option<String> {
    let body = body_token_content(text)?;
    let source = body.trim();
    if source.is_empty() {
        return None;
    }

    let formatted = format_javascript_body_once(source, ctx)?;
    if format_javascript_body_once(&formatted, ctx)? != formatted {
        return None;
    }

    render_body_token(text, formatted.trim_end())
}

#[cfg(feature = "embedded-javascript")]
fn format_javascript_body_once(source: &str, ctx: Ctx) -> Option<String> {
    let source_type = JsFileSource::js_script();
    let wrapper_prefix = "function __sql_dialect_fmt_snowflake_body__() {\n";
    let wrapped = format!("{wrapper_prefix}{source}\n}}\n");
    let parse = parse_js(&wrapped, source_type, JsParserOptions::default());
    if parse.has_errors() {
        return None;
    }

    let line_width = LineWidth::try_from(
        ctx.line_width
            .clamp(LineWidth::MIN as usize, LineWidth::MAX as usize) as u16,
    )
    .ok()?;
    let indent_width_value = ctx.indent_width.clamp(1, u8::MAX as usize);
    let indent_width = IndentWidth::from(indent_width_value as u8);
    let options = JsFormatOptions::new(source_type)
        .with_indent_style(IndentStyle::Space)
        .with_indent_width(indent_width)
        .with_line_width(line_width);
    let syntax = parse.syntax();
    let range_start = TextSize::try_from(wrapper_prefix.len()).ok()?;
    let range_end = TextSize::try_from(wrapper_prefix.len() + source.len()).ok()?;
    let printed = format_js_range(options, &syntax, TextRange::new(range_start, range_end)).ok()?;
    let formatted = printed.as_code().trim();
    if formatted.is_empty() {
        None
    } else {
        Some(formatted.trim_end().to_string())
    }
}

#[cfg(not(feature = "embedded-javascript"))]
fn format_embedded_javascript_body_token(_text: &str, _ctx: Ctx) -> Option<String> {
    None
}

#[cfg(feature = "embedded-python")]
fn format_embedded_python_body_token(text: &str, ctx: Ctx) -> Option<String> {
    let body = body_token_content(text)?;
    let source = body.trim();
    if source.is_empty() {
        return None;
    }

    let formatted = format_python_body_once(source, ctx)?;
    if format_python_body_once(&formatted, ctx)? != formatted {
        return None;
    }

    render_body_token(text, formatted.trim_end())
}

#[cfg(feature = "embedded-python")]
fn format_python_body_once(source: &str, ctx: Ctx) -> Option<String> {
    let line_width =
        PyLineWidth::try_from(ctx.line_width.clamp(1, u16::MAX as usize) as u16).ok()?;
    let indent_width =
        PyIndentWidth::try_from(ctx.indent_width.clamp(1, u8::MAX as usize) as u8).ok()?;
    let options = PyFormatOptions::default()
        .with_indent_style(PyIndentStyle::Space)
        .with_indent_width(indent_width)
        .with_line_width(line_width);
    let printed = format_module_source(source, options).ok()?;
    let formatted = printed.as_code().trim();
    if formatted.is_empty() {
        None
    } else {
        Some(formatted.trim_end().to_string())
    }
}

#[cfg(not(feature = "embedded-python"))]
fn format_embedded_python_body_token(_text: &str, _ctx: Ctx) -> Option<String> {
    None
}

#[cfg(feature = "embedded-brace-formatters")]
fn format_embedded_brace_language_body_token(text: &str, ctx: Ctx) -> Option<String> {
    let body = body_token_content(text)?;
    let source = body.trim();
    if source.is_empty() {
        return None;
    }

    let formatted = format_brace_language_body_once(source, ctx.indent_width)?;
    if format_brace_language_body_once(&formatted, ctx.indent_width)? != formatted {
        return None;
    }

    render_body_token(text, formatted.trim_end())
}

#[cfg(feature = "embedded-brace-formatters")]
fn format_brace_language_body_once(source: &str, indent_width: usize) -> Option<String> {
    let mut rough = String::new();
    let mut chars = source.chars().peekable();
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if starts_triple_quote(&chars) => {
                rough.push_str("\"\"\"");
                chars.next()?;
                chars.next()?;
                copy_triple_quoted_literal(&mut chars, &mut rough)?;
            }
            '"' | '\'' => {
                rough.push(ch);
                copy_quoted_literal(ch, &mut chars, &mut rough)?;
            }
            '/' if chars.peek() == Some(&'/') => {
                rough.push('/');
                rough.push(chars.next()?);
                for next in chars.by_ref() {
                    rough.push(next);
                    if next == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                rough.push('/');
                rough.push(chars.next()?);
                let mut closed = false;
                let mut prev = '\0';
                for next in chars.by_ref() {
                    rough.push(next);
                    if prev == '*' && next == '/' {
                        closed = true;
                        break;
                    }
                    prev = next;
                }
                if !closed {
                    return None;
                }
            }
            '(' => {
                paren_depth += 1;
                rough.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.checked_sub(1)?;
                rough.push(ch);
            }
            '[' => {
                bracket_depth += 1;
                rough.push(ch);
            }
            ']' => {
                bracket_depth = bracket_depth.checked_sub(1)?;
                rough.push(ch);
            }
            '{' => {
                brace_depth += 1;
                rough.push(ch);
                push_newline_if_needed(&mut rough);
            }
            '}' => {
                brace_depth = brace_depth.checked_sub(1)?;
                ensure_newline_before(&mut rough);
                rough.push(ch);
                push_newline_if_needed(&mut rough);
            }
            ';' if paren_depth == 0 && bracket_depth == 0 => {
                rough.push(ch);
                push_newline_if_needed(&mut rough);
            }
            _ => rough.push(ch),
        }
    }

    if paren_depth != 0 || bracket_depth != 0 || brace_depth != 0 {
        return None;
    }

    let indent_unit = " ".repeat(indent_width.clamp(1, 16));
    let mut indent = 0usize;
    let mut lines = Vec::new();
    for raw_line in rough.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('}') {
            indent = indent.saturating_sub(1);
        }
        lines.push(format!("{}{}", indent_unit.repeat(indent), line));
        if line.ends_with('{') {
            indent += 1;
        }
    }

    let formatted = lines.join("\n");
    if formatted.is_empty() {
        None
    } else {
        Some(formatted)
    }
}

#[cfg(not(feature = "embedded-brace-formatters"))]
fn format_embedded_brace_language_body_token(_text: &str, _ctx: Ctx) -> Option<String> {
    None
}

#[cfg(feature = "embedded-brace-formatters")]
fn copy_quoted_literal(
    quote: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    out: &mut String,
) -> Option<()> {
    let mut escaped = false;
    for ch in chars.by_ref() {
        out.push(ch);
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return Some(());
        } else if ch == '\n' {
            return None;
        }
    }
    None
}

#[cfg(feature = "embedded-brace-formatters")]
fn starts_triple_quote(chars: &std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let mut lookahead = chars.clone();
    lookahead.next() == Some('"') && lookahead.next() == Some('"')
}

#[cfg(feature = "embedded-brace-formatters")]
fn copy_triple_quoted_literal(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    out: &mut String,
) -> Option<()> {
    let mut quote_run = 0u8;
    for ch in chars.by_ref() {
        out.push(ch);
        if ch == '"' {
            quote_run += 1;
            if quote_run == 3 {
                return Some(());
            }
        } else {
            quote_run = 0;
        }
    }
    None
}

#[cfg(feature = "embedded-brace-formatters")]
fn push_newline_if_needed(out: &mut String) {
    if !out.ends_with('\n') {
        out.push('\n');
    }
}

#[cfg(feature = "embedded-brace-formatters")]
fn ensure_newline_before(out: &mut String) {
    if out.trim_end().is_empty() {
        return;
    }
    let trimmed_len = out.trim_end().len();
    out.truncate(trimmed_len);
    if !out.ends_with('\n') {
        out.push('\n');
    }
}

fn format_embedded_sql_body_token(text: &str, ctx: Ctx) -> Option<String> {
    let body = body_token_content(text)?;
    let source = body.trim();
    if source.is_empty() {
        return None;
    }
    if text.starts_with('\'') && !is_sql_scripting_body(source) {
        return None;
    }
    let lexed = sql_dialect_fmt_lexer::tokenize_for_dialect(source, ctx.dialect);
    if !lexed.errors.is_empty()
        || lexed.tokens.iter().any(|token| {
            !token.kind.is_trivia() && crate::multiline_token_has_line_trailing_space(token.text)
        })
    {
        return None;
    }

    let parse = sql_dialect_fmt_parser::parse_lexed(source, ctx.dialect, lexed);
    if !parse.errors().is_empty() {
        return None;
    }
    let doc = lower_source(&parse.syntax(), ctx);
    let formatted = print(
        &doc,
        &PrintOptions {
            line_width: ctx.line_width,
            indent_width: ctx.indent_width,
        },
    );
    if formatted.is_empty() {
        None
    } else {
        let formatted = formatted.trim_end();
        if text.starts_with("$$") {
            Some(format!("$$\n{formatted}\n$$"))
        } else {
            Some(format!(
                "'\n{}\n'",
                encode_single_quoted_string_body(formatted)
            ))
        }
    }
}

fn is_sql_scripting_body(source: &str) -> bool {
    source.split_whitespace().next().is_some_and(|word| {
        word.eq_ignore_ascii_case("begin") || word.eq_ignore_ascii_case("declare")
    })
}
