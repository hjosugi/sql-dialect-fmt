//! `snow-fmt-lsp` — a Language Server for Snowflake SQL.
//!
//! Speaks LSP over stdio (via `lsp-server`) and offers five features, each backed by the pure
//! functions in [`snow_fmt_lsp`]: whole-document **formatting**, **semantic tokens** (from the
//! lossless highlighter), **diagnostics** (the parser's recovered errors, published on every
//! open/change), **hover** (keyword/type/symbol docs), and **folding ranges** (per statement).
//! Documents are kept in sync incrementally (range edits are spliced as they arrive). Everything is
//! synchronous — no async runtime — matching the rest of the workspace.

// `lsp_types::Uri` wraps a parsed URI whose hash/eq are value-stable; the lint can't see that, so
// using it as a `HashMap` key is sound here.
#![allow(clippy::mutable_key_type)]

use std::collections::HashMap;
use std::error::Error;

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    PublishDiagnostics,
};
use lsp_types::request::{
    FoldingRangeRequest, Formatting, HoverRequest, Request as _, SemanticTokensFullRequest,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, FoldingRange, FoldingRangeParams, Hover, HoverParams,
    HoverProviderCapability, OneOf, PublishDiagnosticsParams, SemanticTokens, SemanticTokensLegend,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensServerCapabilities,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
    WorkDoneProgressOptions,
};
use snow_fmt_formatter::FormatOptions;
use snow_fmt_lsp::{
    apply_change, diagnostics, folding_ranges, format_edits, hover, semantic_tokens, TOKEN_TYPES,
};

type Docs = HashMap<Uri, String>;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = serde_json::to_value(server_capabilities())?;
    connection.initialize(capabilities)?;
    // Take the connection by value so it is dropped when the loop ends, closing the I/O channels —
    // otherwise `io_threads.join()` would block forever waiting for the writer thread to finish.
    main_loop(connection)?;
    io_threads.join()?;
    Ok(())
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL,
        )),
        document_formatting_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: SemanticTokensLegend {
                    token_types: TOKEN_TYPES.to_vec(),
                    token_modifiers: Vec::new(),
                },
                full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
                range: Some(false),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            },
        )),
        ..ServerCapabilities::default()
    }
}

fn main_loop(connection: Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut docs = Docs::new();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    return Ok(());
                }
                let response = handle_request(request, &docs);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Notification(notification) => {
                handle_notification(&connection, notification, &mut docs)?;
            }
            Message::Response(_) => {} // we issue no server->client requests
        }
    }
    Ok(())
}

fn handle_request(request: Request, docs: &Docs) -> Response {
    match request.method.as_str() {
        Formatting::METHOD => match cast::<Formatting>(request) {
            Ok((id, params)) => ok(id, formatting(params, docs)),
            Err(response) => response,
        },
        SemanticTokensFullRequest::METHOD => match cast::<SemanticTokensFullRequest>(request) {
            Ok((id, params)) => ok(id, semantic_tokens_full(params, docs)),
            Err(response) => response,
        },
        HoverRequest::METHOD => match cast::<HoverRequest>(request) {
            Ok((id, params)) => ok(id, hover_request(params, docs)),
            Err(response) => response,
        },
        FoldingRangeRequest::METHOD => match cast::<FoldingRangeRequest>(request) {
            Ok((id, params)) => ok(id, folding_request(params, docs)),
            Err(response) => response,
        },
        _ => Response::new_err(
            request.id,
            lsp_server::ErrorCode::MethodNotFound as i32,
            format!("unsupported request: {}", request.method),
        ),
    }
}

fn formatting(params: DocumentFormattingParams, docs: &Docs) -> Vec<lsp_types::TextEdit> {
    let Some(text) = docs.get(&params.text_document.uri) else {
        return Vec::new();
    };
    // Honor the editor's indent width; keep the other opinionated defaults.
    let options =
        FormatOptions::default().with_indent_width((params.options.tab_size as usize).max(1));
    format_edits(text, &options)
}

fn semantic_tokens_full(params: SemanticTokensParams, docs: &Docs) -> Option<SemanticTokens> {
    let text = docs.get(&params.text_document.uri)?;
    Some(SemanticTokens {
        result_id: None,
        data: semantic_tokens(text),
    })
}

fn hover_request(params: HoverParams, docs: &Docs) -> Option<Hover> {
    let position = params.text_document_position_params;
    let text = docs.get(&position.text_document.uri)?;
    hover(text, position.position)
}

fn folding_request(params: FoldingRangeParams, docs: &Docs) -> Option<Vec<FoldingRange>> {
    let text = docs.get(&params.text_document.uri)?;
    Some(folding_ranges(text))
}

fn handle_notification(
    connection: &Connection,
    notification: Notification,
    docs: &mut Docs,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    match notification.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams =
                notification.extract(DidOpenTextDocument::METHOD)?;
            let uri = params.text_document.uri;
            docs.insert(uri.clone(), params.text_document.text);
            publish_diagnostics(connection, docs, &uri)?;
        }
        DidChangeTextDocument::METHOD => {
            let params: DidChangeTextDocumentParams =
                notification.extract(DidChangeTextDocument::METHOD)?;
            let uri = params.text_document.uri;
            // Incremental sync: apply each change in order, splicing range edits and honoring
            // whole-document replacements (a change with no range).
            let mut text = docs.remove(&uri).unwrap_or_default();
            for change in params.content_changes {
                text = apply_change(&text, change.range, &change.text);
            }
            docs.insert(uri.clone(), text);
            publish_diagnostics(connection, docs, &uri)?;
        }
        DidCloseTextDocument::METHOD => {
            let params: DidCloseTextDocumentParams =
                notification.extract(DidCloseTextDocument::METHOD)?;
            let uri = params.text_document.uri;
            docs.remove(&uri);
            send_diagnostics(connection, uri, Vec::new())?; // clear on close
        }
        _ => {}
    }
    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    docs: &Docs,
    uri: &Uri,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let diags = docs
        .get(uri)
        .map(|text| diagnostics(text))
        .unwrap_or_default();
    send_diagnostics(connection, uri.clone(), diags)
}

fn send_diagnostics(
    connection: &Connection,
    uri: Uri,
    diagnostics: Vec<lsp_types::Diagnostic>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
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
fn cast<R>(request: Request) -> Result<(RequestId, R::Params), Response>
where
    R: lsp_types::request::Request,
{
    request.extract(R::METHOD).map_err(|err| match err {
        ExtractError::JsonError { method, error } => Response::new_err(
            // `extract` consumed the id on a JSON error; report against a fresh sentinel.
            RequestId::from(0),
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("invalid params for {method}: {error}"),
        ),
        ExtractError::MethodMismatch(request) => Response::new_err(
            request.id,
            lsp_server::ErrorCode::MethodNotFound as i32,
            format!("method mismatch: {}", request.method),
        ),
    })
}
