(() => {
  // Canonical dialect names, in the same order as the Rust `Dialect::ALL` (the Wasm ABI index).
  const DIALECTS = [
    "snowflake",
    "databricks",
    "spark",
    "bigquery",
    "clickhouse",
    "db2",
    "db2i",
    "duckdb",
    "hive",
    "mariadb",
    "mysql",
    "tidb",
    "n1ql",
    "plsql",
    "postgresql",
    "redshift",
    "singlestoredb",
    "sqlite",
    "sql",
    "transactsql",
    "trino",
  ];

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
    const dataTypeCase = app.querySelector("#playground-data-type-case");
    const functionCase = app.querySelector("#playground-function-case");
    const identifierCase = app.querySelector("#playground-identifier-case");
    const selectItemLayout = app.querySelector("#playground-select-layout");
    const commaStyle = app.querySelector("#playground-comma-style");
    const lineEnding = app.querySelector("#playground-line-ending");
    const logicalOperatorNewline = app.querySelector("#playground-logical-operator-newline");
    const denseOperators = app.querySelector("#playground-dense-operators");
    const useTabs = app.querySelector("#playground-use-tabs");
    const newlineBeforeSemicolon = app.querySelector("#playground-newline-before-semicolon");
    const linesBetweenQueries = app.querySelector("#playground-lines-between-queries");
    const expressionWidth = app.querySelector("#playground-expression-width");
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
          dataTypeCase: dataTypeCase.value,
          functionCase: functionCase.value,
          identifierCase: identifierCase.value,
          selectItemLayout: selectItemLayout.value,
          commaStyle: commaStyle.value,
          lineEnding: lineEnding.value,
          logicalOperatorNewline: logicalOperatorNewline.value,
          denseOperators: denseOperators.checked,
          useTabs: useTabs.checked,
          newlineBeforeSemicolon: newlineBeforeSemicolon.checked,
          linesBetweenQueries: optionalInteger(linesBetweenQueries.value),
          expressionWidth: optionalInteger(expressionWidth.value),
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
    const dialectOptions = DIALECTS.map(
      (name) => `<option value="${name}">${DIALECT_LABELS[name] ?? name}</option>`,
    ).join("\n            ");
    app.innerHTML = `
      <div class="playground-toolbar" aria-label="Formatter options">
        <label>
          <span>Dialect</span>
          <select id="playground-dialect">
            ${dialectOptions}
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
          <span>Data type case</span>
          <select id="playground-data-type-case">
            <option value="preserve">Preserve</option>
            <option value="upper">Upper</option>
            <option value="lower">Lower</option>
          </select>
        </label>
        <label>
          <span>Function case</span>
          <select id="playground-function-case">
            <option value="preserve">Preserve</option>
            <option value="upper">Upper</option>
            <option value="lower">Lower</option>
          </select>
        </label>
        <label>
          <span>Identifier case</span>
          <select id="playground-identifier-case">
            <option value="preserve">Preserve</option>
            <option value="upper">Upper</option>
            <option value="lower">Lower</option>
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
        <label>
          <span>AND/OR</span>
          <select id="playground-logical-operator-newline">
            <option value="before">Before</option>
            <option value="after">After</option>
          </select>
        </label>
        <label>
          <span>Dense operators</span>
          <input id="playground-dense-operators" type="checkbox">
        </label>
        <label>
          <span>Use tabs</span>
          <input id="playground-use-tabs" type="checkbox">
        </label>
        <label>
          <span>Newline before ;</span>
          <input id="playground-newline-before-semicolon" type="checkbox">
        </label>
        <label>
          <span>Blank lines between queries</span>
          <input id="playground-lines-between-queries" type="number" min="0" max="9">
        </label>
        <label>
          <span>Expression width</span>
          <input id="playground-expression-width" type="number" min="1" max="240">
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

  // Human labels for the dialect <option>s (fall back to the canonical id).
  const DIALECT_LABELS = {
    snowflake: "Snowflake",
    databricks: "Databricks",
    spark: "Spark",
    bigquery: "BigQuery",
    clickhouse: "ClickHouse",
    db2: "DB2",
    db2i: "DB2 for i",
    duckdb: "DuckDB",
    hive: "Hive",
    mariadb: "MariaDB",
    mysql: "MySQL",
    tidb: "TiDB",
    n1ql: "N1QL",
    plsql: "Oracle PL/SQL",
    postgresql: "PostgreSQL",
    redshift: "Redshift",
    singlestoredb: "SingleStoreDB",
    sqlite: "SQLite",
    sql: "SQL (standard)",
    transactsql: "SQL Server (T-SQL)",
    trino: "Trino",
  };

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
    if (typeof api.sql_dialect_fmt_format_with_options_v2 === "function") {
      return api.sql_dialect_fmt_format_with_options_v2(
        inputPtr,
        inputLength,
        options.lineWidth,
        options.indentWidth,
        enumCode(options.keywordCase, ["upper", "lower", "preserve"]),
        enumCode(options.selectItemLayout, ["auto", "vertical"]),
        enumCode(options.commaStyle, ["trailing", "leading"]),
        enumCode(options.lineEnding, ["auto", "lf", "crlf"]),
        enumCode(options.dialect, DIALECTS),
        enumCode(options.dataTypeCase, ["upper", "lower", "preserve"]),
        enumCode(options.functionCase, ["upper", "lower", "preserve"]),
        enumCode(options.identifierCase, ["upper", "lower", "preserve"]),
        enumCode(options.logicalOperatorNewline, ["before", "after"]),
        booleanFlags(options),
        optionalInteger(options.linesBetweenQueries),
        optionalInteger(options.expressionWidth),
      );
    }
    return api.sql_dialect_fmt_format_with_options(
      inputPtr,
      inputLength,
      options.lineWidth,
      options.indentWidth,
      enumCode(options.keywordCase, ["upper", "lower", "preserve"]),
      enumCode(options.selectItemLayout, ["auto", "vertical"]),
      enumCode(options.commaStyle, ["trailing", "leading"]),
      enumCode(options.lineEnding, ["auto", "lf", "crlf"]),
      enumCode(options.dialect, DIALECTS),
    );
  }

  function booleanFlags(options) {
    return (
      (options.denseOperators ? 1 : 0) |
      (options.useTabs ? 2 : 0) |
      (options.newlineBeforeSemicolon ? 4 : 0)
    );
  }

  function optionalInteger(value) {
    if (value === undefined || value === null || String(value).trim() === "") {
      return 0xffffffff;
    }
    const number = Number(value);
    return Number.isInteger(number) && number >= 0 ? number : 0xffffffff;
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
    module.exports = {
      DIALECTS,
      callFormatter,
      enumCode,
      normalizeInteger,
      optionalInteger,
      validateApi,
    };
  }
})();
