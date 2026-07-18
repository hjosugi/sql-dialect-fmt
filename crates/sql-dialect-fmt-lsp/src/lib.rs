//! LSP feature logic for Snowflake SQL: formatting, diagnostics, hover, folding, document symbols,
//! completion, and semantic tokens.
//!
//! This module is deliberately transport-free (it never touches stdio or an `lsp-server`
//! connection), so every feature is a pure `&str -> data` function that is unit-testable. The
//! binary crate is the thin adapter that wires these into a language server.
//!
//! Positions follow the LSP convention: zero-based lines and **UTF-16** column offsets.

use lsp_types::{
    CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity, DocumentSymbol,
    FoldingRange, FoldingRangeKind, Hover, HoverContents, InsertTextFormat, MarkupContent,
    MarkupKind, Position, Range, SemanticToken, SemanticTokenModifier, SemanticTokenType,
    SymbolKind, TextEdit,
};
use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_highlight::semantic;
use sql_dialect_fmt_parser::{SyntaxKind, SyntaxNode};
use sql_dialect_fmt_syntax::{keyword_texts, BUILTIN_TYPE_WORDS};
use sql_dialect_fmt_text::{LineIndex, Utf16Position, Utf8Position};

mod lint;

pub use lint::{diagnostic_lint_code, LintCode, LintOptions};

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

/// The edits to apply for `textDocument/rangeFormatting`: reformat only the statements the range
/// touches, as one minimal replacement, or an empty list when nothing changes.
pub fn format_range_edits(text: &str, range: Range, options: &FormatOptions) -> Vec<TextEdit> {
    format_range_edits_with_encoding(text, range, options, PositionEncoding::Utf16)
}

/// Encoding-aware variant of [`format_range_edits`].
pub fn format_range_edits_with_encoding(
    text: &str,
    range: Range,
    options: &FormatOptions,
    encoding: PositionEncoding,
) -> Vec<TextEdit> {
    let index = LineIndex::new(text);
    let a = lsp_offset(&index, range.start, encoding);
    let b = lsp_offset(&index, range.end, encoding);
    let target = a.min(b)..a.max(b);
    let Some(edit) = sql_dialect_fmt_formatter::format_range(text, target, options) else {
        return Vec::new();
    };
    vec![TextEdit {
        range: Range::new(
            lsp_position(&index, edit.range.start, encoding),
            lsp_position(&index, edit.range.end, encoding),
        ),
        new_text: edit.new_text,
    }]
}

/// The edits to apply for `textDocument/onTypeFormatting`: after the user types `;` or a newline,
/// reformat the statement that just ended.
pub fn on_type_formatting_edits(
    text: &str,
    position: Position,
    options: &FormatOptions,
) -> Vec<TextEdit> {
    on_type_formatting_edits_with_encoding(text, position, options, PositionEncoding::Utf16)
}

/// Encoding-aware variant of [`on_type_formatting_edits`].
///
/// The trigger position is scanned backwards over whitespace to the completed statement's final
/// non-whitespace byte, so both `;` (cursor right after the terminator) and newline (cursor at the
/// start of the next line) resolve to the statement the user just finished. An already formatted
/// statement yields no edits.
pub fn on_type_formatting_edits_with_encoding(
    text: &str,
    position: Position,
    options: &FormatOptions,
    encoding: PositionEncoding,
) -> Vec<TextEdit> {
    let index = LineIndex::new(text);
    let cursor = lsp_offset(&index, position, encoding);
    let Some(last) = text[..cursor].rfind(|c: char| !c.is_whitespace()) else {
        return Vec::new();
    };
    // `last..cursor` starts on a char boundary and covers the statement's final byte, so the
    // statement intersects the range; the trailing whitespace bytes touch no other statement.
    let Some(edit) = sql_dialect_fmt_formatter::format_range(text, last..cursor, options) else {
        return Vec::new();
    };
    vec![TextEdit {
        range: Range::new(
            lsp_position(&index, edit.range.start, encoding),
            lsp_position(&index, edit.range.end, encoding),
        ),
        new_text: edit.new_text,
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
    diagnostics_with_lint_options(text, options, LintOptions::default(), encoding)
}

/// Options-, lint-, and encoding-aware variant of [`diagnostics`].
pub fn diagnostics_with_lint_options(
    text: &str,
    options: &FormatOptions,
    lint_options: LintOptions,
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
    diagnostics.extend(lint::diagnostics_with_encoding(
        text,
        &highlighted.tokens,
        &index,
        lint_options,
        encoding,
    ));
    diagnostics
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

/// Document symbols for `textDocument/documentSymbol`: one outline item per top-level statement.
pub fn document_symbols(text: &str, options: &FormatOptions) -> Vec<DocumentSymbol> {
    document_symbols_with_encoding(text, options, PositionEncoding::Utf16)
}

/// Encoding-aware variant of [`document_symbols`].
pub fn document_symbols_with_encoding(
    text: &str,
    options: &FormatOptions,
    encoding: PositionEncoding,
) -> Vec<DocumentSymbol> {
    let index = LineIndex::new(text);
    let root = sql_dialect_fmt_parser::parse_with_dialect(text, options.dialect).syntax();
    root.children()
        .filter_map(|stmt| document_symbol_for_statement(&stmt, &index, encoding))
        .collect()
}

#[derive(Clone, Debug)]
struct SymbolToken {
    kind: SyntaxKind,
    text: String,
    range: std::ops::Range<usize>,
}

fn document_symbol_for_statement(
    stmt: &SyntaxNode,
    index: &LineIndex<'_>,
    encoding: PositionEncoding,
) -> Option<DocumentSymbol> {
    let tokens = significant_symbol_tokens(stmt);
    let first = tokens.first()?;
    let last = tokens.last().unwrap_or(first);
    let range = byte_range_to_lsp(index, first.range.start..last.range.end, encoding);
    let selection_range = statement_selection_range(&tokens)
        .map(|range| byte_range_to_lsp(index, range, encoding))
        .unwrap_or(range);
    let name = statement_symbol_name(stmt.kind(), &tokens);
    let kind = statement_symbol_kind(stmt.kind(), &tokens);
    #[allow(deprecated)]
    let symbol = DocumentSymbol {
        name,
        detail: None,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range,
        children: None,
    };
    Some(symbol)
}

fn significant_symbol_tokens(node: &SyntaxNode) -> Vec<SymbolToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| {
            let range = token.text_range();
            let start: usize = range.start().into();
            let end: usize = range.end().into();
            SymbolToken {
                kind: token.kind(),
                text: token.text().to_string(),
                range: start..end,
            }
        })
        .collect()
}

fn byte_range_to_lsp(
    index: &LineIndex<'_>,
    range: std::ops::Range<usize>,
    encoding: PositionEncoding,
) -> Range {
    Range::new(
        lsp_position(index, range.start, encoding),
        lsp_position(index, range.end, encoding),
    )
}

fn statement_selection_range(tokens: &[SymbolToken]) -> Option<std::ops::Range<usize>> {
    create_name_range(tokens).or_else(|| tokens.first().map(|token| token.range.clone()))
}

fn statement_symbol_name(kind: SyntaxKind, tokens: &[SymbolToken]) -> String {
    if let Some(label) = create_statement_label(tokens) {
        return label;
    }

    match kind {
        SyntaxKind::SELECT_STMT => "SELECT".to_string(),
        SyntaxKind::WITH_QUERY => "WITH".to_string(),
        SyntaxKind::INSERT_STMT => statement_words_until(tokens, 4, &["VALUES", "SELECT"]),
        SyntaxKind::UPDATE_STMT => statement_words_until(tokens, 3, &["SET"]),
        SyntaxKind::DELETE_STMT => statement_words_until(tokens, 4, &["WHERE", "USING"]),
        SyntaxKind::MERGE_STMT => statement_words_until(tokens, 4, &["USING"]),
        SyntaxKind::COPY_STMT => statement_words_until(tokens, 4, &["FROM", "FILES"]),
        SyntaxKind::STAGE_FILE_STMT => statement_words_until(tokens, 1, &[]),
        SyntaxKind::CALL_STMT => statement_words_until(tokens, 3, &["("]),
        SyntaxKind::BLOCK_STMT => "BEGIN".to_string(),
        SyntaxKind::SET_OP | SyntaxKind::FLOW_STMT => statement_words_until(tokens, 3, &[]),
        _ => statement_words_until(tokens, 4, &[";", "AS", "WHERE"]),
    }
}

fn statement_symbol_kind(kind: SyntaxKind, tokens: &[SymbolToken]) -> SymbolKind {
    if let Some((object_kind, _)) = create_object_kind(tokens) {
        return match object_kind {
            "TABLE" | "DYNAMIC TABLE" => SymbolKind::STRUCT,
            "VIEW" | "SEMANTIC VIEW" => SymbolKind::INTERFACE,
            "FUNCTION" => SymbolKind::FUNCTION,
            "PROCEDURE" => SymbolKind::METHOD,
            "TASK" => SymbolKind::EVENT,
            "WAREHOUSE" | "STAGE" | "FILE FORMAT" | "STREAM" | "SEQUENCE" => SymbolKind::OBJECT,
            "SCHEMA" | "DATABASE" => SymbolKind::NAMESPACE,
            _ => SymbolKind::OBJECT,
        };
    }

    match kind {
        SyntaxKind::SELECT_STMT | SyntaxKind::WITH_QUERY | SyntaxKind::SET_OP => {
            SymbolKind::FUNCTION
        }
        SyntaxKind::INSERT_STMT
        | SyntaxKind::UPDATE_STMT
        | SyntaxKind::DELETE_STMT
        | SyntaxKind::MERGE_STMT
        | SyntaxKind::COPY_STMT
        | SyntaxKind::STAGE_FILE_STMT
        | SyntaxKind::CALL_STMT
        | SyntaxKind::SET_STMT
        | SyntaxKind::EXECUTE_STMT => SymbolKind::METHOD,
        SyntaxKind::BLOCK_STMT | SyntaxKind::IF_STMT | SyntaxKind::LOOP_STMT => SymbolKind::MODULE,
        _ => SymbolKind::OBJECT,
    }
}

fn create_statement_label(tokens: &[SymbolToken]) -> Option<String> {
    let (object_kind, name_start) = create_object_kind(tokens)?;
    let name = dotted_name(tokens, skip_if_not_exists(tokens, name_start));
    Some(match name {
        Some(name) => format!("CREATE {object_kind} {name}"),
        None => format!("CREATE {object_kind}"),
    })
}

fn create_name_range(tokens: &[SymbolToken]) -> Option<std::ops::Range<usize>> {
    let (_, name_start) = create_object_kind(tokens)?;
    tokens
        .iter()
        .skip(skip_if_not_exists(tokens, name_start))
        .find(|token| is_name_token(token))
        .map(|token| token.range.clone())
}

fn create_object_kind(tokens: &[SymbolToken]) -> Option<(&'static str, usize)> {
    let create_index = tokens.iter().position(|token| word(token, "CREATE"))?;
    for index in create_index + 1..tokens.len() {
        let token = &tokens[index];
        if word(token, "DYNAMIC") && tokens.get(index + 1).is_some_and(|t| word(t, "TABLE")) {
            return Some(("DYNAMIC TABLE", index + 2));
        }
        if word(token, "SEMANTIC") && tokens.get(index + 1).is_some_and(|t| word(t, "VIEW")) {
            return Some(("SEMANTIC VIEW", index + 2));
        }
        if word(token, "FILE") && tokens.get(index + 1).is_some_and(|t| word(t, "FORMAT")) {
            return Some(("FILE FORMAT", index + 2));
        }
        if word(token, "MASKING") && tokens.get(index + 1).is_some_and(|t| word(t, "POLICY")) {
            return Some(("MASKING POLICY", index + 2));
        }
        if word(token, "ACCESS") && tokens.get(index + 1).is_some_and(|t| word(t, "POLICY")) {
            return Some(("ACCESS POLICY", index + 2));
        }

        for candidate in [
            "TABLE",
            "VIEW",
            "FUNCTION",
            "PROCEDURE",
            "TASK",
            "WAREHOUSE",
            "STAGE",
            "SCHEMA",
            "DATABASE",
            "SEQUENCE",
            "STREAM",
        ] {
            if word(token, candidate) {
                return Some((candidate, index + 1));
            }
        }
    }
    None
}

fn skip_if_not_exists(tokens: &[SymbolToken], index: usize) -> usize {
    if tokens.get(index).is_some_and(|token| word(token, "IF"))
        && tokens
            .get(index + 1)
            .is_some_and(|token| word(token, "NOT"))
        && tokens
            .get(index + 2)
            .is_some_and(|token| word(token, "EXISTS"))
    {
        index + 3
    } else {
        index
    }
}

fn dotted_name(tokens: &[SymbolToken], start: usize) -> Option<String> {
    let mut out = String::new();
    let mut saw_name = false;
    let mut allow_dot = false;

    for token in tokens.iter().skip(start) {
        if token.text == "." && allow_dot {
            out.push('.');
            allow_dot = false;
            continue;
        }
        if is_name_token(token) {
            out.push_str(&token.text);
            saw_name = true;
            allow_dot = true;
            continue;
        }
        break;
    }

    while out.ends_with('.') {
        out.pop();
    }
    saw_name.then_some(out)
}

fn statement_words_until(tokens: &[SymbolToken], max_words: usize, stops: &[&str]) -> String {
    let mut words = Vec::new();
    for token in tokens.iter().filter(|token| symbol_word(token)) {
        if !words.is_empty() && stops.iter().any(|stop| word(token, stop)) {
            break;
        }
        words.push(display_word(token));
        if words.len() == max_words {
            break;
        }
    }
    if words.is_empty() {
        "SQL".to_string()
    } else {
        words.join(" ")
    }
}

fn display_word(token: &SymbolToken) -> String {
    if token.kind.is_keyword() || token.kind == SyntaxKind::CONTEXTUAL_KEYWORD {
        token.text.to_ascii_uppercase()
    } else {
        token.text.clone()
    }
}

fn symbol_word(token: &SymbolToken) -> bool {
    token.kind.is_keyword()
        || matches!(
            token.kind,
            SyntaxKind::IDENT | SyntaxKind::QUOTED_IDENT | SyntaxKind::CONTEXTUAL_KEYWORD
        )
}

fn is_name_token(token: &SymbolToken) -> bool {
    matches!(
        token.kind,
        SyntaxKind::IDENT | SyntaxKind::QUOTED_IDENT | SyntaxKind::CONTEXTUAL_KEYWORD
    )
}

fn word(token: &SymbolToken, expected: &str) -> bool {
    symbol_word(token) && token.text.eq_ignore_ascii_case(expected)
}

/// Static SQL completion items for `textDocument/completion`.
pub fn completion_items() -> Vec<CompletionItem> {
    let mut items = Vec::new();
    items.extend(keyword_texts().map(|keyword| {
        let label = keyword.to_ascii_uppercase();
        completion_item(
            &label,
            CompletionItemKind::KEYWORD,
            "SQL keyword",
            "Keyword recognized by sql-dialect-fmt.",
            None,
            "1",
        )
    }));
    items.extend(BUILTIN_TYPE_WORDS.iter().map(|label| {
        completion_item(
            label,
            CompletionItemKind::TYPE_PARAMETER,
            "SQL type",
            type_documentation(label),
            None,
            "2",
        )
    }));
    items.extend(SQL_SNIPPETS.iter().map(|snippet| {
        completion_item(
            snippet.label,
            CompletionItemKind::SNIPPET,
            "SQL snippet",
            snippet.documentation,
            Some(snippet.insert_text),
            "3",
        )
    }));
    items
}

fn type_documentation(label: &str) -> &'static str {
    match label {
        "NUMBER" => "Exact fixed-point numeric type.",
        "DECIMAL" | "NUMERIC" => "Alias for NUMBER.",
        "INT" | "INTEGER" | "BIGINT" => "Integer numeric alias.",
        "FLOAT" | "DOUBLE" | "REAL" => "Approximate floating-point numeric type.",
        "VARCHAR" => "Variable-length character data.",
        "STRING" | "TEXT" => "Alias for VARCHAR.",
        "CHAR" => "Character data.",
        "BOOLEAN" => "TRUE/FALSE logical value.",
        "VARIANT" => "Semi-structured value.",
        "OBJECT" => "Semi-structured key/value object.",
        "ARRAY" => "Semi-structured ordered collection.",
        "MAP" => "Structured key/value collection.",
        "VECTOR" => "Vector type for embeddings.",
        "DATE" => "Calendar date without time of day.",
        "TIME" => "Time of day without date.",
        "TIMESTAMP" => "Timestamp family.",
        "TIMESTAMP_NTZ" => "Timestamp without time zone.",
        "TIMESTAMP_LTZ" => "Timestamp using the session time zone.",
        "TIMESTAMP_TZ" => "Timestamp with explicit offset.",
        "BINARY" => "Variable-length binary data.",
        "GEOGRAPHY" => "Spherical geospatial data.",
        "GEOMETRY" => "Planar geospatial data.",
        _ => "Built-in SQL type.",
    }
}

fn completion_item(
    label: &str,
    kind: CompletionItemKind,
    detail: &str,
    documentation: &str,
    insert_text: Option<&str>,
    sort_prefix: &str,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: Some(detail.to_string()),
        documentation: Some(lsp_types::Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: documentation.to_string(),
        })),
        insert_text: insert_text.map(str::to_string),
        insert_text_format: insert_text.map(|_| InsertTextFormat::SNIPPET),
        sort_text: Some(format!("{sort_prefix}-{label}")),
        ..CompletionItem::default()
    }
}

struct Snippet {
    label: &'static str,
    insert_text: &'static str,
    documentation: &'static str,
}

const SQL_SNIPPETS: &[Snippet] = &[
    Snippet {
        label: "SELECT ... FROM ...",
        insert_text: "SELECT ${1:columns}\nFROM ${2:table};",
        documentation: "Scaffold a SELECT statement.",
    },
    Snippet {
        label: "WITH ... AS (...)",
        insert_text: "WITH ${1:cte} AS (\n    SELECT ${2:*}\n    FROM ${3:source}\n)\nSELECT ${4:*}\nFROM ${1:cte};",
        documentation: "Scaffold a common table expression.",
    },
    Snippet {
        label: "CREATE TABLE ...",
        insert_text: "CREATE TABLE ${1:table_name} (\n    ${2:column_name} ${3:NUMBER}\n);",
        documentation: "Scaffold a CREATE TABLE statement.",
    },
    Snippet {
        label: "INSERT INTO ...",
        insert_text: "INSERT INTO ${1:table_name} (${2:columns})\nVALUES (${3:values});",
        documentation: "Scaffold an INSERT statement.",
    },
    Snippet {
        label: "CREATE PROCEDURE ...",
        insert_text: "CREATE PROCEDURE ${1:procedure_name}(${2:args})\nRETURNS ${3:STRING}\nLANGUAGE SQL\nAS\n$$\nBEGIN\n    ${4:RETURN 'ok';}\nEND;\n$$;",
        documentation: "Scaffold a Snowflake SQL procedure.",
    },
];

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

/// The delta-encoded semantic tokens that intersect `range`.
pub fn semantic_tokens_range_with_encoding(
    text: &str,
    range: Range,
    encoding: PositionEncoding,
) -> Vec<SemanticToken> {
    let full = semantic_tokens_with_encoding(text, encoding);
    let mut absolute = Vec::with_capacity(full.len());
    let mut line = 0u32;
    let mut start = 0u32;
    for token in full {
        line += token.delta_line;
        start = if token.delta_line == 0 {
            start + token.delta_start
        } else {
            token.delta_start
        };
        absolute.push(AbsoluteSemanticToken { line, start, token });
    }

    let mut previous: Option<(u32, u32)> = None;
    absolute
        .into_iter()
        .filter(|token| semantic_token_intersects_range(token, range))
        .map(|absolute| {
            let delta_line = previous.map_or(absolute.line, |(line, _)| absolute.line - line);
            let delta_start = if delta_line == 0 {
                previous.map_or(absolute.start, |(_, start)| absolute.start - start)
            } else {
                absolute.start
            };
            previous = Some((absolute.line, absolute.start));
            SemanticToken {
                delta_line,
                delta_start,
                length: absolute.token.length,
                token_type: absolute.token.token_type,
                token_modifiers_bitset: absolute.token.token_modifiers_bitset,
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
struct AbsoluteSemanticToken {
    line: u32,
    start: u32,
    token: SemanticToken,
}

fn semantic_token_intersects_range(token: &AbsoluteSemanticToken, range: Range) -> bool {
    let token_start = Position::new(token.line, token.start);
    let token_end = Position::new(token.line, token.start + token.token.length);
    position_lt(token_start, range.end) && position_lt(range.start, token_end)
}

fn position_lt(a: Position, b: Position) -> bool {
    a.line < b.line || (a.line == b.line && a.character < b.character)
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
    fn range_formatting_reformats_only_the_selected_statement() {
        let text = "select 1;\nselect a,b from t;\n";
        // A range on line 1 — the second statement.
        let range = Range::new(Position::new(1, 0), Position::new(1, 3));
        let edits = format_range_edits(text, range, &FormatOptions::default());
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "SELECT a, b\nFROM t;");
        // The edit is scoped to the second statement, not the top of the document.
        assert_eq!(edits[0].range.start, Position::new(1, 0));
    }

    #[test]
    fn range_formatting_already_formatted_selection_yields_no_edits() {
        let text = "SELECT 1;\nSELECT a, b\nFROM t;\n";
        let range = Range::new(Position::new(0, 0), Position::new(0, 8));
        assert!(format_range_edits(text, range, &FormatOptions::default()).is_empty());
    }

    #[test]
    fn on_type_formatting_after_semicolon_reformats_the_finished_statement() {
        // Cursor right after the `;` the user just typed on line 1.
        let text = "SELECT 1;\nselect a,b from t;\n";
        let edits = on_type_formatting_edits(text, Position::new(1, 18), &FormatOptions::default());
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "SELECT a, b\nFROM t;");
        // Only the second statement is touched.
        assert_eq!(edits[0].range.start, Position::new(1, 0));
    }

    #[test]
    fn on_type_formatting_after_newline_reformats_the_previous_statement() {
        // Cursor at the start of the line after the statement (the user just typed Enter).
        let text = "select a,b from t;\n";
        let edits = on_type_formatting_edits(text, Position::new(1, 0), &FormatOptions::default());
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "SELECT a, b\nFROM t;");
    }

    #[test]
    fn on_type_formatting_already_formatted_statement_yields_no_edits() {
        let text = "SELECT a, b\nFROM t;\n";
        assert!(
            on_type_formatting_edits(text, Position::new(1, 7), &FormatOptions::default())
                .is_empty()
        );
    }

    #[test]
    fn on_type_formatting_in_leading_whitespace_yields_no_edits() {
        let text = "\n\nselect 1;\n";
        assert!(
            on_type_formatting_edits(text, Position::new(1, 0), &FormatOptions::default())
                .is_empty()
        );
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
    fn lint_diagnostics_have_codes_and_respect_options() {
        let wildcard = diagnostics("SELECT * FROM t;")
            .into_iter()
            .find(|diag| diag.message.contains("avoid SELECT *"))
            .expect("SELECT * lint");
        assert_eq!(
            wildcard.code,
            Some(lsp_types::NumberOrString::String("SDF001".to_string()))
        );

        let text = "SELECT id FROM t WHERE id IN (1, 2, 3);";
        let diags = diagnostics_with_lint_options(
            text,
            &FormatOptions::default(),
            LintOptions {
                large_in_list_threshold: 2,
                ..LintOptions::default()
            },
            PositionEncoding::Utf16,
        );
        assert!(diags.iter().any(|diag| {
            diag.code == Some(lsp_types::NumberOrString::String("SDF002".to_string()))
        }));

        let disabled = diagnostics_with_lint_options(
            text,
            &FormatOptions::default(),
            LintOptions {
                large_in_list: false,
                large_in_list_threshold: 2,
                ..LintOptions::default()
            },
            PositionEncoding::Utf16,
        );
        assert!(
            disabled
                .iter()
                .all(|diag| diag.code
                    != Some(lsp_types::NumberOrString::String("SDF002".to_string())))
        );
    }

    #[test]
    fn unsupported_embedded_language_has_a_code() {
        let diags = diagnostics("CREATE FUNCTION f() RETURNS STRING LANGUAGE RUBY AS $$x$$;");
        let language = diags
            .iter()
            .find(|diag| diag.message.contains("unsupported embedded language RUBY"))
            .expect("unsupported language lint");
        assert_eq!(
            language.code,
            Some(lsp_types::NumberOrString::String("SDF003".to_string()))
        );
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
    fn document_symbols_name_top_level_statements() {
        let symbols = document_symbols(
            "CREATE TABLE db.t (id INT);\n\nSELECT id\nFROM db.t;",
            &FormatOptions::default(),
        );
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "CREATE TABLE db.t");
        assert_eq!(symbols[0].kind, SymbolKind::STRUCT);
        assert_eq!(symbols[0].selection_range.start, Position::new(0, 13));
        assert_eq!(symbols[1].name, "SELECT");
        assert_eq!(symbols[1].kind, SymbolKind::FUNCTION);
    }

    #[test]
    fn document_symbols_name_stage_file_operations() {
        let symbols = document_symbols(
            "PUT file:///tmp/x.csv @stage;\nLIST @stage/path;",
            &FormatOptions::default(),
        );
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "PUT");
        assert_eq!(symbols[0].kind, SymbolKind::METHOD);
        assert_eq!(symbols[1].name, "LIST");
        assert_eq!(symbols[1].kind, SymbolKind::METHOD);
    }

    #[test]
    fn completion_items_include_keywords_types_and_snippets() {
        let items = completion_items();
        assert!(items.iter().any(|item| {
            item.label == "SELECT" && item.kind == Some(CompletionItemKind::KEYWORD)
        }));
        assert!(items
            .iter()
            .any(|item| item.label == "NUMBER"
                && item.kind == Some(CompletionItemKind::TYPE_PARAMETER)));
        assert!(items.iter().any(|item| {
            item.label == "CREATE TABLE ..."
                && item.kind == Some(CompletionItemKind::SNIPPET)
                && item.insert_text_format == Some(InsertTextFormat::SNIPPET)
        }));
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

    #[test]
    fn semantic_tokens_range_filters_and_reencodes_tokens() {
        let text = "SELECT a\nFROM t\nWHERE a = 1";
        let tokens = semantic_tokens_range_with_encoding(
            text,
            Range::new(Position::new(1, 0), Position::new(2, 0)),
            PositionEncoding::Utf16,
        );
        assert!(!tokens.is_empty());
        assert_eq!(tokens[0].delta_line, 1);

        let mut line = 0u32;
        let mut start = 0u32;
        for token in tokens {
            line += token.delta_line;
            start = if token.delta_line == 0 {
                start + token.delta_start
            } else {
                token.delta_start
            };
            assert_eq!(line, 1);
            assert!(start + token.length <= 6);
        }
    }
}
