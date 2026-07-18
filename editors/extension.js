// Runtime entry point for the Snowflake SQL (sql-dialect-fmt) VS Code extension.
//
// The extension contributes a document (and range) formatter for the `snowflake-sql` language.
// Formatting runs entirely locally: the bundled `vendor/sql_dialect_fmt_wasm.wasm` module is the
// same WebAssembly build the Chrome extension uses, loaded here through Node's `WebAssembly` API.
// There is no network access, telemetry, or remote formatting — the SQL never leaves the machine.

const vscode = require("vscode");
const fs = require("fs");
const path = require("path");

const LANGUAGE_ID = "snowflake-sql";
const CONFIG_SECTION = "sqlDialectFmt";

// Instantiating the WebAssembly module is deferred until the first format request and then cached
// for the lifetime of the extension host.
let wasmInstancePromise = null;

function activate(context) {
  const selector = { language: LANGUAGE_ID };

  const documentProvider = {
    provideDocumentFormattingEdits(document) {
      return formatDocument(context, document);
    },
  };

  const rangeProvider = {
    provideDocumentRangeFormattingEdits(document, range) {
      return formatRange(context, document, range);
    },
  };

  context.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider(selector, documentProvider),
    vscode.languages.registerDocumentRangeFormattingEditProvider(selector, rangeProvider),
  );
}

function deactivate() {
  wasmInstancePromise = null;
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
