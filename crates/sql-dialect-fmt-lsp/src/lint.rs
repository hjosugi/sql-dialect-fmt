use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range};
use sql_dialect_fmt_highlight::{HighlightKind, HighlightToken};
use sql_dialect_fmt_text::LineIndex;

use crate::{lsp_position, PositionEncoding};

/// LSP-only lint knobs layered on top of parser diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LintOptions {
    /// Warn on top-level `SELECT *` in a select list.
    pub select_wildcard: bool,
    /// Warn on large literal `IN (...)` lists.
    pub large_in_list: bool,
    /// Warn when `LANGUAGE <name> AS $$...$$` uses an embedded language the formatter cannot format.
    pub unsupported_embedded_language: bool,
    /// Item count above which a literal `IN (...)` list is considered large.
    pub large_in_list_threshold: usize,
}

impl Default for LintOptions {
    fn default() -> Self {
        Self {
            select_wildcard: true,
            large_in_list: true,
            unsupported_embedded_language: true,
            large_in_list_threshold: 100,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LintCode {
    SelectWildcard,
    LargeInList,
    UnsupportedEmbeddedLanguage,
}

impl LintCode {
    pub fn as_str(self) -> &'static str {
        match self {
            LintCode::SelectWildcard => "SDF001",
            LintCode::LargeInList => "SDF002",
            LintCode::UnsupportedEmbeddedLanguage => "SDF003",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "SDF001" => Some(Self::SelectWildcard),
            "SDF002" => Some(Self::LargeInList),
            "SDF003" => Some(Self::UnsupportedEmbeddedLanguage),
            _ => None,
        }
    }
}

pub fn diagnostic_lint_code(diagnostic: &Diagnostic) -> Option<LintCode> {
    match diagnostic.code.as_ref()? {
        NumberOrString::String(value) => LintCode::from_str(value),
        NumberOrString::Number(_) => None,
    }
}

pub(crate) fn diagnostics_with_encoding(
    text: &str,
    tokens: &[HighlightToken<'_>],
    index: &LineIndex<'_>,
    options: LintOptions,
    encoding: PositionEncoding,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if SelectWildcardRule::enabled(options) {
        diagnostics.extend(SelectWildcardRule::diagnostics(
            tokens, index, options, encoding,
        ));
    }
    if LargeInListRule::enabled(options) {
        diagnostics.extend(LargeInListRule::diagnostics(
            tokens, index, options, encoding,
        ));
    }
    if UnsupportedEmbeddedLanguageRule::enabled(options) {
        diagnostics.extend(UnsupportedEmbeddedLanguageRule::diagnostics(
            tokens, index, options, encoding,
        ));
    }
    diagnostics
        .into_iter()
        .filter(|diagnostic| !is_suppressed(text, diagnostic))
        .collect()
}

trait LintRule {
    const CODE: LintCode;

    fn enabled(options: LintOptions) -> bool;

    fn diagnostics(
        tokens: &[HighlightToken<'_>],
        index: &LineIndex<'_>,
        options: LintOptions,
        encoding: PositionEncoding,
    ) -> Vec<Diagnostic>;

    fn warning(
        index: &LineIndex<'_>,
        range: std::ops::Range<usize>,
        message: &str,
        encoding: PositionEncoding,
    ) -> Diagnostic {
        Diagnostic {
            range: Range::new(
                lsp_position(index, range.start, encoding),
                lsp_position(index, range.end, encoding),
            ),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(Self::CODE.as_str().to_string())),
            source: Some("sql-dialect-fmt".to_string()),
            message: message.to_string(),
            ..Default::default()
        }
    }
}

struct SelectWildcardRule;

impl LintRule for SelectWildcardRule {
    const CODE: LintCode = LintCode::SelectWildcard;

    fn enabled(options: LintOptions) -> bool {
        options.select_wildcard
    }

    fn diagnostics(
        tokens: &[HighlightToken<'_>],
        index: &LineIndex<'_>,
        _options: LintOptions,
        encoding: PositionEncoding,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut in_select_list = false;
        let mut paren_depth = 0usize;
        let mut previous_significant: Option<&str> = None;

        for token in tokens.iter().filter(|token| is_significant(token.kind)) {
            if token.kind == HighlightKind::Keyword && token.text.eq_ignore_ascii_case("select") {
                in_select_list = true;
                paren_depth = 0;
                previous_significant = Some(token.text);
                continue;
            }

            if in_select_list {
                match token.text {
                    "(" => paren_depth += 1,
                    ")" => paren_depth = paren_depth.saturating_sub(1),
                    ";" if paren_depth == 0 => in_select_list = false,
                    _ => {
                        if paren_depth == 0
                            && token.kind == HighlightKind::Keyword
                            && token.text.eq_ignore_ascii_case("from")
                        {
                            in_select_list = false;
                        } else if paren_depth == 0
                            && token.text == "*"
                            && previous_significant.is_some_and(is_wildcard_prefix)
                        {
                            diagnostics.push(Self::warning(
                                index,
                                token.range.clone(),
                                "avoid SELECT * in shared SQL; list columns explicitly",
                                encoding,
                            ));
                        }
                    }
                }
            }

            previous_significant = Some(token.text);
        }

        diagnostics
    }
}

struct LargeInListRule;

impl LintRule for LargeInListRule {
    const CODE: LintCode = LintCode::LargeInList;

    fn enabled(options: LintOptions) -> bool {
        options.large_in_list
    }

    fn diagnostics(
        tokens: &[HighlightToken<'_>],
        index: &LineIndex<'_>,
        options: LintOptions,
        encoding: PositionEncoding,
    ) -> Vec<Diagnostic> {
        #[derive(Debug)]
        struct InList {
            start: usize,
            depth: usize,
            commas: usize,
            saw_top_level_item: bool,
            possible_subquery: bool,
        }

        let mut diagnostics = Vec::new();
        let mut pending_in: Option<usize> = None;
        let mut list: Option<InList> = None;

        for token in tokens.iter().filter(|token| is_significant(token.kind)) {
            let mut close_list_at: Option<usize> = None;
            if let Some(active) = list.as_mut() {
                match token.text {
                    "(" => active.depth += 1,
                    ")" => {
                        active.depth = active.depth.saturating_sub(1);
                        if active.depth == 0 {
                            close_list_at = Some(token.range.end);
                        }
                    }
                    "," if active.depth == 1 => active.commas += 1,
                    _ if active.depth == 1 && !active.saw_top_level_item => {
                        active.saw_top_level_item = true;
                        if token.kind == HighlightKind::Keyword
                            && (token.text.eq_ignore_ascii_case("select")
                                || token.text.eq_ignore_ascii_case("with"))
                        {
                            active.possible_subquery = true;
                        }
                    }
                    _ => {}
                }

                if let Some(end) = close_list_at {
                    if let Some(active) = list.take() {
                        let item_count = if active.saw_top_level_item {
                            active.commas + 1
                        } else {
                            0
                        };
                        if !active.possible_subquery && item_count > options.large_in_list_threshold
                        {
                            diagnostics.push(Self::warning(
                                index,
                                active.start..end,
                                "large IN list; prefer a temp table, CTE, or semi-join when practical",
                                encoding,
                            ));
                        }
                    }
                }
                continue;
            }

            if let Some(start) = pending_in.take() {
                if token.text == "(" {
                    list = Some(InList {
                        start,
                        depth: 1,
                        commas: 0,
                        saw_top_level_item: false,
                        possible_subquery: false,
                    });
                    continue;
                }
            }

            if token.kind == HighlightKind::Keyword && token.text.eq_ignore_ascii_case("in") {
                pending_in = Some(token.range.start);
            }
        }

        diagnostics
    }
}

struct UnsupportedEmbeddedLanguageRule;

impl LintRule for UnsupportedEmbeddedLanguageRule {
    const CODE: LintCode = LintCode::UnsupportedEmbeddedLanguage;

    fn enabled(options: LintOptions) -> bool {
        options.unsupported_embedded_language
    }

    fn diagnostics(
        tokens: &[HighlightToken<'_>],
        index: &LineIndex<'_>,
        _options: LintOptions,
        encoding: PositionEncoding,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut expect_language_name = false;
        let mut language_name: Option<(&str, std::ops::Range<usize>)> = None;
        let mut saw_as_after_language = false;

        for token in tokens {
            match token.kind {
                HighlightKind::Whitespace | HighlightKind::Comment => {}
                HighlightKind::Punctuation if token.text == ";" => {
                    expect_language_name = false;
                    language_name = None;
                    saw_as_after_language = false;
                }
                HighlightKind::DollarString => {
                    if saw_as_after_language {
                        if let Some((word, range)) = language_name.take() {
                            if !is_supported_embedded_language(word) {
                                diagnostics.push(Self::warning(
                                    index,
                                    range,
                                    &format!(
                                        "unsupported embedded language {word}; expected SQL, JAVASCRIPT, PYTHON, JAVA, or SCALA"
                                    ),
                                    encoding,
                                ));
                            }
                        }
                    }
                    expect_language_name = false;
                    saw_as_after_language = false;
                }
                HighlightKind::Keyword if token.text.eq_ignore_ascii_case("language") => {
                    expect_language_name = true;
                    language_name = None;
                    saw_as_after_language = false;
                }
                HighlightKind::Keyword | HighlightKind::Identifier | HighlightKind::Type
                    if expect_language_name =>
                {
                    language_name = Some((token.text, token.range.clone()));
                    expect_language_name = false;
                }
                HighlightKind::Keyword
                    if language_name.is_some() && token.text.eq_ignore_ascii_case("as") =>
                {
                    saw_as_after_language = true;
                }
                _ => {
                    expect_language_name = false;
                }
            }
        }

        diagnostics
    }
}

fn is_suppressed(text: &str, diagnostic: &Diagnostic) -> bool {
    let Some(code) = diagnostic_lint_code(diagnostic) else {
        return false;
    };
    let line = diagnostic.range.start.line as usize;
    if line == 0 {
        return false;
    }
    let Some(previous_line) = text.lines().nth(line - 1) else {
        return false;
    };
    line_suppresses_code(previous_line, code)
}

fn line_suppresses_code(line: &str, code: LintCode) -> bool {
    let trimmed = line.trim_start();
    let Some(comment) = trimmed.strip_prefix("--") else {
        return false;
    };
    let Some((_, rest)) = comment.split_once("sql-dialect-fmt: disable-next-line") else {
        return false;
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return true;
    }
    rest.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
        .filter(|part| !part.is_empty())
        .any(|part| {
            part.eq_ignore_ascii_case(code.as_str())
                || part.eq_ignore_ascii_case("all")
                || part.eq_ignore_ascii_case("lint")
        })
}

fn is_wildcard_prefix(text: &str) -> bool {
    matches!(text, "," | ".")
        || text.eq_ignore_ascii_case("select")
        || text.eq_ignore_ascii_case("distinct")
        || text.eq_ignore_ascii_case("all")
}

fn is_significant(kind: HighlightKind) -> bool {
    !matches!(kind, HighlightKind::Whitespace | HighlightKind::Comment)
}

fn is_supported_embedded_language(word: &str) -> bool {
    ["SQL", "JAVASCRIPT", "PYTHON", "JAVA", "SCALA"]
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sql_dialect_fmt_highlight::highlight;

    fn lint_diagnostics(text: &str, options: LintOptions) -> Vec<Diagnostic> {
        let index = LineIndex::new(text);
        let highlighted = highlight(text);
        diagnostics_with_encoding(
            text,
            &highlighted.tokens,
            &index,
            options,
            PositionEncoding::Utf16,
        )
    }

    #[test]
    fn disable_next_line_suppresses_specific_lint_code() {
        let text = "-- sql-dialect-fmt: disable-next-line SDF001\nSELECT * FROM t;";
        assert!(lint_diagnostics(text, LintOptions::default()).is_empty());
    }

    #[test]
    fn disable_next_line_suppresses_all_lint_codes_when_code_is_omitted() {
        let text = "-- sql-dialect-fmt: disable-next-line\nSELECT * FROM t;";
        assert!(lint_diagnostics(text, LintOptions::default()).is_empty());
    }

    #[test]
    fn disable_next_line_does_not_suppress_other_lines() {
        let text =
            "-- sql-dialect-fmt: disable-next-line SDF001\nSELECT a FROM t;\nSELECT * FROM t;";
        assert_eq!(lint_diagnostics(text, LintOptions::default()).len(), 1);
    }
}
