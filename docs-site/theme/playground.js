(() => {
  if (typeof document !== "undefined") {
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", init);
    } else {
      init();
    }
  }

  function init() {
    const app = document.querySelector("#playground-app");
    if (!app) {
      return;
    }
    renderShell(app);

    const input = app.querySelector("#playground-input");
    const output = app.querySelector("#playground-output");
    const dialect = app.querySelector("#playground-dialect");
    const lineWidth = app.querySelector("#playground-line-width");
    const indentWidth = app.querySelector("#playground-indent-width");
    const keywordCase = app.querySelector("#playground-keyword-case");
    const selectItemLayout = app.querySelector("#playground-select-layout");
    const commaStyle = app.querySelector("#playground-comma-style");
    const lineEnding = app.querySelector("#playground-line-ending");
    const formatButton = app.querySelector("#playground-format");
    const copyButton = app.querySelector("#playground-copy");
    const status = app.querySelector("#playground-status");

    let wasmInstancePromise = null;

    formatButton.addEventListener("click", async () => {
      await runFormat();
    });
    copyButton.addEventListener("click", async () => {
      await navigator.clipboard.writeText(output.value || "");
      setStatus("Copied");
    });

    runFormat();

    async function runFormat() {
      formatButton.disabled = true;
      setStatus("Formatting");
      try {
        output.value = await formatSql(input.value, {
          dialect: dialect.value,
          lineWidth: normalizeInteger(lineWidth.value, 80),
          indentWidth: normalizeInteger(indentWidth.value, 2),
          keywordCase: keywordCase.value,
          selectItemLayout: selectItemLayout.value,
          commaStyle: commaStyle.value,
          lineEnding: lineEnding.value,
        });
        setStatus(output.value === input.value ? "Already formatted" : "Formatted");
      } catch (error) {
        setStatus(error instanceof Error ? error.message : String(error));
      } finally {
        formatButton.disabled = false;
      }
    }

    async function formatSql(source, options) {
      const instance = await loadWasm();
      const api = instance.exports;
      const encoder = new TextEncoder();
      const decoder = new TextDecoder();
      const bytes = encoder.encode(source);
      const inputPtr = api.sql_dialect_fmt_alloc(bytes.length);

      try {
        new Uint8Array(api.memory.buffer, inputPtr, bytes.length).set(bytes);
        const result = callFormatter(api, inputPtr, bytes.length, options);
        if (result !== 0) {
          throw new Error(`Formatter failed with status ${result}`);
        }
        const resultPtr = api.sql_dialect_fmt_result_ptr();
        const resultLen = api.sql_dialect_fmt_result_len();
        return decoder.decode(new Uint8Array(api.memory.buffer, resultPtr, resultLen));
      } finally {
        api.sql_dialect_fmt_dealloc(inputPtr, bytes.length);
        api.sql_dialect_fmt_clear_result();
      }
    }

    async function loadWasm() {
      if (!wasmInstancePromise) {
        wasmInstancePromise = (async () => {
          const wasmUrl = new URL("sql_dialect_fmt_wasm.wasm", document.baseURI);
          const response = await fetch(wasmUrl);
          if (!response.ok) {
            throw new Error(`Failed to load formatter WASM: HTTP ${response.status}`);
          }
          const bytes = await response.arrayBuffer();
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

    function setStatus(message) {
      status.textContent = message;
    }
  }

  function renderShell(app) {
    app.innerHTML = `
      <div class="playground-toolbar" aria-label="Formatter options">
        <label>
          <span>Dialect</span>
          <select id="playground-dialect">
            <option value="snowflake">Snowflake</option>
            <option value="databricks">Databricks</option>
          </select>
        </label>
        <label>
          <span>Line width</span>
          <input id="playground-line-width" type="number" min="1" max="240" value="80">
        </label>
        <label>
          <span>Indent</span>
          <input id="playground-indent-width" type="number" min="1" max="16" value="2">
        </label>
        <label>
          <span>Keyword case</span>
          <select id="playground-keyword-case">
            <option value="upper">Upper</option>
            <option value="lower">Lower</option>
            <option value="preserve">Preserve</option>
          </select>
        </label>
        <label>
          <span>SELECT items</span>
          <select id="playground-select-layout">
            <option value="vertical">Vertical</option>
            <option value="auto">Auto</option>
          </select>
        </label>
        <label>
          <span>Commas</span>
          <select id="playground-comma-style">
            <option value="trailing">Trailing</option>
            <option value="leading">Leading</option>
          </select>
        </label>
        <label>
          <span>Line endings</span>
          <select id="playground-line-ending">
            <option value="auto">Auto</option>
            <option value="lf">LF</option>
            <option value="crlf">CRLF</option>
          </select>
        </label>
        <button id="playground-format" type="button">Format</button>
        <button id="playground-copy" type="button">Copy</button>
        <span id="playground-status" class="playground-status" role="status">Loading formatter</span>
      </div>

      <div class="playground-grid">
        <div class="playground-editor">
          <label for="playground-input">Input</label>
          <textarea id="playground-input" spellcheck="false">select customer_id, count(*) as orders, sum(total_amount) as revenue
from analytics.orders
where order_status = 'paid' and created_at >= dateadd(day, -30, current_timestamp())
group by customer_id
qualify row_number() over (partition by customer_id order by revenue desc) = 1;</textarea>
        </div>

        <div class="playground-editor">
          <label for="playground-output">Output</label>
          <textarea id="playground-output" spellcheck="false" readonly></textarea>
        </div>
      </div>
    `;
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

  function validateApi(api) {
    if (!(api.memory instanceof WebAssembly.Memory)) {
      throw new Error("Formatter WASM does not export WebAssembly memory");
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
        throw new Error(`Formatter WASM is missing required export ${name}`);
      }
    }
  }

  function normalizeInteger(value, fallback) {
    const number = Number(value);
    return Number.isInteger(number) && number > 0 ? number : fallback;
  }

  if (typeof module !== "undefined" && module.exports) {
    module.exports = { callFormatter, enumCode, normalizeInteger, validateApi };
  }
})();
