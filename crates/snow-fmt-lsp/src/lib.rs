//! LSP feature logic for Snowflake SQL: formatting edits, parse diagnostics, and semantic tokens.
//!
//! This module is deliberately transport-free (it never touches stdio or an `lsp-server`
//! connection), so every feature is a pure `&str -> data` function that is unit-testable. The
//! binary ([`crate::main`]) is the thin adapter that wires these into a language server.
//!
//! Positions follow the LSP convention: zero-based lines and **UTF-16** column offsets.

use lsp_types::{
    Diagnostic, DiagnosticSeverity, FoldingRange, FoldingRangeKind, Hover, HoverContents,
    MarkupContent, MarkupKind, Position, Range, SemanticToken, SemanticTokenModifier,
    SemanticTokenType, TextEdit,
};
use snow_fmt_formatter::{format, FormatOptions};
use snow_fmt_highlight::{semantic, HighlightKind};

/// The semantic-token type legend, mirrored from the single source of truth in
/// `snow-fmt-highlight` ([`semantic::SemanticTokenType::LEGEND`]). A token's `token_type` field is
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

/// Maps byte offsets into a document to LSP [`Position`]s (UTF-16 columns).
pub struct LineIndex<'a> {
    text: &'a str,
    /// Byte offset of the start of each line.
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        );
        LineIndex { text, line_starts }
    }

    /// The LSP position of a byte `offset` (clamped to the document end).
    pub fn position(&self, offset: usize) -> Position {
        let offset = offset.min(self.text.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        };
        let line_start = self.line_starts[line];
        let col: usize = self.text[line_start..offset]
            .chars()
            .map(char::len_utf16)
            .sum();
        Position::new(line as u32, col as u32)
    }

    /// The position one past the last character — the end of the document.
    pub fn end(&self) -> Position {
        self.position(self.text.len())
    }

    /// The byte offset of an LSP [`Position`] (the inverse of [`Self::position`]). Out-of-range
    /// lines/columns clamp to the line or document end.
    pub fn offset(&self, position: Position) -> usize {
        let line = position.line as usize;
        let Some(&line_start) = self.line_starts.get(line) else {
            return self.text.len();
        };
        let mut remaining = position.character as usize; // UTF-16 units to consume
        let mut offset = line_start;
        for ch in self.text[line_start..].chars() {
            let width = ch.len_utf16();
            if remaining < width || ch == '\n' {
                break;
            }
            remaining -= width;
            offset += ch.len_utf8();
        }
        offset
    }
}

/// The edits to apply for `textDocument/formatting`: a single whole-document replacement, or an
/// empty list when the input is already formatted (so the editor records no change).
pub fn format_edits(text: &str, options: &FormatOptions) -> Vec<TextEdit> {
    let formatted = format(text, options);
    if formatted == text {
        return Vec::new();
    }
    let index = LineIndex::new(text);
    vec![TextEdit {
        range: Range::new(Position::new(0, 0), index.end()),
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
    let index = LineIndex::new(text);
    let to_range = |span: std::ops::Range<usize>| {
        Range::new(index.position(span.start), index.position(span.end))
    };
    let make = |range: Range, message: String| Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("snow-fmt".to_string()),
        message,
        ..Default::default()
    };

    let lex_errors = snow_fmt_highlight::highlight(text).errors;
    let parse = snow_fmt_parser::parse(text);
    let mut diagnostics: Vec<_> = lex_errors
        .into_iter()
        .map(|err| make(to_range(err.range()), err.message))
        .chain(
            parse
                .errors()
                .iter()
                .map(|err| make(to_range(err.range()), err.message.clone())),
        )
        .collect();
    diagnostics.extend(embedded_language_diagnostics(text, &index));
    diagnostics
}

fn embedded_language_diagnostics(text: &str, index: &LineIndex<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut expect_language_name = false;
    let mut language_name: Option<(&str, std::ops::Range<usize>)> = None;
    let mut saw_as_after_language = false;

    for token in snow_fmt_highlight::highlight(text).tokens {
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
                                range: Range::new(index.position(range.start), index.position(range.end)),
                                severity: Some(DiagnosticSeverity::WARNING),
                                source: Some("snow-fmt".to_string()),
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
                language_name = Some((token.text, token.range));
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
    let index = LineIndex::new(text);
    let info = snow_fmt_hover::hover_at(text, index.offset(position))?;
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
            index.position(info.range.start),
            index.position(info.range.end),
        )),
    })
}

/// Folding ranges for `textDocument/foldingRange`: one region per multi-line top-level statement,
/// so an editor can collapse each statement in a script. The CST's root children are the statements.
pub fn folding_ranges(text: &str) -> Vec<FoldingRange> {
    let index = LineIndex::new(text);
    let root = snow_fmt_parser::parse(text).syntax();
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
            let start = index.position(first.text_range().start().into()).line;
            let end = index.position(last.text_range().end().into()).line;
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
    let Some(range) = range else {
        return new_text.to_string();
    };
    let index = LineIndex::new(text);
    let a = index.offset(range.start);
    let b = index.offset(range.end);
    let (start, end) = (a.min(b), a.max(b));
    let mut out = String::with_capacity(text.len() - (end - start) + new_text.len());
    out.push_str(&text[..start]);
    out.push_str(new_text);
    out.push_str(&text[end..]);
    out
}

/// The delta-encoded semantic tokens for `textDocument/semanticTokens/full`.
///
/// This is a thin adapter over `snow-fmt-highlight`'s [`semantic::semantic_tokens_lsp`]: the
/// highlighter already splits multi-line tokens (block comments, dollar-quoted strings) into one
/// token per line — the LSP encoding requires each token to stay on a single line — computes UTF-16
/// columns/lengths, and delta-encodes into `(deltaLine, deltaStartChar, length, tokenType,
/// tokenModifiers)` quintuples against the same legend the server advertises. We only reshape each
/// quintuple into an `lsp_types::SemanticToken`, **preserving** the modifier bitset (rather than
/// hardcoding 0) so `defaultLibrary` / `documentation` modifiers reach the editor.
pub fn semantic_tokens(text: &str) -> Vec<SemanticToken> {
    semantic::semantic_tokens_lsp(text)
        .into_iter()
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
        assert_eq!(index.position(0), Position::new(0, 0));
        assert_eq!(index.position(7), Position::new(0, 7)); // the `a`
        let from = text.find("FROM").unwrap();
        assert_eq!(index.position(from), Position::new(1, 0));
        let semicolon = text.find(';').unwrap();
        assert_eq!(index.position(semicolon), Position::new(1, 6)); // FROM<sp>芋 = 6 utf16 units
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
    fn offset_is_the_inverse_of_position() {
        let text = "SELECT a\nFROM 芋;\n";
        let index = LineIndex::new(text);
        for offset in [
            0usize,
            7,
            text.find("FROM").unwrap(),
            text.find(';').unwrap(),
        ] {
            assert_eq!(index.offset(index.position(offset)), offset);
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
        // It includes `namespace` (index 8) — the type the old LSP legend was missing.
        assert_eq!(advertised.last().map(String::as_str), Some("namespace"));

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
