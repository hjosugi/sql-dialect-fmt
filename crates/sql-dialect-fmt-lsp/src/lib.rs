//! LSP feature logic for Snowflake SQL: formatting edits, parse diagnostics, and semantic tokens.
//!
//! This module is deliberately transport-free (it never touches stdio or an `lsp-server`
//! connection), so every feature is a pure `&str -> data` function that is unit-testable. The
//! binary crate is the thin adapter that wires these into a language server.
//!
//! Positions follow the LSP convention: zero-based lines and **UTF-16** column offsets.

use lsp_types::{
    Diagnostic, DiagnosticSeverity, FoldingRange, FoldingRangeKind, Hover, HoverContents,
    MarkupContent, MarkupKind, Position, Range, SemanticToken, SemanticTokenModifier,
    SemanticTokenType, TextEdit,
};
use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_highlight::{semantic, HighlightKind, HighlightToken};
use sql_dialect_fmt_text::{LineIndex, Utf16Position, Utf8Position};

/// LSP position encoding negotiated with the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionEncoding {
    /// UTF-16 code units, the LSP default.
    Utf16,
    /// UTF-8 byte offsets.
    Utf8,
}

/// The semantic-token type legend, mirrored from the single source of truth in
/// `sql-dialect-fmt-highlight` ([`semantic::SemanticTokenType::LEGEND`]). A token's `token_type` field is
/// an index into this slice, so the order here is the contract with the editor (declared in the
/// server's capabilities) and *must* equal the highlighter's legend — otherwise an editor would
/// decode our token types against the wrong names.
pub fn token_types() -> Vec<SemanticTokenType> {
    semantic::SemanticTokenType::LEGEND
        .iter()
        .map(|ty| SemanticTokenType::new(ty.name()))
        .collect()
}

/// The semantic-token modifier legend, mirrored from
/// [`semantic::SemanticTokenModifiers::LEGEND`]. A token's `token_modifiers_bitset` is decoded
/// bit-by-bit against this slice, so it too must match the highlighter.
pub fn token_modifiers() -> Vec<SemanticTokenModifier> {
    semantic::SemanticTokenModifiers::LEGEND
        .iter()
        .map(|&name| SemanticTokenModifier::new(name))
        .collect()
}

fn lsp_position(index: &LineIndex<'_>, offset: usize, encoding: PositionEncoding) -> Position {
    match encoding {
        PositionEncoding::Utf16 => {
            let position = index.utf16_position(offset);
            Position::new(position.line, position.character)
        }
        PositionEncoding::Utf8 => {
            let position = index.utf8_position(offset);
            Position::new(position.line, position.character)
        }
    }
}

fn lsp_end_position(index: &LineIndex<'_>, encoding: PositionEncoding) -> Position {
    match encoding {
        PositionEncoding::Utf16 => {
            let position = index.end_utf16_position();
            Position::new(position.line, position.character)
        }
        PositionEncoding::Utf8 => {
            let position = index.end_utf8_position();
            Position::new(position.line, position.character)
        }
    }
}

fn lsp_offset(index: &LineIndex<'_>, position: Position, encoding: PositionEncoding) -> usize {
    match encoding {
        PositionEncoding::Utf16 => {
            index.offset_for_utf16_position(Utf16Position::new(position.line, position.character))
        }
        PositionEncoding::Utf8 => {
            index.offset_for_utf8_position(Utf8Position::new(position.line, position.character))
        }
    }
}

/// The edits to apply for `textDocument/formatting`: a single whole-document replacement, or an
/// empty list when the input is already formatted (so the editor records no change).
pub fn format_edits(text: &str, options: &FormatOptions) -> Vec<TextEdit> {
    format_edits_with_encoding(text, options, PositionEncoding::Utf16)
}

/// Encoding-aware variant of [`format_edits`].
pub fn format_edits_with_encoding(
    text: &str,
    options: &FormatOptions,
    encoding: PositionEncoding,
) -> Vec<TextEdit> {
    let formatted = format(text, options);
    if formatted == text {
        return Vec::new();
    }
    let index = LineIndex::new(text);
    vec![TextEdit {
        range: Range::new(Position::new(0, 0), lsp_end_position(&index, encoding)),
        new_text: formatted,
    }]
}

/// Diagnostics for `textDocument/publishDiagnostics`: both lexer and parser errors. Neither stage
/// ever fails, so this is the set of *recovered* errors (empty for clean input).
///
/// Lexer errors (unterminated literals/comments, stray characters) are surfaced too — they are
/// reported by the tokenizer before the parser runs, so the parser-only error list would otherwise
/// miss them. We read them through the highlighter, which already re-exposes the lexer's errors.
/// Each diagnostic's range covers the whole offending token (via its byte `range()`), not a single
/// character, so editors underline the real span.
pub fn diagnostics(text: &str) -> Vec<Diagnostic> {
    diagnostics_with_options(text, &FormatOptions::default(), PositionEncoding::Utf16)
}

/// Encoding-aware variant of [`diagnostics`].
pub fn diagnostics_with_encoding(text: &str, encoding: PositionEncoding) -> Vec<Diagnostic> {
    diagnostics_with_options(text, &FormatOptions::default(), encoding)
}

/// Options- and encoding-aware variant of [`diagnostics`].
pub fn diagnostics_with_options(
    text: &str,
    options: &FormatOptions,
    encoding: PositionEncoding,
) -> Vec<Diagnostic> {
    let index = LineIndex::new(text);
    let to_range = |span: std::ops::Range<usize>| {
        Range::new(
            lsp_position(&index, span.start, encoding),
            lsp_position(&index, span.end, encoding),
        )
    };
    let make = |range: Range, message: String| Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("sql-dialect-fmt".to_string()),
        message,
        ..Default::default()
    };

    let lexed = sql_dialect_fmt_lexer::tokenize_for_dialect(text, options.dialect);
    let lex_errors = lexed.errors.clone();
    let parse = sql_dialect_fmt_parser::parse_lexed(text, options.dialect, lexed);
    let highlighted = sql_dialect_fmt_highlight::highlight(text);
    let mut diagnostics: Vec<_> = lex_errors
        .iter()
        .map(|err| make(to_range(err.range()), err.message.clone()))
        .chain(
            parse
                .errors()
                .iter()
                .map(|err| make(to_range(err.range()), err.message.clone())),
        )
        .collect();
    diagnostics.extend(embedded_language_diagnostics_with_encoding(
        &highlighted.tokens,
        &index,
        encoding,
    ));
    diagnostics.extend(lint_diagnostics_with_encoding(
        &highlighted.tokens,
        &index,
        encoding,
    ));
    diagnostics
}

fn lint_diagnostics_with_encoding(
    tokens: &[HighlightToken<'_>],
    index: &LineIndex<'_>,
    encoding: PositionEncoding,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    diagnostics.extend(select_wildcard_diagnostics(tokens, index, encoding));
    diagnostics.extend(large_in_list_diagnostics(tokens, index, encoding));

    diagnostics
}

fn lint_warning(
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
        source: Some("sql-dialect-fmt".to_string()),
        message: message.to_string(),
        ..Default::default()
    }
}

fn select_wildcard_diagnostics(
    tokens: &[HighlightToken<'_>],
    index: &LineIndex<'_>,
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
                        diagnostics.push(lint_warning(
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

fn is_wildcard_prefix(text: &str) -> bool {
    matches!(text, "," | ".")
        || text.eq_ignore_ascii_case("select")
        || text.eq_ignore_ascii_case("distinct")
        || text.eq_ignore_ascii_case("all")
}

fn large_in_list_diagnostics(
    tokens: &[HighlightToken<'_>],
    index: &LineIndex<'_>,
    encoding: PositionEncoding,
) -> Vec<Diagnostic> {
    const LARGE_IN_LIST_THRESHOLD: usize = 100;

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
                    if !active.possible_subquery && item_count > LARGE_IN_LIST_THRESHOLD {
                        diagnostics.push(lint_warning(
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

fn is_significant(kind: HighlightKind) -> bool {
    !matches!(kind, HighlightKind::Whitespace | HighlightKind::Comment)
}

fn embedded_language_diagnostics_with_encoding(
    tokens: &[HighlightToken<'_>],
    index: &LineIndex<'_>,
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
                            diagnostics.push(Diagnostic {
                                range: Range::new(
                                    lsp_position(index, range.start, encoding),
                                    lsp_position(index, range.end, encoding),
                                ),
                                severity: Some(DiagnosticSeverity::WARNING),
                                source: Some("sql-dialect-fmt".to_string()),
                                message: format!(
                                    "unsupported embedded language {word}; expected SQL, JAVASCRIPT, PYTHON, JAVA, or SCALA"
                                ),
                                ..Default::default()
                            });
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

fn is_supported_embedded_language(word: &str) -> bool {
    ["SQL", "JAVASCRIPT", "PYTHON", "JAVA", "SCALA"]
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(word))
}

/// Hover information for `textDocument/hover`: the keyword/type/symbol description at `position`,
/// rendered as Markdown with an optional docs link, scoped to the hovered token's range.
pub fn hover(text: &str, position: Position) -> Option<Hover> {
    hover_with_encoding(text, position, PositionEncoding::Utf16)
}

/// Encoding-aware variant of [`hover`].
pub fn hover_with_encoding(
    text: &str,
    position: Position,
    encoding: PositionEncoding,
) -> Option<Hover> {
    let index = LineIndex::new(text);
    let info = sql_dialect_fmt_hover::hover_at(text, lsp_offset(&index, position, encoding))?;
    let mut value = format!("**{}**\n\n{}", info.title, info.body);
    if let Some(url) = info.docs_url {
        value.push_str(&format!("\n\n[Snowflake docs]({url})"));
    }
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(Range::new(
            lsp_position(&index, info.range.start, encoding),
            lsp_position(&index, info.range.end, encoding),
        )),
    })
}

/// Folding ranges for `textDocument/foldingRange`: one region per multi-line top-level statement,
/// so an editor can collapse each statement in a script. The CST's root children are the statements.
pub fn folding_ranges(text: &str) -> Vec<FoldingRange> {
    let index = LineIndex::new(text);
    let root = sql_dialect_fmt_parser::parse(text).syntax();
    root.children()
        .filter_map(|stmt| {
            // Use the span of the statement's significant tokens, so a leading/trailing blank line
            // (attached as trivia) doesn't inflate a single-line statement into a foldable region.
            let mut tokens = stmt
                .descendants_with_tokens()
                .filter_map(|el| el.into_token())
                .filter(|t| !t.kind().is_trivia());
            let first = tokens.next()?;
            let last = tokens.last().unwrap_or_else(|| first.clone());
            let start = lsp_position(
                &index,
                first.text_range().start().into(),
                PositionEncoding::Utf16,
            )
            .line;
            let end = lsp_position(
                &index,
                last.text_range().end().into(),
                PositionEncoding::Utf16,
            )
            .line;
            (end > start).then_some(FoldingRange {
                start_line: start,
                end_line: end,
                kind: Some(FoldingRangeKind::Region),
                ..FoldingRange::default()
            })
        })
        .collect()
}

/// Apply one `textDocument/didChange` content change to `text`, returning the new document.
///
/// Supports incremental sync: a `Some(range)` splices `new_text` over the byte span the range
/// covers; a `None` range is a whole-document replacement (the editor sent the full text). The
/// range is treated as ordered and clamped to the document, so a malformed event can't panic.
pub fn apply_change(text: &str, range: Option<Range>, new_text: &str) -> String {
    apply_change_with_encoding(text, range, new_text, PositionEncoding::Utf16)
}

/// Encoding-aware variant of [`apply_change`].
pub fn apply_change_with_encoding(
    text: &str,
    range: Option<Range>,
    new_text: &str,
    encoding: PositionEncoding,
) -> String {
    let Some(range) = range else {
        return new_text.to_string();
    };
    let index = LineIndex::new(text);
    let a = lsp_offset(&index, range.start, encoding);
    let b = lsp_offset(&index, range.end, encoding);
    let (start, end) = (a.min(b), a.max(b));
    let mut out = String::with_capacity(text.len() - (end - start) + new_text.len());
    out.push_str(&text[..start]);
    out.push_str(new_text);
    out.push_str(&text[end..]);
    out
}

/// The delta-encoded semantic tokens for `textDocument/semanticTokens/full`.
///
/// This is a thin adapter over `sql-dialect-fmt-highlight`'s [`semantic::semantic_tokens_lsp`]: the
/// highlighter already splits multi-line tokens (block comments, dollar-quoted strings) into one
/// token per line — the LSP encoding requires each token to stay on a single line — computes UTF-16
/// columns/lengths, and delta-encodes into `(deltaLine, deltaStartChar, length, tokenType,
/// tokenModifiers)` quintuples against the same legend the server advertises. We only reshape each
/// quintuple into an `lsp_types::SemanticToken`, **preserving** the modifier bitset (rather than
/// hardcoding 0) so `defaultLibrary` / `documentation` modifiers reach the editor.
pub fn semantic_tokens(text: &str) -> Vec<SemanticToken> {
    semantic_tokens_with_encoding(text, PositionEncoding::Utf16)
}

/// Encoding-aware variant of [`semantic_tokens`].
pub fn semantic_tokens_with_encoding(text: &str, encoding: PositionEncoding) -> Vec<SemanticToken> {
    let raw = match encoding {
        PositionEncoding::Utf16 => semantic::semantic_tokens_lsp(text),
        PositionEncoding::Utf8 => sql_dialect_fmt_highlight::semantic_tokens_lsp_utf8(text),
    };
    raw.into_iter()
        .map(
            |[delta_line, delta_start, length, token_type, token_modifiers_bitset]| SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers_bitset,
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_maps_offsets_to_utf16_positions() {
        let text = "SELECT a\nFROM 芋;\n"; // 芋 is one UTF-16 unit but 3 bytes
        let index = LineIndex::new(text);
        assert_eq!(
            lsp_position(&index, 0, PositionEncoding::Utf16),
            Position::new(0, 0)
        );
        assert_eq!(
            lsp_position(&index, 7, PositionEncoding::Utf16),
            Position::new(0, 7)
        ); // the `a`
        let from = text.find("FROM").unwrap();
        assert_eq!(
            lsp_position(&index, from, PositionEncoding::Utf16),
            Position::new(1, 0)
        );
        let semicolon = text.find(';').unwrap();
        assert_eq!(
            lsp_position(&index, semicolon, PositionEncoding::Utf16),
            Position::new(1, 6)
        ); // FROM<sp>芋 = 6 utf16 units
    }

    #[test]
    fn line_index_maps_offsets_to_utf8_positions() {
        let text = "SELECT a\nFROM 芋;\n";
        let index = LineIndex::new(text);
        let semicolon = text.find(';').unwrap();
        assert_eq!(
            lsp_position(&index, semicolon, PositionEncoding::Utf8),
            Position::new(1, 8)
        ); // FROM<sp>芋 = 8 utf8 bytes
    }

    #[test]
    fn formatting_replaces_the_whole_document() {
        let edits = format_edits("select a,b from t", &FormatOptions::default());
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "SELECT a, b\nFROM t;\n");
        assert_eq!(edits[0].range.start, Position::new(0, 0));
    }

    #[test]
    fn already_formatted_input_yields_no_edits() {
        let formatted = "SELECT a, b\nFROM t;\n";
        assert!(format_edits(formatted, &FormatOptions::default()).is_empty());
    }

    #[test]
    fn clean_sql_has_no_diagnostics() {
        assert!(diagnostics("select 1").is_empty());
    }

    #[test]
    fn broken_sql_reports_a_diagnostic() {
        let diags = diagnostics("select from where");
        assert!(!diags.is_empty());
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn lexer_errors_reach_diagnostics() {
        // An unterminated string is a *lexer* error (the parser never sees it as a token boundary
        // problem). It must still surface as a diagnostic, and its range must cover the whole
        // unterminated literal, not a single character.
        let text = "SELECT 'oops";
        let diags = diagnostics(text);
        let lex_diag = diags
            .iter()
            .find(|d| d.message.contains("unterminated string"))
            .expect("an unterminated-string diagnostic");
        assert_eq!(lex_diag.severity, Some(DiagnosticSeverity::ERROR));
        let quote = text.find('\'').unwrap() as u32;
        assert_eq!(lex_diag.range.start, Position::new(0, quote));
        // Spans to end of the line (the whole literal), so end column > start column.
        assert!(lex_diag.range.end.character > lex_diag.range.start.character);
    }

    #[test]
    fn parser_diagnostic_range_covers_the_token() {
        // `MERGE tgt ...` (no INTO) reports "expected INTO" at the `tgt` token; the LSP range must
        // span the whole 3-character identifier, not one character.
        let text = "MERGE tgt USING src ON a = b";
        let diags = diagnostics(text);
        let into = diags
            .iter()
            .find(|d| d.message == "expected INTO")
            .expect("an INTO diagnostic");
        let col = text.find("tgt").unwrap() as u32;
        assert_eq!(into.range.start, Position::new(0, col));
        assert_eq!(into.range.end, Position::new(0, col + 3));
    }

    #[test]
    fn clean_sql_still_has_no_lexer_or_parser_diagnostics() {
        assert!(diagnostics("SELECT a FROM t").is_empty());
    }

    #[test]
    fn unsupported_embedded_language_is_a_warning() {
        let text = "CREATE FUNCTION f() RETURNS STRING LANGUAGE RUBY AS $$x$$;";
        let diags = diagnostics(text);
        let language = diags
            .iter()
            .find(|d| d.message.contains("unsupported embedded language RUBY"))
            .expect("unsupported-language diagnostic");
        assert_eq!(language.severity, Some(DiagnosticSeverity::WARNING));
        let col = text.find("RUBY").unwrap() as u32;
        assert_eq!(language.range.start, Position::new(0, col));
        assert_eq!(language.range.end, Position::new(0, col + 4));
    }

    #[test]
    fn embedded_language_warning_does_not_fire_for_plain_columns_or_dynamic_sql() {
        for text in [
            "SELECT language FROM t;",
            "EXECUTE IMMEDIATE $$ SELECT 1 $$;",
        ] {
            assert!(
                diagnostics(text)
                    .iter()
                    .all(|diag| !diag.message.contains("unsupported embedded language")),
                "{text}"
            );
        }
    }

    #[test]
    fn select_wildcard_lint_is_a_warning() {
        let text = "SELECT * FROM t;";
        let diags = diagnostics(text);
        let wildcard = diags
            .iter()
            .find(|d| d.message.contains("avoid SELECT *"))
            .expect("SELECT * warning");
        assert_eq!(wildcard.severity, Some(DiagnosticSeverity::WARNING));
        let col = text.find('*').unwrap() as u32;
        assert_eq!(wildcard.range.start, Position::new(0, col));
        assert_eq!(wildcard.range.end, Position::new(0, col + 1));
    }

    #[test]
    fn select_wildcard_lint_ignores_function_stars() {
        let text = "SELECT count(*) FROM t;";
        assert!(
            diagnostics(text)
                .iter()
                .all(|diag| !diag.message.contains("avoid SELECT *")),
            "{text}"
        );
    }

    #[test]
    fn large_in_list_lint_is_a_warning() {
        let values = (0..101)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!("SELECT id FROM t WHERE id IN ({values});");
        let diags = diagnostics(&text);
        let in_list = diags
            .iter()
            .find(|d| d.message.contains("large IN list"))
            .expect("large IN-list warning");
        assert_eq!(in_list.severity, Some(DiagnosticSeverity::WARNING));
        let col = text.find("IN").unwrap() as u32;
        assert_eq!(in_list.range.start, Position::new(0, col));
    }

    #[test]
    fn normal_in_list_and_subquery_have_no_lint() {
        for text in [
            "SELECT id FROM t WHERE id IN (1, 2, 3);",
            "SELECT id FROM t WHERE id IN (SELECT id FROM src);",
        ] {
            assert!(
                diagnostics(text)
                    .iter()
                    .all(|diag| !diag.message.contains("large IN list")),
                "{text}"
            );
        }
    }

    #[test]
    fn offset_is_the_inverse_of_position() {
        let text = "SELECT a\nFROM 芋;\n";
        let index = LineIndex::new(text);
        for offset in [
            0usize,
            7,
            text.find("FROM").unwrap(),
            text.find(';').unwrap(),
        ] {
            assert_eq!(
                lsp_offset(
                    &index,
                    lsp_position(&index, offset, PositionEncoding::Utf16),
                    PositionEncoding::Utf16
                ),
                offset
            );
        }
    }

    #[test]
    fn hover_describes_a_type() {
        // Hover over the `varchar` cast target should return a Snowflake type description.
        let src = "select x::varchar from t";
        let col = src.find("varchar").unwrap() as u32;
        let hover = hover(src, Position::new(0, col)).expect("hover");
        assert!(hover.range.is_some());
        match hover.contents {
            HoverContents::Markup(m) => assert!(m.value.to_lowercase().contains("varchar")),
            _ => panic!("expected markup"),
        }
    }

    #[test]
    fn apply_change_splices_an_incremental_edit() {
        // Replace "world" (line 1, cols 0..5) with "snow".
        let text = "hello\nworld\n";
        let range = Range::new(Position::new(1, 0), Position::new(1, 5));
        assert_eq!(apply_change(text, Some(range), "snow"), "hello\nsnow\n");
    }

    #[test]
    fn apply_change_splices_utf8_encoded_ranges() {
        let text = "SELECT '長芋'\nFROM t\n";
        let start = Position::new(0, "SELECT '長".len() as u32);
        let end = Position::new(0, "SELECT '長芋".len() as u32);
        assert_eq!(
            apply_change_with_encoding(
                text,
                Some(Range::new(start, end)),
                "山芋",
                PositionEncoding::Utf8
            ),
            "SELECT '長山芋'\nFROM t\n"
        );
    }

    #[test]
    fn apply_change_with_no_range_replaces_whole_document() {
        assert_eq!(apply_change("old", None, "new text"), "new text");
    }

    #[test]
    fn folding_ranges_cover_multiline_statements() {
        let ranges = folding_ranges("select a,\nb\nfrom t;\n\nselect 1;");
        assert_eq!(ranges.len(), 1); // only the first (multi-line) statement folds
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 2);
    }

    #[test]
    fn semantic_tokens_tag_keywords() {
        let tokens = semantic_tokens("select a from t");
        assert!(!tokens.is_empty());
        // The first token is `select`, a keyword (legend index 0), at line 0 column 0.
        assert_eq!(tokens[0].delta_line, 0);
        assert_eq!(tokens[0].delta_start, 0);
        assert_eq!(tokens[0].length, 6);
        assert_eq!(tokens[0].token_type, 0);
        // The keyword carries the `defaultLibrary` modifier — it must not be hardcoded to 0.
        assert_eq!(
            tokens[0].token_modifiers_bitset,
            semantic::SemanticTokenModifiers::DEFAULT_LIBRARY.bits()
        );
    }

    #[test]
    fn server_legend_equals_the_highlighter_legend() {
        // The advertised type legend must be exactly the highlighter's LEGEND, in order — this is
        // the contract an editor decodes `token_type` against.
        let advertised: Vec<String> = token_types()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect();
        let expected: Vec<String> = semantic::SemanticTokenType::LEGEND
            .iter()
            .map(|t| t.name().to_string())
            .collect();
        assert_eq!(advertised, expected);
        // It includes `function` (currently the last appended type) for Cortex/AISQL recognition.
        assert_eq!(advertised.last().map(String::as_str), Some("function"));

        // Likewise the modifier legend mirrors the highlighter's.
        let mods: Vec<String> = token_modifiers()
            .iter()
            .map(|m| m.as_str().to_string())
            .collect();
        assert_eq!(mods, vec!["documentation", "defaultLibrary"]);
    }

    #[test]
    fn semantic_tokens_are_monotonic_and_never_panic_on_multiline() {
        // A multi-line block comment must split into per-line tokens without panicking.
        let tokens = semantic_tokens("select 1 /* a\nb */ from t");
        // Deltas must be non-negative by construction (u32) and the stream stays consistent.
        assert!(tokens.iter().all(|t| t.length > 0));
    }
}
