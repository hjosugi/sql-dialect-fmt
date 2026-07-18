//! `sql-dialect-fmt-lsp` — a Language Server for Snowflake SQL.
//!
//! Speaks LSP over stdio (via `lsp-server`) and exposes whole-document, range, and on-type
//! **formatting**, **semantic tokens**, **diagnostics**, **hover**, **folding ranges**,
//! **document symbols**, and static SQL **completion**. Documents are kept in sync incrementally (range edits are spliced as
//! they arrive). Everything is synchronous — no async runtime — matching the rest of the workspace.

// `lsp_types::Uri` wraps a parsed URI whose hash/eq are value-stable; the lint can't see that, so
// using it as a `HashMap` key is sound here.
#![allow(clippy::mutable_key_type)]

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
    Notification as _, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, Completion, DocumentSymbolRequest, FoldingRangeRequest, Formatting,
    HoverRequest, OnTypeFormatting, RangeFormatting, Request as _, SemanticTokensFullDeltaRequest,
    SemanticTokensFullRequest, SemanticTokensRangeRequest,
};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, CodeActionResponse, CompletionOptions, CompletionParams,
    CompletionResponse, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams,
    DocumentOnTypeFormattingOptions, DocumentOnTypeFormattingParams, DocumentRangeFormattingParams,
    DocumentSymbolOptions, DocumentSymbolParams, DocumentSymbolResponse, FoldingRange,
    FoldingRangeParams, Hover, HoverParams, HoverProviderCapability, InitializeParams, OneOf,
    Position, PositionEncodingKind, PublishDiagnosticsParams, Range, SemanticTokens,
    SemanticTokensDeltaParams, SemanticTokensFullDeltaResult, SemanticTokensLegend,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensRangeParams,
    SemanticTokensRangeResult, SemanticTokensServerCapabilities, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri, WorkDoneProgressOptions,
    WorkspaceEdit,
};
use serde::Deserialize;
use sql_dialect_fmt_config::Config;
use sql_dialect_fmt_formatter::{FormatOptions, KeywordCase, LineEnding};
use sql_dialect_fmt_lsp::{
    apply_change_with_encoding, completion_items, diagnostic_lint_code,
    diagnostics_with_lint_options, document_symbols_with_encoding, folding_ranges,
    format_edits_with_encoding, format_range_edits_with_encoding, hover_with_encoding,
    on_type_formatting_edits_with_encoding, semantic_tokens_range_with_encoding,
    semantic_tokens_with_encoding, token_modifiers, token_types, LintOptions,
    PositionEncoding as NegotiatedPositionEncoding,
};
use sql_dialect_fmt_parser::Dialect;

#[derive(Debug)]
struct Document {
    text: String,
    version: Option<i32>,
}

type Docs = HashMap<Uri, Document>;

/// Keys under which a client may nest the `sql-dialect-fmt` settings block.
const SETTINGS_SECTION_KEYS: [&str; 3] = ["sqlDialectFmt", "sql-dialect-fmt", "sql_dialect_fmt"];

/// Server state: the editor-provided settings overlay plus protocol bookkeeping.
///
/// Effective [`FormatOptions`] are resolved per request as **formatter defaults → the nearest
/// `sql-dialect-fmt.toml` for the document → these editor settings**. Editor settings win, mirroring
/// how the CLI lets explicit flags override the config file. Keeping the editor overlay as a partial
/// (rather than pre-merged options) is what lets a project's config file layer underneath it.
#[derive(Clone, Debug)]
struct ServerState {
    editor: FormatterSettings,
    position_encoding: NegotiatedPositionEncoding,
    semantic_result_seq: u64,
}

impl ServerState {
    fn from_initialize(params: &InitializeParams) -> Self {
        let mut state = Self {
            editor: FormatterSettings::default(),
            position_encoding: position_encoding_from_client(params),
            semantic_result_seq: 0,
        };
        if let Some(settings) = &params.initialization_options {
            state.apply_settings(settings);
        }
        state
    }

    /// Merge an `initializationOptions` / `didChangeConfiguration` payload into the editor overlay.
    ///
    /// Accepts the settings at the top level and under the `sqlDialectFmt` (and legacy) keys, with
    /// or without a wrapping `settings` object, so it works across client shapes.
    fn apply_settings(&mut self, settings: &serde_json::Value) {
        merge_settings_value(&mut self.editor, settings);
        for key in SETTINGS_SECTION_KEYS {
            if let Some(nested) = settings.get(key) {
                merge_settings_value(&mut self.editor, nested);
            }
        }
        if let Some(nested) = settings.get("settings") {
            merge_settings_value(&mut self.editor, nested);
            for key in SETTINGS_SECTION_KEYS {
                if let Some(section) = nested.get(key) {
                    merge_settings_value(&mut self.editor, section);
                }
            }
        }
    }

    /// Resolve effective format options for `uri`: defaults → nearest config file → editor overlay.
    fn effective_options(&self, uri: &Uri) -> FormatOptions {
        let mut options = FormatOptions::default();
        if let Some(config) = discover_config(uri) {
            config.apply_to(&mut options);
        }
        apply_editor_format(&mut options, &self.editor);
        options
    }

    /// Resolve effective lint options. Config files carry no lint keys, so these come from the
    /// editor overlay only.
    fn effective_lint(&self) -> LintOptions {
        let mut lint = LintOptions::default();
        apply_editor_lint(&mut lint, &self.editor.lint);
        lint
    }

    fn next_semantic_result_id(&mut self) -> String {
        self.semantic_result_seq += 1;
        self.semantic_result_seq.to_string()
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct FormatterSettings {
    #[serde(alias = "line_width")]
    line_width: Option<usize>,
    #[serde(alias = "indent_width")]
    indent_width: Option<usize>,
    #[serde(alias = "uppercase_keywords")]
    uppercase_keywords: Option<bool>,
    #[serde(alias = "keyword_case")]
    keyword_case: Option<String>,
    #[serde(alias = "line_ending")]
    line_ending: Option<String>,
    dialect: Option<String>,
    #[serde(default)]
    lint: LintSettings,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct LintSettings {
    enabled: Option<bool>,
    #[serde(alias = "select_wildcard")]
    select_wildcard: Option<bool>,
    #[serde(alias = "large_in_list")]
    large_in_list: Option<bool>,
    #[serde(alias = "unsupported_embedded_language")]
    unsupported_embedded_language: Option<bool>,
    #[serde(alias = "delete_without_where")]
    delete_without_where: Option<bool>,
    #[serde(alias = "update_without_where")]
    update_without_where: Option<bool>,
    #[serde(alias = "comma_join")]
    comma_join: Option<bool>,
    #[serde(alias = "order_by_ordinal")]
    order_by_ordinal: Option<bool>,
    #[serde(alias = "large_in_list_threshold")]
    large_in_list_threshold: Option<usize>,
}

impl FormatterSettings {
    /// Overlay the fields `other` actually set onto `self`; later notifications win field-by-field.
    fn merge_from(&mut self, other: &FormatterSettings) {
        if other.line_width.is_some() {
            self.line_width = other.line_width;
        }
        if other.indent_width.is_some() {
            self.indent_width = other.indent_width;
        }
        if other.uppercase_keywords.is_some() {
            self.uppercase_keywords = other.uppercase_keywords;
        }
        if other.keyword_case.is_some() {
            self.keyword_case = other.keyword_case.clone();
        }
        if other.line_ending.is_some() {
            self.line_ending = other.line_ending.clone();
        }
        if other.dialect.is_some() {
            self.dialect = other.dialect.clone();
        }
        self.lint.merge_from(&other.lint);
    }
}

impl LintSettings {
    fn merge_from(&mut self, other: &LintSettings) {
        if other.enabled.is_some() {
            self.enabled = other.enabled;
        }
        if other.select_wildcard.is_some() {
            self.select_wildcard = other.select_wildcard;
        }
        if other.large_in_list.is_some() {
            self.large_in_list = other.large_in_list;
        }
        if other.unsupported_embedded_language.is_some() {
            self.unsupported_embedded_language = other.unsupported_embedded_language;
        }
        if other.delete_without_where.is_some() {
            self.delete_without_where = other.delete_without_where;
        }
        if other.update_without_where.is_some() {
            self.update_without_where = other.update_without_where;
        }
        if other.comma_join.is_some() {
            self.comma_join = other.comma_join;
        }
        if other.order_by_ordinal.is_some() {
            self.order_by_ordinal = other.order_by_ordinal;
        }
        if other.large_in_list_threshold.is_some() {
            self.large_in_list_threshold = other.large_in_list_threshold;
        }
    }
}

fn position_encoding_from_client(params: &InitializeParams) -> NegotiatedPositionEncoding {
    let Some(encodings) = params
        .capabilities
        .general
        .as_ref()
        .and_then(|general| general.position_encodings.as_ref())
    else {
        return NegotiatedPositionEncoding::Utf16;
    };
    if encodings
        .iter()
        .any(|encoding| encoding == &PositionEncodingKind::UTF8)
    {
        NegotiatedPositionEncoding::Utf8
    } else {
        NegotiatedPositionEncoding::Utf16
    }
}

fn merge_settings_value(editor: &mut FormatterSettings, value: &serde_json::Value) {
    if let Ok(incoming) = serde_json::from_value::<FormatterSettings>(value.clone()) {
        editor.merge_from(&incoming);
    }
}

fn apply_editor_format(options: &mut FormatOptions, settings: &FormatterSettings) {
    if let Some(line_width) = settings.line_width {
        options.line_width = line_width;
    }
    if let Some(indent_width) = settings.indent_width {
        options.indent_width = indent_width;
    }
    if let Some(uppercase_keywords) = settings.uppercase_keywords {
        *options = (*options).with_uppercase_keywords(uppercase_keywords);
    }
    if let Some(keyword_case) = settings
        .keyword_case
        .as_deref()
        .and_then(parse_keyword_case)
    {
        *options = (*options).with_keyword_case(keyword_case);
    }
    if let Some(line_ending) = settings.line_ending.as_deref().and_then(parse_line_ending) {
        *options = (*options).with_line_ending(line_ending);
    }
    if let Some(dialect) = settings.dialect.as_deref().and_then(parse_dialect) {
        options.dialect = dialect;
    }
}

fn apply_editor_lint(options: &mut LintOptions, settings: &LintSettings) {
    if let Some(enabled) = settings.enabled {
        options.select_wildcard = enabled;
        options.large_in_list = enabled;
        options.unsupported_embedded_language = enabled;
        options.delete_without_where = enabled;
        options.update_without_where = enabled;
        options.comma_join = enabled;
        options.order_by_ordinal = enabled;
    }
    if let Some(select_wildcard) = settings.select_wildcard {
        options.select_wildcard = select_wildcard;
    }
    if let Some(large_in_list) = settings.large_in_list {
        options.large_in_list = large_in_list;
    }
    if let Some(unsupported_embedded_language) = settings.unsupported_embedded_language {
        options.unsupported_embedded_language = unsupported_embedded_language;
    }
    if let Some(delete_without_where) = settings.delete_without_where {
        options.delete_without_where = delete_without_where;
    }
    if let Some(update_without_where) = settings.update_without_where {
        options.update_without_where = update_without_where;
    }
    if let Some(comma_join) = settings.comma_join {
        options.comma_join = comma_join;
    }
    if let Some(order_by_ordinal) = settings.order_by_ordinal {
        options.order_by_ordinal = order_by_ordinal;
    }
    if let Some(threshold) = settings.large_in_list_threshold {
        options.large_in_list_threshold = threshold;
    }
}

/// Discover and load the nearest `sql-dialect-fmt.toml` for a document, if any.
///
/// A malformed config file is ignored (the document just falls back to defaults + editor settings)
/// rather than surfaced as an error — the editor should keep working regardless.
fn discover_config(uri: &Uri) -> Option<Config> {
    let path = uri_to_path(uri)?;
    let config_path = sql_dialect_fmt_config::discover(&path)?;
    Config::load(&config_path).ok()
}

/// Best-effort `file://` URI → filesystem path. Returns `None` for non-file URIs (e.g. `untitled:`),
/// so those documents simply fall back to defaults + editor settings.
fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let after_scheme = uri.as_str().strip_prefix("file://")?;
    // Drop an optional authority component: `file://host/p` → `/p`; `file:///p` → `/p`.
    let path_part = match after_scheme.find('/') {
        Some(0) => after_scheme,
        Some(index) => &after_scheme[index..],
        None => return None,
    };
    let decoded = percent_decode(path_part);
    #[cfg(windows)]
    {
        // `file:///C:/dir` decodes to `/C:/dir`; drop the leading slash before the drive letter.
        let bytes = decoded.as_bytes();
        if bytes.first() == Some(&b'/')
            && bytes.get(1).is_some_and(u8::is_ascii_alphabetic)
            && bytes.get(2) == Some(&b':')
        {
            return Some(PathBuf::from(&decoded[1..]));
        }
    }
    Some(PathBuf::from(decoded))
}

/// Decode `%XX` escapes in a URI path component. Unrecognized escapes are left verbatim.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_dialect(value: &str) -> Option<Dialect> {
    match value.to_ascii_lowercase().as_str() {
        "snowflake" => Some(Dialect::Snowflake),
        "databricks" => Some(Dialect::Databricks),
        _ => None,
    }
}

fn parse_keyword_case(value: &str) -> Option<KeywordCase> {
    match value.to_ascii_lowercase().as_str() {
        "upper" => Some(KeywordCase::Upper),
        "lower" => Some(KeywordCase::Lower),
        "preserve" => Some(KeywordCase::Preserve),
        _ => None,
    }
}

fn parse_line_ending(value: &str) -> Option<LineEnding> {
    match value.to_ascii_lowercase().as_str() {
        "auto" => Some(LineEnding::Auto),
        "lf" => Some(LineEnding::Lf),
        "crlf" => Some(LineEnding::Crlf),
        _ => None,
    }
}

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();
    let (initialize_id, initialize_params) = connection.initialize_start()?;
    let initialize_params: InitializeParams = serde_json::from_value(initialize_params)?;
    let mut state = ServerState::from_initialize(&initialize_params);
    let initialize_result = serde_json::json!({
        "capabilities": server_capabilities(state.position_encoding),
    });
    connection.initialize_finish(initialize_id, initialize_result)?;
    // Take the connection by value so it is dropped when the loop ends, closing the I/O channels —
    // otherwise `io_threads.join()` would block forever waiting for the writer thread to finish.
    main_loop(connection, &mut state)?;
    io_threads.join()?;
    Ok(())
}

fn server_capabilities(position_encoding: NegotiatedPositionEncoding) -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(match position_encoding {
            NegotiatedPositionEncoding::Utf8 => PositionEncodingKind::UTF8,
            NegotiatedPositionEncoding::Utf16 => PositionEncodingKind::UTF16,
        }),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL,
        )),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        document_on_type_formatting_provider: Some(DocumentOnTypeFormattingOptions {
            first_trigger_character: ";".to_string(),
            more_trigger_character: Some(vec!["\n".to_string()]),
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
        document_symbol_provider: Some(OneOf::Right(DocumentSymbolOptions {
            label: Some("SQL".to_string()),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        })),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            trigger_characters: Some(vec![".".to_string()]),
            ..CompletionOptions::default()
        }),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
            resolve_provider: Some(false),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        })),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: SemanticTokensLegend {
                    token_types: token_types(),
                    token_modifiers: token_modifiers(),
                },
                full: Some(lsp_types::SemanticTokensFullOptions::Delta { delta: Some(true) }),
                range: Some(true),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            },
        )),
        ..ServerCapabilities::default()
    }
}

fn main_loop(
    connection: Connection,
    state: &mut ServerState,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut docs = Docs::new();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    return Ok(());
                }
                let response = handle_request(request, &docs, state);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Notification(notification) => {
                handle_notification(&connection, notification, &mut docs, state)?;
            }
            Message::Response(_) => {} // we issue no server->client requests
        }
    }
    Ok(())
}

fn handle_request(request: Request, docs: &Docs, state: &mut ServerState) -> Response {
    match request.method.as_str() {
        Formatting::METHOD => match cast::<Formatting>(request) {
            Ok((id, params)) => ok(id, formatting(params, docs, state)),
            Err(response) => *response,
        },
        RangeFormatting::METHOD => match cast::<RangeFormatting>(request) {
            Ok((id, params)) => ok(id, range_formatting(params, docs, state)),
            Err(response) => *response,
        },
        OnTypeFormatting::METHOD => match cast::<OnTypeFormatting>(request) {
            Ok((id, params)) => ok(id, on_type_formatting(params, docs, state)),
            Err(response) => *response,
        },
        SemanticTokensFullRequest::METHOD => match cast::<SemanticTokensFullRequest>(request) {
            Ok((id, params)) => ok(id, semantic_tokens_full(params, docs, state)),
            Err(response) => *response,
        },
        SemanticTokensFullDeltaRequest::METHOD => {
            match cast::<SemanticTokensFullDeltaRequest>(request) {
                Ok((id, params)) => ok(id, semantic_tokens_full_delta(params, docs, state)),
                Err(response) => *response,
            }
        }
        SemanticTokensRangeRequest::METHOD => match cast::<SemanticTokensRangeRequest>(request) {
            Ok((id, params)) => ok(id, semantic_tokens_range(params, docs, state)),
            Err(response) => *response,
        },
        HoverRequest::METHOD => match cast::<HoverRequest>(request) {
            Ok((id, params)) => ok(id, hover_request(params, docs, state)),
            Err(response) => *response,
        },
        FoldingRangeRequest::METHOD => match cast::<FoldingRangeRequest>(request) {
            Ok((id, params)) => ok(id, folding_request(params, docs)),
            Err(response) => *response,
        },
        DocumentSymbolRequest::METHOD => match cast::<DocumentSymbolRequest>(request) {
            Ok((id, params)) => ok(id, document_symbol_request(params, docs, state)),
            Err(response) => *response,
        },
        Completion::METHOD => match cast::<Completion>(request) {
            Ok((id, params)) => ok(id, completion_request(params)),
            Err(response) => *response,
        },
        CodeActionRequest::METHOD => match cast::<CodeActionRequest>(request) {
            Ok((id, params)) => ok(id, code_action_request(params, docs)),
            Err(response) => *response,
        },
        _ => Response::new_err(
            request.id,
            lsp_server::ErrorCode::MethodNotFound as i32,
            format!("unsupported request: {}", request.method),
        ),
    }
}

fn formatting(
    params: DocumentFormattingParams,
    docs: &Docs,
    state: &ServerState,
) -> Vec<lsp_types::TextEdit> {
    let Some(document) = docs.get(&params.text_document.uri) else {
        return Vec::new();
    };
    let mut options = state.effective_options(&params.text_document.uri);
    if params.options.insert_spaces {
        options.indent_width = (params.options.tab_size as usize).max(1);
    }
    format_edits_with_encoding(&document.text, &options, state.position_encoding)
}

fn range_formatting(
    params: DocumentRangeFormattingParams,
    docs: &Docs,
    state: &ServerState,
) -> Vec<lsp_types::TextEdit> {
    let Some(document) = docs.get(&params.text_document.uri) else {
        return Vec::new();
    };
    let mut options = state.effective_options(&params.text_document.uri);
    if params.options.insert_spaces {
        options.indent_width = (params.options.tab_size as usize).max(1);
    }
    format_range_edits_with_encoding(
        &document.text,
        params.range,
        &options,
        state.position_encoding,
    )
}

fn on_type_formatting(
    params: DocumentOnTypeFormattingParams,
    docs: &Docs,
    state: &ServerState,
) -> Vec<TextEdit> {
    let uri = &params.text_document_position.text_document.uri;
    let Some(document) = docs.get(uri) else {
        return Vec::new();
    };
    let mut options = state.effective_options(uri);
    if params.options.insert_spaces {
        options.indent_width = (params.options.tab_size as usize).max(1);
    }
    on_type_formatting_edits_with_encoding(
        &document.text,
        params.text_document_position.position,
        &options,
        state.position_encoding,
    )
}

fn semantic_tokens_full(
    params: SemanticTokensParams,
    docs: &Docs,
    state: &mut ServerState,
) -> Option<lsp_types::SemanticTokensResult> {
    let text = &docs.get(&params.text_document.uri)?.text;
    Some(
        SemanticTokens {
            result_id: Some(state.next_semantic_result_id()),
            data: semantic_tokens_with_encoding(text, state.position_encoding),
        }
        .into(),
    )
}

fn semantic_tokens_full_delta(
    params: SemanticTokensDeltaParams,
    docs: &Docs,
    state: &mut ServerState,
) -> Option<SemanticTokensFullDeltaResult> {
    let text = &docs.get(&params.text_document.uri)?.text;
    Some(
        SemanticTokens {
            result_id: Some(state.next_semantic_result_id()),
            data: semantic_tokens_with_encoding(text, state.position_encoding),
        }
        .into(),
    )
}

fn semantic_tokens_range(
    params: SemanticTokensRangeParams,
    docs: &Docs,
    state: &ServerState,
) -> Option<SemanticTokensRangeResult> {
    let text = &docs.get(&params.text_document.uri)?.text;
    Some(
        SemanticTokens {
            result_id: None,
            data: semantic_tokens_range_with_encoding(text, params.range, state.position_encoding),
        }
        .into(),
    )
}

fn hover_request(params: HoverParams, docs: &Docs, state: &ServerState) -> Option<Hover> {
    let position = params.text_document_position_params;
    let text = &docs.get(&position.text_document.uri)?.text;
    hover_with_encoding(text, position.position, state.position_encoding)
}

fn folding_request(params: FoldingRangeParams, docs: &Docs) -> Option<Vec<FoldingRange>> {
    let text = &docs.get(&params.text_document.uri)?.text;
    Some(folding_ranges(text))
}

fn document_symbol_request(
    params: DocumentSymbolParams,
    docs: &Docs,
    state: &ServerState,
) -> Option<DocumentSymbolResponse> {
    let text = &docs.get(&params.text_document.uri)?.text;
    let options = state.effective_options(&params.text_document.uri);
    Some(document_symbols_with_encoding(text, &options, state.position_encoding).into())
}

fn completion_request(_params: CompletionParams) -> Option<CompletionResponse> {
    Some(completion_items().into())
}

fn code_action_request(params: CodeActionParams, docs: &Docs) -> Option<CodeActionResponse> {
    if !allows_quickfix(params.context.only.as_deref()) {
        return Some(Vec::new());
    }

    let uri = params.text_document.uri;
    let document = docs.get(&uri)?;
    let mut actions = Vec::new();
    let mut seen = Vec::new();
    for diagnostic in params.context.diagnostics {
        let Some(code) = diagnostic_lint_code(&diagnostic) else {
            continue;
        };
        let line = diagnostic.range.start.line;
        if seen.contains(&(line, code.as_str())) {
            continue;
        }
        seen.push((line, code.as_str()));
        let Some(indent) = line_indent(&document.text, line) else {
            continue;
        };

        let edit = TextEdit {
            range: Range::new(Position::new(line, 0), Position::new(line, 0)),
            new_text: format!(
                "{indent}-- sql-dialect-fmt: disable-next-line {}\n",
                code.as_str()
            ),
        };
        let mut changes = HashMap::new();
        changes.insert(uri.clone(), vec![edit]);
        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: format!("Suppress {} for next line", code.as_str()),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic]),
            edit: Some(WorkspaceEdit::new(changes)),
            is_preferred: Some(false),
            ..CodeAction::default()
        }));
    }

    Some(actions)
}

fn allows_quickfix(only: Option<&[CodeActionKind]>) -> bool {
    let Some(kinds) = only else {
        return true;
    };
    kinds.iter().any(|kind| {
        kind.as_str().is_empty()
            || kind.as_str() == CodeActionKind::QUICKFIX.as_str()
            || kind.as_str().starts_with("quickfix.")
    })
}

fn line_indent(text: &str, line: u32) -> Option<&str> {
    let line = text.lines().nth(line as usize)?;
    let end = line
        .find(|ch| ch != ' ' && ch != '\t')
        .unwrap_or(line.len());
    Some(&line[..end])
}

fn handle_notification(
    connection: &Connection,
    notification: Notification,
    docs: &mut Docs,
    state: &mut ServerState,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    match notification.method.as_str() {
        DidChangeConfiguration::METHOD => {
            let params: DidChangeConfigurationParams =
                notification.extract(DidChangeConfiguration::METHOD)?;
            state.apply_settings(&params.settings);
            for uri in docs.keys().cloned().collect::<Vec<_>>() {
                publish_diagnostics(connection, docs, &uri, state)?;
            }
        }
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams =
                notification.extract(DidOpenTextDocument::METHOD)?;
            let uri = params.text_document.uri;
            docs.insert(
                uri.clone(),
                Document {
                    text: params.text_document.text,
                    version: Some(params.text_document.version),
                },
            );
            publish_diagnostics(connection, docs, &uri, state)?;
        }
        DidChangeTextDocument::METHOD => {
            let params: DidChangeTextDocumentParams =
                notification.extract(DidChangeTextDocument::METHOD)?;
            let uri = params.text_document.uri;
            // Incremental sync: apply each change in order, splicing range edits and honoring
            // whole-document replacements (a change with no range).
            let mut text = docs
                .remove(&uri)
                .map(|document| document.text)
                .unwrap_or_default();
            for change in params.content_changes {
                text = apply_change_with_encoding(
                    &text,
                    change.range,
                    &change.text,
                    state.position_encoding,
                );
            }
            docs.insert(
                uri.clone(),
                Document {
                    text,
                    version: Some(params.text_document.version),
                },
            );
            publish_diagnostics(connection, docs, &uri, state)?;
        }
        DidCloseTextDocument::METHOD => {
            let params: DidCloseTextDocumentParams =
                notification.extract(DidCloseTextDocument::METHOD)?;
            let uri = params.text_document.uri;
            let version = docs.remove(&uri).and_then(|document| document.version);
            send_diagnostics(connection, uri, Vec::new(), version)?; // clear on close
        }
        _ => {}
    }
    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    docs: &Docs,
    uri: &Uri,
    state: &ServerState,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let diags = docs
        .get(uri)
        .map(|document| {
            diagnostics_with_lint_options(
                &document.text,
                &state.effective_options(uri),
                state.effective_lint(),
                state.position_encoding,
            )
        })
        .unwrap_or_default();
    let version = docs.get(uri).and_then(|document| document.version);
    send_diagnostics(connection, uri.clone(), diags, version)
}

fn send_diagnostics(
    connection: &Connection,
    uri: Uri,
    diagnostics: Vec<lsp_types::Diagnostic>,
    version: Option<i32>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version,
    };
    let notification = Notification::new(PublishDiagnostics::METHOD.to_string(), params);
    connection
        .sender
        .send(Message::Notification(notification))?;
    Ok(())
}

/// Build a successful response, serializing `result` (which may be `()`/`None`/a vec).
fn ok<T: serde::Serialize>(id: RequestId, result: T) -> Response {
    Response::new_ok(id, result)
}

/// Extract a typed request, turning a method/shape mismatch into an error response.
fn cast<R>(request: Request) -> Result<(RequestId, R::Params), Box<Response>>
where
    R: lsp_types::request::Request,
{
    request.extract(R::METHOD).map_err(|err| match err {
        ExtractError::JsonError { method, error } => Box::new(Response::new_err(
            // `extract` consumed the id on a JSON error; report against a fresh sentinel.
            RequestId::from(0),
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("invalid params for {method}: {error}"),
        )),
        ExtractError::MethodMismatch(request) => Box::new(Response::new_err(
            request.id,
            lsp_server::ErrorCode::MethodNotFound as i32,
            format!("method mismatch: {}", request.method),
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{
        TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
        VersionedTextDocumentIdentifier,
    };

    fn test_uri() -> Uri {
        "file:///workspace/query.sql".parse().expect("valid URI")
    }

    /// Build a `file://` URI from an absolute path, cross-platform. Windows drive paths use
    /// backslashes and need `file:///C:/...`, so normalize separators and add the leading slash.
    fn file_uri(path: &std::path::Path) -> Uri {
        let mut text = path.display().to_string().replace('\\', "/");
        if !text.starts_with('/') {
            text.insert(0, '/');
        }
        format!("file://{text}").parse().expect("valid URI")
    }

    fn test_state() -> ServerState {
        ServerState {
            editor: FormatterSettings::default(),
            position_encoding: NegotiatedPositionEncoding::Utf16,
            semantic_result_seq: 0,
        }
    }

    fn recv_diagnostics(client: &Connection) -> PublishDiagnosticsParams {
        let Message::Notification(notification) =
            client.receiver.recv().expect("diagnostics notification")
        else {
            panic!("expected diagnostics notification")
        };
        assert_eq!(notification.method, PublishDiagnostics::METHOD);
        serde_json::from_value(notification.params).expect("publishDiagnostics params")
    }

    #[test]
    fn did_open_stores_and_publishes_document_version() {
        let (server, client) = Connection::memory();
        let mut docs = Docs::new();
        let mut state = test_state();
        let uri = test_uri();
        let notification = Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "sql".to_string(),
                    version: 7,
                    text: "select * from".to_string(),
                },
            },
        );

        handle_notification(&server, notification, &mut docs, &mut state).expect("handle didOpen");

        assert_eq!(
            docs.get(&uri).and_then(|document| document.version),
            Some(7)
        );
        let params = recv_diagnostics(&client);
        assert_eq!(params.uri, uri);
        assert_eq!(params.version, Some(7));
        assert!(!params.diagnostics.is_empty());
    }

    #[test]
    fn did_change_updates_and_publishes_document_version() {
        let (server, client) = Connection::memory();
        let mut docs = Docs::new();
        let mut state = test_state();
        let uri = test_uri();
        docs.insert(
            uri.clone(),
            Document {
                text: "select * from".to_string(),
                version: Some(7),
            },
        );
        let notification = Notification::new(
            DidChangeTextDocument::METHOD.to_string(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 8,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "select * from t".to_string(),
                }],
            },
        );

        handle_notification(&server, notification, &mut docs, &mut state)
            .expect("handle didChange");

        let document = docs.get(&uri).expect("document retained");
        assert_eq!(document.text, "select * from t");
        assert_eq!(document.version, Some(8));
        let params = recv_diagnostics(&client);
        assert_eq!(params.uri, uri);
        assert_eq!(params.version, Some(8));
    }

    #[test]
    fn did_close_clears_diagnostics_with_last_document_version() {
        let (server, client) = Connection::memory();
        let mut docs = Docs::new();
        let mut state = test_state();
        let uri = test_uri();
        docs.insert(
            uri.clone(),
            Document {
                text: "select * from".to_string(),
                version: Some(9),
            },
        );
        let notification = Notification::new(
            DidCloseTextDocument::METHOD.to_string(),
            DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
            },
        );

        handle_notification(&server, notification, &mut docs, &mut state).expect("handle didClose");

        assert!(!docs.contains_key(&uri));
        let params = recv_diagnostics(&client);
        assert_eq!(params.uri, uri);
        assert_eq!(params.version, Some(9));
        assert!(params.diagnostics.is_empty());
    }

    #[test]
    fn did_change_configuration_updates_dialect_and_republishes_diagnostics() {
        let (server, client) = Connection::memory();
        let mut docs = Docs::new();
        let mut state = test_state();
        let uri = test_uri();
        docs.insert(
            uri.clone(),
            Document {
                text: "SELECT a <=> b FROM t;".to_string(),
                version: Some(4),
            },
        );
        let notification = Notification::new(
            DidChangeConfiguration::METHOD.to_string(),
            DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "sqlDialectFmt": {
                        "dialect": "databricks",
                        "keywordCase": "lower",
                        "lineEnding": "crlf"
                    }
                }),
            },
        );

        handle_notification(&server, notification, &mut docs, &mut state)
            .expect("handle didChangeConfiguration");

        let options = state.effective_options(&uri);
        assert_eq!(options.dialect, Dialect::Databricks);
        assert_eq!(options.keyword_case, KeywordCase::Lower);
        assert_eq!(options.line_ending, LineEnding::Crlf);
        let params = recv_diagnostics(&client);
        assert_eq!(params.uri, uri);
        assert_eq!(params.version, Some(4));
        assert!(params
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unexpected character")));
    }

    #[test]
    fn did_change_configuration_updates_lint_options() {
        let (server, client) = Connection::memory();
        let mut docs = Docs::new();
        let mut state = test_state();
        let uri = test_uri();
        docs.insert(
            uri.clone(),
            Document {
                text: "SELECT id FROM t WHERE id IN (1, 2, 3);".to_string(),
                version: Some(5),
            },
        );
        let notification = Notification::new(
            DidChangeConfiguration::METHOD.to_string(),
            DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "sqlDialectFmt": {
                        "lint": {
                            "largeInList": false,
                            "largeInListThreshold": 2
                        }
                    }
                }),
            },
        );

        handle_notification(&server, notification, &mut docs, &mut state)
            .expect("handle didChangeConfiguration");

        let lint = state.effective_lint();
        assert!(!lint.large_in_list);
        assert_eq!(lint.large_in_list_threshold, 2);
        let params = recv_diagnostics(&client);
        assert!(params.diagnostics.iter().all(|diagnostic| {
            diagnostic.code != Some(lsp_types::NumberOrString::String("SDF002".to_string()))
        }));
    }

    #[test]
    fn uri_to_path_decodes_file_uris() {
        let plain: Uri = "file:///home/u/query.sql".parse().expect("valid URI");
        assert_eq!(
            uri_to_path(&plain),
            Some(PathBuf::from("/home/u/query.sql"))
        );

        let escaped: Uri = "file:///home/a%20b/q.sql".parse().expect("valid URI");
        assert_eq!(
            uri_to_path(&escaped),
            Some(PathBuf::from("/home/a b/q.sql"))
        );

        let non_file: Uri = "untitled:Untitled-1".parse().expect("valid URI");
        assert_eq!(uri_to_path(&non_file), None);
    }

    #[test]
    fn config_file_layers_under_editor_settings() {
        use std::io::Write;

        let dir = tempfile::tempdir().expect("tempdir");
        let mut file =
            std::fs::File::create(dir.path().join("sql-dialect-fmt.toml")).expect("create config");
        writeln!(
            file,
            "dialect = \"databricks\"\nline_width = 40\nindent_width = 8"
        )
        .expect("write config");
        drop(file);

        let uri = file_uri(&dir.path().join("query.sql"));

        // With no editor settings, the config file supplies dialect and widths.
        let mut state = test_state();
        let options = state.effective_options(&uri);
        assert_eq!(options.dialect, Dialect::Databricks);
        assert_eq!(options.line_width, 40);
        assert_eq!(options.indent_width, 8);

        // Editor settings win over the config file, field by field.
        state.apply_settings(&serde_json::json!({ "sqlDialectFmt": { "lineWidth": 120 } }));
        let options = state.effective_options(&uri);
        assert_eq!(options.line_width, 120); // editor override
        assert_eq!(options.indent_width, 8); // still from the config file
        assert_eq!(options.dialect, Dialect::Databricks); // still from the config file
    }

    #[test]
    fn code_action_inserts_lint_suppression_comment() {
        let uri = test_uri();
        let text = "    SELECT * FROM t;";
        let docs = HashMap::from([(
            uri.clone(),
            Document {
                text: text.to_string(),
                version: Some(1),
            },
        )]);
        let diagnostic = diagnostics_with_lint_options(
            text,
            &FormatOptions::default(),
            LintOptions::default(),
            NegotiatedPositionEncoding::Utf16,
        )
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code == Some(lsp_types::NumberOrString::String("SDF001".to_string()))
        })
        .expect("SELECT * lint diagnostic");

        let response = code_action_request(
            CodeActionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range: diagnostic.range,
                context: lsp_types::CodeActionContext {
                    diagnostics: vec![diagnostic.clone()],
                    only: None,
                    trigger_kind: None,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
            &docs,
        )
        .expect("code action response");

        assert_eq!(response.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &response[0] else {
            panic!("expected code action")
        };
        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        let changes = action
            .edit
            .as_ref()
            .and_then(|edit| edit.changes.as_ref())
            .expect("workspace edit changes");
        let edit = changes
            .get(&uri)
            .and_then(|edits| edits.first())
            .expect("text edit");
        assert_eq!(
            edit.new_text,
            "    -- sql-dialect-fmt: disable-next-line SDF001\n"
        );

        let updated = format!("{}{}", edit.new_text, text);
        assert!(diagnostics_with_lint_options(
            &updated,
            &FormatOptions::default(),
            LintOptions::default(),
            NegotiatedPositionEncoding::Utf16,
        )
        .iter()
        .all(|diagnostic| diagnostic.code
            != Some(lsp_types::NumberOrString::String("SDF001".to_string()))));
    }
}
