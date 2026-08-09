"use strict";

const fs = require("fs");
const path = require("path");

let wasmInstancePromise = null;

async function formatText(context, source, options) {
  const instance = await loadWasm(context);
  const api = instance.exports;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const input = encoder.encode(source);
  const inputPtr = api.sql_dialect_fmt_alloc(input.length);

  try {
    new Uint8Array(api.memory.buffer, inputPtr, input.length).set(input);
    const status = callFormatter(api, inputPtr, input.length, options);
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

function callFormatter(api, inputPtr, inputLength, options) {
  return api.sql_dialect_fmt_format_with_options(
    inputPtr,
    inputLength,
    options.lineWidth,
    options.indentWidth,
    enumCode(options.keywordCase, ["upper", "lower", "preserve"]),
    enumCode(options.selectItemLayout, ["auto", "vertical"]),
    enumCode(options.commaStyle, ["trailing", "leading"]),
    enumCode(options.lineEnding, ["auto", "lf", "crlf"]),
    enumCode(options.dialect, ["snowflake", "databricks"]),
  );
}

function enumCode(value, variants) {
  const index = variants.indexOf(value);
  return index < 0 ? 0 : index;
}

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
      const instance = await WebAssembly.instantiate(module, wasmImportsFor(module));
      validateApi(instance.exports);
      return instance;
    })().catch((error) => {
      wasmInstancePromise = null;
      throw error;
    });
  }
  return wasmInstancePromise;
}

function validateApi(api) {
  if (!(api.memory instanceof WebAssembly.Memory)) {
    throw new Error("bundled formatter does not export WebAssembly memory");
  }
  for (const name of [
    "sql_dialect_fmt_alloc",
    "sql_dialect_fmt_dealloc",
    "sql_dialect_fmt_format_with_options",
    "sql_dialect_fmt_result_ptr",
    "sql_dialect_fmt_result_len",
    "sql_dialect_fmt_clear_result",
  ]) {
    if (typeof api[name] !== "function") {
      throw new Error(`bundled formatter is missing required export ${name}`);
    }
  }
}

function resetWasm() {
  wasmInstancePromise = null;
}

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

function messageOf(error) {
  return error instanceof Error ? error.message : String(error);
}

module.exports = { formatText, resetWasm };
