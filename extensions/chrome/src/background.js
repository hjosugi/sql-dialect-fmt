let wasmInstancePromise = null;

chrome.action.onClicked.addListener((tab) => {
  if (!tab.id) {
    return;
  }
  chrome.tabs.sendMessage(tab.id, { type: "snow-fmt:run" }).catch(() => {});
});

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (!message || message.type !== "snow-fmt:format") {
    return false;
  }

  formatSql(message.source, message.options)
    .then((formatted) => sendResponse({ ok: true, formatted }))
    .catch((error) => {
      sendResponse({
        ok: false,
        error: error instanceof Error ? error.message : String(error)
      });
    });
  return true;
});

async function formatSql(source, options = {}) {
  if (typeof source !== "string") {
    throw new Error("No SQL source was provided.");
  }

  const instance = await loadWasm();
  const api = instance.exports;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const input = encoder.encode(source);
  const inputPtr = api.snow_fmt_alloc(input.length);

  try {
    new Uint8Array(api.memory.buffer, inputPtr, input.length).set(input);
    const status = api.snow_fmt_format(
      inputPtr,
      input.length,
      normalizeInteger(options.lineWidth, 100),
      normalizeInteger(options.indentWidth, 4),
      options.uppercaseKeywords === false ? 0 : 1
    );

    if (status !== 0) {
      throw new Error(`Formatter failed with status ${status}.`);
    }

    const resultPtr = api.snow_fmt_result_ptr();
    const resultLen = api.snow_fmt_result_len();
    return decoder.decode(new Uint8Array(api.memory.buffer, resultPtr, resultLen));
  } finally {
    api.snow_fmt_dealloc(inputPtr, input.length);
    api.snow_fmt_clear_result();
  }
}

async function loadWasm() {
  if (!wasmInstancePromise) {
    wasmInstancePromise = (async () => {
      const response = await fetch(chrome.runtime.getURL("vendor/snow_fmt_wasm.wasm"));
      if (!response.ok) {
        throw new Error(`Failed to load snow_fmt_wasm.wasm: HTTP ${response.status}`);
      }
      const bytes = await response.arrayBuffer();
      const module = await WebAssembly.compile(bytes);
      const instance = await WebAssembly.instantiate(module, wasmImportsFor(module));
      return instance;
    })();
  }
  return wasmInstancePromise;
}

function wasmImportsFor(module) {
  const imports = {};
  for (const item of WebAssembly.Module.imports(module)) {
    imports[item.module] ||= {};

    if (item.kind !== "function") {
      throw new Error(`Unsupported WASM import ${item.module}.${item.name}`);
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
      throw new Error(`Unsupported WASM import ${item.module}.${item.name}`);
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
