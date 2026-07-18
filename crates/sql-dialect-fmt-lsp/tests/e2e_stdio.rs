//! End-to-end tests driving the real `sql-dialect-fmt-lsp` binary over stdio.
//!
//! These exercise the full LSP transport — `Content-Length` framing, `initialize` handshake
//! (including position-encoding negotiation), document lifecycle notifications with
//! `publishDiagnostics`, the formatting/hover/symbol/semantic-token requests, configuration
//! changes, and the `shutdown`/`exit` sequence — against the server the editors actually spawn.
//! Everything is hermetic: no network, and config discovery uses a private temp directory.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Value};

/// Generous upper bound for a single message so a wedged server fails the test instead of
/// hanging CI.
const RECV_TIMEOUT: Duration = Duration::from_secs(60);

/// A test client speaking framed JSON-RPC to a spawned `sql-dialect-fmt-lsp` process.
struct Server {
    child: Child,
    stdin: ChildStdin,
    incoming: mpsc::Receiver<Value>,
    /// Messages received while waiting for something else (e.g. diagnostics published before a
    /// response is read).
    buffered: VecDeque<Value>,
    reader: Option<JoinHandle<()>>,
    next_id: i64,
}

impl Server {
    /// Spawn the server binary and complete the `initialize`/`initialized` handshake. Returns the
    /// client and the raw `initialize` result for capability assertions.
    fn start(capabilities: Value, initialization_options: Value) -> (Self, Value) {
        let child = Command::new(env!("CARGO_BIN_EXE_sql-dialect-fmt-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sql-dialect-fmt-lsp");
        let mut server = Self::wrap(child);
        let result = server.request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": null,
                "capabilities": capabilities,
                "initializationOptions": initialization_options,
            }),
        );
        server.notify("initialized", json!({}));
        (server, result)
    }

    fn wrap(mut child: Child) -> Self {
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");
        let (sender, incoming) = mpsc::channel();
        let reader = std::thread::spawn(move || read_messages(stdout, &sender));
        Self {
            child,
            stdin,
            incoming,
            buffered: VecDeque::new(),
            reader: Some(reader),
            next_id: 0,
        }
    }

    fn send(&mut self, message: &Value) {
        let body = serde_json::to_string(message).expect("serialize message");
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)
            .and_then(|()| self.stdin.flush())
            .expect("write to server stdin");
    }

    /// Send a request and wait for its response, buffering any notifications that arrive first.
    /// Panics if the server answers with an error.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let message = self.recv();
            if message.get("id").and_then(Value::as_i64) == Some(id) {
                assert!(
                    message.get("error").is_none(),
                    "{method} answered with an error: {message}"
                );
                return message.get("result").cloned().unwrap_or(Value::Null);
            }
            self.buffered.push_back(message);
        }
    }

    /// Send a request expected to fail; returns the JSON-RPC error object.
    fn request_err(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let message = self.recv();
            if message.get("id").and_then(Value::as_i64) == Some(id) {
                return message.get("error").cloned().expect("error response");
            }
            self.buffered.push_back(message);
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Wait for the next notification with `method`, buffering everything else.
    fn notification(&mut self, method: &str) -> Value {
        if let Some(index) = self
            .buffered
            .iter()
            .position(|message| message.get("method").and_then(Value::as_str) == Some(method))
        {
            let message = self.buffered.remove(index).expect("buffered message");
            return message.get("params").cloned().unwrap_or(Value::Null);
        }
        loop {
            let message = self.recv();
            if message.get("method").and_then(Value::as_str) == Some(method) {
                return message.get("params").cloned().unwrap_or(Value::Null);
            }
            self.buffered.push_back(message);
        }
    }

    fn recv(&mut self) -> Value {
        self.incoming
            .recv_timeout(RECV_TIMEOUT)
            .expect("message from server before timeout")
    }

    /// Run the `shutdown` request + `exit` notification sequence and reap the process.
    fn shutdown(mut self) {
        let result = self.request("shutdown", Value::Null);
        assert_eq!(result, Value::Null, "shutdown result should be null");
        self.notify("exit", json!(null));
        let status = self.child.wait().expect("server exit");
        assert!(status.success(), "server exited with {status}");
        if let Some(reader) = self.reader.take() {
            reader.join().expect("reader thread");
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Don't leak a server if an assertion failed mid-test.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Decode `Content-Length`-framed JSON-RPC messages until the stream closes.
fn read_messages(stdout: impl Read, sender: &mpsc::Sender<Value>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return; // EOF
            }
            let line = line.trim_end();
            if line.is_empty() {
                break; // end of headers
            }
            if let Some(value) = line
                .strip_prefix("Content-Length:")
                .map(str::trim)
                .and_then(|value| value.parse().ok())
            {
                content_length = Some(value);
            }
        }
        let Some(length) = content_length else { return };
        let mut body = vec![0u8; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        let Ok(message) = serde_json::from_slice(&body) else {
            return;
        };
        if sender.send(message).is_err() {
            return;
        }
    }
}

/// Build a `file://` URI from an absolute path, cross-platform (Windows drive paths need a
/// leading slash and forward separators).
fn file_uri(path: &Path) -> String {
    let mut text = path.display().to_string().replace('\\', "/");
    if !text.starts_with('/') {
        text.insert(0, '/');
    }
    format!("file://{text}")
}

fn did_open(server: &mut Server, uri: &str, version: i64, text: &str) {
    server.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": "sql",
                "version": version,
                "text": text,
            }
        }),
    );
}

const FORMATTING_OPTIONS: fn() -> Value = || json!({ "tabSize": 4, "insertSpaces": true });

const TEST_URI: &str = "file:///workspace/query.sql";

#[test]
fn initialize_defaults_to_utf16_and_advertises_capabilities() {
    let (mut server, result) = Server::start(json!({}), json!(null));
    let capabilities = &result["capabilities"];

    assert_eq!(capabilities["positionEncoding"], "utf-16");
    assert_eq!(capabilities["textDocumentSync"], 2); // incremental
    assert_eq!(capabilities["documentFormattingProvider"], true);
    assert_eq!(capabilities["documentRangeFormattingProvider"], true);
    assert_eq!(
        capabilities["documentOnTypeFormattingProvider"]["firstTriggerCharacter"],
        ";"
    );
    assert_eq!(capabilities["hoverProvider"], true);
    assert_eq!(capabilities["foldingRangeProvider"], true);
    let legend = &capabilities["semanticTokensProvider"]["legend"];
    assert_eq!(legend["tokenTypes"][0], "keyword");
    assert_eq!(
        legend["tokenModifiers"],
        json!(["documentation", "defaultLibrary"])
    );
    assert_eq!(capabilities["semanticTokensProvider"]["range"], true);

    // Unknown request methods are answered, not dropped.
    let error = server.request_err(
        "textDocument/references",
        json!({
            "textDocument": { "uri": TEST_URI },
            "position": { "line": 0, "character": 0 },
            "context": { "includeDeclaration": false },
        }),
    );
    assert_eq!(error["code"], -32601); // MethodNotFound

    server.shutdown();
}

#[test]
fn did_open_publishes_lint_diagnostics_and_formatting_edits_the_document() {
    let (mut server, _) = Server::start(json!({}), json!(null));

    did_open(&mut server, TEST_URI, 3, "select * from t");
    let params = server.notification("textDocument/publishDiagnostics");
    assert_eq!(params["uri"], TEST_URI);
    assert_eq!(params["version"], 3);
    let diagnostics = params["diagnostics"].as_array().expect("diagnostics");
    let wildcard = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["code"] == "SDF001")
        .expect("SELECT * lint diagnostic with code");
    assert_eq!(wildcard["source"], "sql-dialect-fmt");
    assert_eq!(wildcard["severity"], 2); // warning
    assert!(wildcard["message"]
        .as_str()
        .expect("message")
        .contains("avoid SELECT *"));

    // Whole-document formatting returns one replacement edit.
    let edits = server.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": TEST_URI }, "options": FORMATTING_OPTIONS() }),
    );
    assert_eq!(edits.as_array().map(Vec::len), Some(1));
    assert_eq!(edits[0]["newText"], "SELECT *\nFROM t;\n");
    assert_eq!(
        edits[0]["range"]["start"],
        json!({ "line": 0, "character": 0 })
    );

    // A whole-document didChange (no range) replaces the text and republishes diagnostics.
    server.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": TEST_URI, "version": 4 },
            "contentChanges": [{ "text": "SELECT a\nFROM t;\n" }],
        }),
    );
    let params = server.notification("textDocument/publishDiagnostics");
    assert_eq!(params["version"], 4);
    assert_eq!(params["diagnostics"], json!([]));

    // Already formatted: no edits.
    let edits = server.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": TEST_URI }, "options": FORMATTING_OPTIONS() }),
    );
    assert_eq!(edits, json!([]));

    // Closing clears diagnostics at the last seen version.
    server.notify(
        "textDocument/didClose",
        json!({ "textDocument": { "uri": TEST_URI } }),
    );
    let params = server.notification("textDocument/publishDiagnostics");
    assert_eq!(params["version"], 4);
    assert_eq!(params["diagnostics"], json!([]));

    server.shutdown();
}

#[test]
fn range_and_on_type_formatting_touch_only_the_finished_statement() {
    let (mut server, _) = Server::start(json!({}), json!(null));
    did_open(&mut server, TEST_URI, 1, "SELECT 1;\nselect a,b from t;\n");
    let params = server.notification("textDocument/publishDiagnostics");
    assert_eq!(params["diagnostics"], json!([]));

    let edits = server.request(
        "textDocument/rangeFormatting",
        json!({
            "textDocument": { "uri": TEST_URI },
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 1, "character": 3 },
            },
            "options": FORMATTING_OPTIONS(),
        }),
    );
    assert_eq!(edits.as_array().map(Vec::len), Some(1));
    assert_eq!(edits[0]["newText"], "SELECT a, b\nFROM t;");
    assert_eq!(
        edits[0]["range"]["start"],
        json!({ "line": 1, "character": 0 })
    );

    // Typing the terminating `;` reformats the statement that just ended.
    let edits = server.request(
        "textDocument/onTypeFormatting",
        json!({
            "textDocument": { "uri": TEST_URI },
            "position": { "line": 1, "character": 18 },
            "ch": ";",
            "options": FORMATTING_OPTIONS(),
        }),
    );
    assert_eq!(edits.as_array().map(Vec::len), Some(1));
    assert_eq!(edits[0]["newText"], "SELECT a, b\nFROM t;");
    assert_eq!(
        edits[0]["range"]["start"],
        json!({ "line": 1, "character": 0 })
    );

    // The already formatted first statement yields no on-type edits.
    let edits = server.request(
        "textDocument/onTypeFormatting",
        json!({
            "textDocument": { "uri": TEST_URI },
            "position": { "line": 0, "character": 9 },
            "ch": ";",
            "options": FORMATTING_OPTIONS(),
        }),
    );
    assert_eq!(edits, json!([]));

    server.shutdown();
}

#[test]
fn hover_document_symbols_and_semantic_tokens_answer_over_stdio() {
    let (mut server, _) = Server::start(json!({}), json!(null));
    let text =
        "CREATE TABLE db.t (id INT);\n\nSELECT dateadd(day, 1, id)\nFROM db.t\nWHERE id = 1;\n";
    did_open(&mut server, TEST_URI, 1, text);
    let params = server.notification("textDocument/publishDiagnostics");
    assert_eq!(params["diagnostics"], json!([]));

    // Hover over `dateadd` (line 2, column 7) returns Markdown with the signature.
    let hover = server.request(
        "textDocument/hover",
        json!({
            "textDocument": { "uri": TEST_URI },
            "position": { "line": 2, "character": 7 },
        }),
    );
    assert_eq!(hover["contents"]["kind"], "markdown");
    assert!(hover["contents"]["value"]
        .as_str()
        .expect("hover markdown")
        .contains("DATEADD( <date_or_time_part>"));

    // Document symbols outline both top-level statements.
    let symbols = server.request(
        "textDocument/documentSymbol",
        json!({ "textDocument": { "uri": TEST_URI } }),
    );
    assert_eq!(symbols.as_array().map(Vec::len), Some(2));
    assert_eq!(symbols[0]["name"], "CREATE TABLE db.t");
    assert_eq!(symbols[0]["kind"], 23); // Struct
    assert_eq!(symbols[1]["name"], "SELECT");
    assert_eq!(symbols[1]["kind"], 12); // Function

    // Full semantic tokens: the first token is the CREATE keyword with the defaultLibrary
    // modifier, and a result id is attached for delta requests.
    let tokens = server.request(
        "textDocument/semanticTokens/full",
        json!({ "textDocument": { "uri": TEST_URI } }),
    );
    assert!(tokens["resultId"].is_string());
    let data = tokens["data"].as_array().expect("token data");
    assert!(data.len() >= 5);
    assert_eq!(
        &data[..5],
        &[json!(0), json!(0), json!(6), json!(0), json!(2)]
    );

    // Range request: only tokens on `FROM db.t` (line 3) are returned.
    let tokens = server.request(
        "textDocument/semanticTokens/range",
        json!({
            "textDocument": { "uri": TEST_URI },
            "range": {
                "start": { "line": 3, "character": 0 },
                "end": { "line": 4, "character": 0 },
            },
        }),
    );
    let data = tokens["data"].as_array().expect("token data");
    assert!(!data.is_empty());
    assert_eq!(data.len() % 5, 0);
    assert_eq!(data[0], json!(3)); // first token re-encoded relative to line 0
    let mut line = 0u64;
    for chunk in data.chunks(5) {
        line += chunk[0].as_u64().expect("delta line");
        assert_eq!(line, 3, "range tokens must all stay on line 3");
    }

    server.shutdown();
}

#[test]
fn utf8_position_encoding_is_negotiated_and_applied() {
    let (mut server, result) = Server::start(
        json!({ "general": { "positionEncodings": ["utf-8", "utf-16"] } }),
        json!(null),
    );
    assert_eq!(result["capabilities"]["positionEncoding"], "utf-8");

    // `芋` is three UTF-8 bytes but one UTF-16 unit, so the lint diagnostic on `*` lands at byte
    // column 14 (it would be 12 under UTF-16).
    did_open(&mut server, TEST_URI, 1, "SELECT '芋', * FROM t;");
    let params = server.notification("textDocument/publishDiagnostics");
    let diagnostics = params["diagnostics"].as_array().expect("diagnostics");
    let wildcard = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["code"] == "SDF001")
        .expect("SELECT * lint diagnostic");
    assert_eq!(
        wildcard["range"]["start"],
        json!({ "line": 0, "character": 14 })
    );

    // Incremental didChange ranges are interpreted as UTF-8 byte columns: replace the multibyte
    // character (bytes 8..11) with ASCII and the wildcard moves to column 13.
    server.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": TEST_URI, "version": 2 },
            "contentChanges": [{
                "range": {
                    "start": { "line": 0, "character": 8 },
                    "end": { "line": 0, "character": 11 },
                },
                "text": "im",
            }],
        }),
    );
    let params = server.notification("textDocument/publishDiagnostics");
    assert_eq!(params["version"], 2);
    let diagnostics = params["diagnostics"].as_array().expect("diagnostics");
    let wildcard = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["code"] == "SDF001")
        .expect("SELECT * lint diagnostic after edit");
    assert_eq!(
        wildcard["range"]["start"],
        json!({ "line": 0, "character": 13 })
    );

    server.shutdown();
}

#[test]
fn did_change_configuration_switches_dialect_and_republishes_diagnostics() {
    let (mut server, _) = Server::start(json!({}), json!(null));

    // `<=>` is Databricks-only: Snowflake parsing reports an error diagnostic.
    did_open(&mut server, TEST_URI, 1, "SELECT a <=> b FROM t;");
    let params = server.notification("textDocument/publishDiagnostics");
    let diagnostics = params["diagnostics"].as_array().expect("diagnostics");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["severity"] == 1),
        "expected an error diagnostic under the Snowflake dialect: {diagnostics:?}"
    );

    // Switching the dialect republishes diagnostics for the open document without a new edit.
    server.notify(
        "workspace/didChangeConfiguration",
        json!({ "settings": { "sqlDialectFmt": { "dialect": "databricks" } } }),
    );
    let params = server.notification("textDocument/publishDiagnostics");
    assert_eq!(params["uri"], TEST_URI);
    assert_eq!(params["version"], 1);
    assert_eq!(params["diagnostics"], json!([]));

    // Formatting now succeeds under the configured dialect.
    let edits = server.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": TEST_URI }, "options": FORMATTING_OPTIONS() }),
    );
    assert_eq!(edits[0]["newText"], "SELECT a <=> b\nFROM t;\n");

    server.shutdown();
}

#[test]
fn nearest_config_file_is_discovered_and_editor_settings_win() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("sql-dialect-fmt.toml"),
        "keyword_case = \"lower\"\n",
    )
    .expect("write config");
    let uri = file_uri(&dir.path().join("query.sql"));

    let (mut server, _) = Server::start(json!({}), json!(null));
    did_open(&mut server, &uri, 1, "SELECT a,b FROM t");
    server.notification("textDocument/publishDiagnostics");

    // The config file next to the document drives keyword casing (insertSpaces=false keeps the
    // editor's tab settings out of the way).
    let edits = server.request(
        "textDocument/formatting",
        json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": false },
        }),
    );
    assert_eq!(edits[0]["newText"], "select a, b\nfrom t;\n");

    // Editor settings overlay the config file, field by field.
    server.notify(
        "workspace/didChangeConfiguration",
        json!({ "settings": { "sqlDialectFmt": { "keywordCase": "upper" } } }),
    );
    server.notification("textDocument/publishDiagnostics");
    let edits = server.request(
        "textDocument/formatting",
        json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": false },
        }),
    );
    assert_eq!(edits[0]["newText"], "SELECT a, b\nFROM t;\n");

    server.shutdown();
}
