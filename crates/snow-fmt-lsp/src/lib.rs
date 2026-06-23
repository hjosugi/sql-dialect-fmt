//! LSP feature logic for Snowflake SQL: formatting edits, parse diagnostics, and semantic tokens.
//!
//! This module is deliberately transport-free (it never touches stdio or an `lsp-server`
//! connection), so every feature is a pure `&str -> data` function that is unit-testable. The
//! binary ([`crate::main`]) is the thin adapter that wires these into a language server.
//!
//! Positions follow the LSP convention: zero-based lines and **UTF-16** column offsets.

use lsp_types::{
    Diagnostic, DiagnosticSeverity, FoldingRange, FoldingRangeKind, Hover, HoverContents,
    MarkupContent, MarkupKind, Position, Range, SemanticToken, SemanticTokenType, TextEdit,
};
use snow_fmt_formatter::{format, FormatOptions};
use snow_fmt_highlight::HighlightKind;

/// The semantic-token legend. A token's `token_type` field is an index into this slice, so the
/// order here is the contract with the editor (declared in the server's capabilities).
pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,   // 0
    SemanticTokenType::TYPE,      // 1
    SemanticTokenType::VARIABLE,  // 2  identifiers (plain / quoted)
    SemanticTokenType::STRING,    // 3  string + dollar-quoted
    SemanticTokenType::NUMBER,    // 4
    SemanticTokenType::PARAMETER, // 5  bind / positional variables ($1, :name)
    SemanticTokenType::OPERATOR,  // 6
    SemanticTokenType::COMMENT,   // 7
];

/// The legend index for a highlight kind, or `None` for kinds that carry no semantic token
/// (whitespace, punctuation, and lex errors — the latter are surfaced as diagnostics instead).
fn token_type(kind: HighlightKind) -> Option<u32> {
    Some(match kind {
        HighlightKind::Keyword => 0,
        HighlightKind::Type => 1,
        HighlightKind::Identifier | HighlightKind::QuotedIdentifier => 2,
        HighlightKind::String | HighlightKind::DollarString => 3,
        HighlightKind::Number => 4,
        HighlightKind::Variable => 5,
        HighlightKind::Operator => 6,
        HighlightKind::Comment => 7,
        HighlightKind::Whitespace | HighlightKind::Punctuation | HighlightKind::Error => {
            return None
        }
    })
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

/// Parse diagnostics for `textDocument/publishDiagnostics`. The parser never fails, so this is the
/// set of recovered errors (empty for clean input).
pub fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let parse = snow_fmt_parser::parse(text);
    let index = LineIndex::new(text);
    parse
        .errors()
        .iter()
        .map(|err| Diagnostic {
            range: Range::new(index.position(err.offset), index.position(err.offset + 1)),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("snow-fmt".to_string()),
            message: err.message.clone(),
            ..Default::default()
        })
        .collect()
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

/// The delta-encoded semantic tokens for `textDocument/semanticTokens/full`. Multi-line tokens
/// (block comments, dollar-quoted strings) are split into one token per line, as the LSP encoding
/// requires each token to stay on a single line.
pub fn semantic_tokens(text: &str) -> Vec<SemanticToken> {
    let highlighted = snow_fmt_highlight::highlight(text);
    let index = LineIndex::new(text);
    let mut tokens = Vec::new();
    let (mut prev_line, mut prev_col) = (0u32, 0u32);

    for token in &highlighted.tokens {
        let Some(token_type) = token_type(token.kind) else {
            continue;
        };
        // Walk each line-piece of the (possibly multi-line) token at its real byte offset.
        let mut piece_start = token.range.start;
        for piece in token.text.split('\n') {
            let length: u32 = piece.chars().map(|c| c.len_utf16() as u32).sum();
            if length > 0 {
                let pos = index.position(piece_start);
                let delta_line = pos.line - prev_line;
                let delta_start = if delta_line == 0 {
                    pos.character - prev_col
                } else {
                    pos.character
                };
                tokens.push(SemanticToken {
                    delta_line,
                    delta_start,
                    length,
                    token_type,
                    token_modifiers_bitset: 0,
                });
                (prev_line, prev_col) = (pos.line, pos.character);
            }
            piece_start += piece.len() + 1; // skip the piece and its trailing '\n'
        }
    }
    tokens
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
    }

    #[test]
    fn semantic_tokens_are_monotonic_and_never_panic_on_multiline() {
        // A multi-line block comment must split into per-line tokens without panicking.
        let tokens = semantic_tokens("select 1 /* a\nb */ from t");
        // Deltas must be non-negative by construction (u32) and the stream stays consistent.
        assert!(tokens.iter().all(|t| t.length > 0));
    }
}
