// Runtime entry point for the Snowflake SQL (sql-dialect-fmt) VS Code extension.
//
// The extension contributes a document (and range) formatter for the `snowflake-sql` language.
// Formatting runs entirely locally: the bundled `vendor/sql_dialect_fmt_wasm.wasm` module is the
// same WebAssembly build the Chrome extension uses, loaded here through Node's `WebAssembly` API.
// There is no network access, telemetry, or remote formatting — the SQL never leaves the machine.
//
// Optionally (`sqlDialectFmt.lsp.enabled`, off by default), the extension starts the
// `sql-dialect-fmt-lsp` language server for diagnostics, hover, completion, semantic tokens,
// document symbols, folding, and on-type formatting. The server also runs locally over stdio.
// While the server is running it takes over formatting (the WebAssembly providers are
// unregistered so documents never see two competing formatters); when the binary is missing or
// fails to start, the extension logs to the "sql-dialect-fmt" output channel and keeps the
// bundled WebAssembly formatter — never an error popup at startup.

const vscode = require("vscode");
const fs = require("fs");
const os = require("os");
const path = require("path");

const LANGUAGE_ID = "snowflake-sql";
const CONFIG_SECTION = "sqlDialectFmt";
const SERVER_BINARY_NAME = "sql-dialect-fmt-lsp";
// `sqlDialectFmt` settings forwarded to the language server. `lsp.*` is client-only.
const SERVER_SETTING_KEYS = ["dialect", "lineWidth", "indentWidth", "uppercaseKeywords", "lint"];

// Instantiating the WebAssembly module is deferred until the first format request and then cached
// for the lifetime of the extension host.
let wasmInstancePromise = null;

// Client-mode state: either the WebAssembly formatter providers are registered, or the language
// server client is running — never both. Transitions are serialized through `modeTransition`.
let outputChannel = null;
let lspClient = null;
let wasmProviderRegistrations = [];
let modeTransition = Promise.resolve();

function activate(context) {
  outputChannel = vscode.window.createOutputChannel("sql-dialect-fmt");
  context.subscriptions.push(
    outputChannel,
    new vscode.Disposable(disposeWasmProviders),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration(`${CONFIG_SECTION}.lsp`)) {
        scheduleModeSync(context);
      }
    }),
  );
  scheduleModeSync(context);
}

function deactivate() {
  wasmInstancePromise = null;
  const client = lspClient;
  lspClient = null;
  return client ? client.stop() : undefined;
}

/** Queue a (re)evaluation of the formatter/LSP mode; transitions run one at a time. */
function scheduleModeSync(context) {
  modeTransition = modeTransition
    .then(() => applyMode(context))
    .catch((error) => log(`failed to apply the sqlDialectFmt.lsp settings: ${messageOf(error)}`));
  return modeTransition;
}

/**
 * Reconcile the runtime with the `sqlDialectFmt.lsp.*` settings: start the language server when
 * it is enabled and its binary exists, otherwise fall back to the bundled WebAssembly formatter.
 */
async function applyMode(context) {
  const config = vscode.workspace.getConfiguration(CONFIG_SECTION);
  const command = config.get("lsp.enabled", false) === true ? resolveServerCommand(config) : null;

  // The server (re)start below is not conditional on the command changing; a same-path restart
  // only happens when a `sqlDialectFmt.lsp` setting was edited, where reloading is what users
  // expect. Everything else (`sqlDialectFmt.lineWidth`, …) flows through `synchronize` without
  // restarting.
  await stopLspClient();
  if (command && (await startLspClient(context, command))) {
    disposeWasmProviders();
    return;
  }
  ensureWasmProviders(context);
}

/** Resolve the server executable, logging (never popping up) why the LSP stays off. */
function resolveServerCommand(config) {
  const configured = String(config.get("lsp.path", "") || "").trim();
  if (configured) {
    const candidate = expandHomePath(configured);
    if (isExecutableFile(candidate)) {
      return candidate;
    }
    log(
      `sqlDialectFmt.lsp.path is set to "${configured}" but no executable exists there; ` +
        "falling back to the bundled WebAssembly formatter.",
    );
    return null;
  }
  const found = findOnPath(SERVER_BINARY_NAME);
  if (!found) {
    log(
      `sqlDialectFmt.lsp.enabled is on but "${SERVER_BINARY_NAME}" was not found on PATH; ` +
        "falling back to the bundled WebAssembly formatter. " +
        "Install the server with `cargo install sql-dialect-fmt-lsp` " +
        "or point sqlDialectFmt.lsp.path at the binary.",
    );
  }
  return found;
}

/** Start the language client for `command`; on any failure log and report `false`. */
async function startLspClient(context, command) {
  let languageclient;
  try {
    // Lazy so a source checkout without `npm install` still provides the WebAssembly formatter.
    languageclient = require("vscode-languageclient/node");
  } catch (error) {
    log(
      `the vscode-languageclient module could not be loaded (${messageOf(error)}); ` +
        "falling back to the bundled WebAssembly formatter.",
    );
    return false;
  }

  const serverOptions = { command, args: [] };
  const health = { initialized: false };
  const clientOptions = {
    documentSelector: [{ language: LANGUAGE_ID }],
    outputChannel,
    revealOutputChannelOn: languageclient.RevealOutputChannelOn.Never,
    initializationOptions: () => ({ [CONFIG_SECTION]: serverSettingsSnapshot() }),
    synchronize: { configurationSection: CONFIG_SECTION },
    initializationFailedHandler: (error) => {
      log(`language server initialization failed: ${messageOf(error)}`);
      return false;
    },
    errorHandler: makeErrorHandler(context, languageclient, health),
  };
  const client = new languageclient.LanguageClient(
    "sqlDialectFmtLsp",
    "sql-dialect-fmt language server",
    serverOptions,
    clientOptions,
  );

  try {
    await client.start();
    health.initialized = true;
  } catch (error) {
    log(
      `could not start "${command}" (${messageOf(error)}); ` +
        "falling back to the bundled WebAssembly formatter.",
    );
    try {
      await client.dispose();
    } catch {
      // The client never became healthy; there is nothing more to clean up.
    }
    return false;
  }
  lspClient = client;
  log(`language server started: ${command}`);
  return true;
}

/**
 * Like vscode-languageclient's default handler (restart the server, up to 4 crashes in 3
 * minutes) but without its error popups, and with a final fallback: when the server keeps
 * crashing — or never comes up at all — re-register the WebAssembly formatters so formatting
 * keeps working.
 */
function makeErrorHandler(context, languageclient, health) {
  let crashTimes = [];
  const giveUp = (reason) => {
    log(`${reason}; falling back to the bundled WebAssembly formatter.`);
    modeTransition = modeTransition
      .then(async () => {
        await stopLspClient();
        ensureWasmProviders(context);
      })
      .catch((fallbackError) => log(`wasm fallback failed: ${messageOf(fallbackError)}`));
    return { action: languageclient.CloseAction.DoNotRestart, handled: true };
  };
  return {
    error(error) {
      log(`language server connection error: ${messageOf(error)}`);
      return { action: languageclient.ErrorAction.Continue, handled: true };
    },
    closed() {
      if (!health.initialized) {
        // The server never completed a handshake; restarting the same broken binary would loop.
        return giveUp("the language server exited before finishing the initialize handshake");
      }
      crashTimes.push(Date.now());
      crashTimes = crashTimes.filter((time) => Date.now() - time <= 3 * 60 * 1000);
      if (crashTimes.length <= 4) {
        log("the language server connection closed; restarting it.");
        return { action: languageclient.CloseAction.Restart, handled: true };
      }
      return giveUp("the language server keeps crashing; giving up on it");
    },
  };
}

async function stopLspClient() {
  const client = lspClient;
  if (!client) {
    return;
  }
  lspClient = null;
  try {
    await client.dispose();
  } catch (error) {
    log(`error while stopping the language server: ${messageOf(error)}`);
  }
}

/** Settings snapshot forwarded to the server (`initializationOptions`, nested per its schema). */
function serverSettingsSnapshot() {
  const config = vscode.workspace.getConfiguration(CONFIG_SECTION);
  const snapshot = {};
  for (const key of SERVER_SETTING_KEYS) {
    const value = config.get(key);
    if (value !== undefined) {
      snapshot[key] = value;
    }
  }
  return snapshot;
}

/** Register the WebAssembly document/range formatters unless they are already registered. */
function ensureWasmProviders(context) {
  if (wasmProviderRegistrations.length > 0) {
    return;
  }
  const selector = { language: LANGUAGE_ID };
  wasmProviderRegistrations = [
    vscode.languages.registerDocumentFormattingEditProvider(selector, {
      provideDocumentFormattingEdits(document) {
        return formatDocument(context, document);
      },
    }),
    vscode.languages.registerDocumentRangeFormattingEditProvider(selector, {
      provideDocumentRangeFormattingEdits(document, range) {
        return formatRange(context, document, range);
      },
    }),
  ];
}

function disposeWasmProviders() {
  for (const registration of wasmProviderRegistrations.splice(0)) {
    registration.dispose();
  }
}

/** Locate `binary` on PATH, honoring PATHEXT on Windows. Returns the full path or `null`. */
function findOnPath(binary) {
  const suffixes =
    process.platform === "win32"
      ? (process.env.PATHEXT || ".EXE;.CMD;.BAT;.COM").split(";").filter(Boolean)
      : [""];
  for (const directory of (process.env.PATH || "").split(path.delimiter)) {
    if (!directory) {
      continue;
    }
    for (const suffix of suffixes) {
      const candidate = path.join(directory, binary + suffix.toLowerCase());
      if (isExecutableFile(candidate)) {
        return candidate;
      }
    }
  }
  return null;
}

function isExecutableFile(candidate) {
  try {
    if (!fs.statSync(candidate).isFile()) {
      return false;
    }
    if (process.platform !== "win32") {
      fs.accessSync(candidate, fs.constants.X_OK);
    }
    return true;
  } catch {
    return false;
  }
}

function expandHomePath(value) {
  if (value === "~") {
    return os.homedir();
  }
  if (value.startsWith("~/") || value.startsWith("~\\")) {
    return path.join(os.homedir(), value.slice(2));
  }
  return value;
}

function log(message) {
  if (outputChannel) {
    outputChannel.appendLine(message);
  }
}

/** Format the whole document and return a single replacement edit (or nothing when unchanged). */
async function formatDocument(context, document) {
  try {
    const original = document.getText();
    const formatted = await formatText(context, original, readOptions());
    if (formatted === original) {
      return [];
    }
    const fullRange = new vscode.Range(
      document.positionAt(0),
      document.positionAt(original.length),
    );
    return [vscode.TextEdit.replace(fullRange, formatted)];
  } catch (error) {
    reportError(error);
    return [];
  }
}

/** Format the selected range. Unparseable fragments pass through unchanged (the format is lossless). */
async function formatRange(context, document, range) {
  try {
    const original = document.getText(range);
    let formatted = await formatText(context, original, readOptions());
    // The formatter always emits a trailing newline. When the selection is an inline fragment that
    // did not end with one, dropping it keeps a "Format Selection" from splicing a stray newline in.
    if (!original.endsWith("\n") && formatted.endsWith("\n")) {
      formatted = formatted.replace(/\r?\n$/, "");
    }
    if (formatted === original) {
      return [];
    }
    return [vscode.TextEdit.replace(range, formatted)];
  } catch (error) {
    reportError(error);
    return [];
  }
}

/** Resolve formatter options from the `sqlDialectFmt.*` workspace settings. */
function readOptions() {
  const config = vscode.workspace.getConfiguration(CONFIG_SECTION);
  return {
    dialect: config.get("dialect", "snowflake") === "databricks" ? "databricks" : "snowflake",
    lineWidth: normalizeInteger(config.get("lineWidth", 100), 100),
    indentWidth: normalizeInteger(config.get("indentWidth", 4), 4),
    uppercaseKeywords: config.get("uppercaseKeywords", true) !== false,
  };
}

/** Run `source` through the WebAssembly formatter and return the formatted UTF-8 text. */
async function formatText(context, source, options) {
  const instance = await loadWasm(context);
  const api = instance.exports;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const input = encoder.encode(source);
  const inputPtr = api.sql_dialect_fmt_alloc(input.length);

  try {
    // Views are taken after each call that may grow (and therefore detach) the memory buffer.
    new Uint8Array(api.memory.buffer, inputPtr, input.length).set(input);
    const format = api.sql_dialect_fmt_format_with_dialect || api.sql_dialect_fmt_format;
    const dialect = options.dialect === "databricks" ? 1 : 0;
    const status = format(
      inputPtr,
      input.length,
      options.lineWidth,
      options.indentWidth,
      options.uppercaseKeywords ? 1 : 0,
      dialect,
    );

    if (status !== 0) {
      throw new Error(`formatter returned status ${status}`);
    }

    const resultPtr = api.sql_dialect_fmt_result_ptr();
    const resultLen = api.sql_dialect_fmt_result_len();
    return decoder.decode(new Uint8Array(api.memory.buffer, resultPtr, resultLen));
  } finally {
    api.sql_dialect_fmt_dealloc(inputPtr, input.length);
    api.sql_dialect_fmt_clear_result();
  }
}

/** Compile and instantiate the bundled WebAssembly formatter, memoizing the instance. */
function loadWasm(context) {
  if (!wasmInstancePromise) {
    wasmInstancePromise = (async () => {
      const wasmPath = context.asAbsolutePath(path.join("vendor", "sql_dialect_fmt_wasm.wasm"));
      let bytes;
      try {
        bytes = await fs.promises.readFile(wasmPath);
      } catch (error) {
        throw new Error(`could not read the bundled formatter at ${wasmPath}: ${messageOf(error)}`);
      }
      const module = await WebAssembly.compile(bytes);
      return WebAssembly.instantiate(module, wasmImportsFor(module));
    })().catch((error) => {
      // Do not cache a failed load; a later attempt should be able to retry.
      wasmInstancePromise = null;
      throw error;
    });
  }
  return wasmInstancePromise;
}

// The raw (non-`wasm-bindgen`) build normally imports nothing, but tolerate the placeholder imports
// a `wasm-bindgen` toolchain would emit so the loader keeps working across build configurations.
function wasmImportsFor(module) {
  const imports = {};
  for (const item of WebAssembly.Module.imports(module)) {
    imports[item.module] ||= {};

    if (item.kind !== "function") {
      throw new Error(`unsupported WASM import ${item.module}.${item.name}`);
    }

    if (item.module === "__wbindgen_placeholder__" && item.name === "__wbindgen_describe") {
      imports[item.module][item.name] = () => {};
    } else if (
      item.module === "__wbindgen_placeholder__" &&
      item.name.startsWith("__wbg___wbindgen_throw_")
    ) {
      imports[item.module][item.name] = (ptr, len) => {
        throw new Error(`wasm-bindgen throw at ${ptr}:${len}`);
      };
    } else if (
      item.module === "__wbindgen_externref_xform__" &&
      item.name === "__wbindgen_externref_table_set_null"
    ) {
      imports[item.module][item.name] = () => {};
    } else if (
      item.module === "__wbindgen_externref_xform__" &&
      item.name === "__wbindgen_externref_table_grow"
    ) {
      imports[item.module][item.name] = () => -1;
    } else {
      throw new Error(`unsupported WASM import ${item.module}.${item.name}`);
    }
  }
  return imports;
}

function normalizeInteger(value, fallback) {
  const number = Number(value);
  if (!Number.isInteger(number) || number <= 0) {
    return fallback;
  }
  return number;
}

function reportError(error) {
  vscode.window.showErrorMessage(`sql-dialect-fmt: ${messageOf(error)}`);
}

function messageOf(error) {
  return error instanceof Error ? error.message : String(error);
}

module.exports = { activate, deactivate };
