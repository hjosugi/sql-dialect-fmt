"use strict";

// Canonical dialect names, in the same order as the Rust `Dialect::ALL` (the Wasm ABI index).
const DIALECTS = Object.freeze([
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
]);

// Common aliases accepted in settings, normalized to a canonical dialect name.
const DIALECT_ALIASES = Object.freeze({
  oracle: "plsql",
  "pl/sql": "plsql",
  tsql: "transactsql",
  mssql: "transactsql",
  sqlserver: "transactsql",
  postgres: "postgresql",
  pg: "postgresql",
  presto: "trino",
  couchbase: "n1ql",
  singlestore: "singlestoredb",
  memsql: "singlestoredb",
  maria: "mariadb",
  sqlite3: "sqlite",
  sparksql: "spark",
  bq: "bigquery",
  standard: "sql",
  ansi: "sql",
  generic: "sql",
});

const FORMATTER_DEFAULTS = Object.freeze({
  dialect: "snowflake",
  lineWidth: 80,
  indentWidth: 2,
  keywordCase: "upper",
  dataTypeCase: "preserve",
  functionCase: "preserve",
  identifierCase: "preserve",
  selectItemLayout: "vertical",
  commaStyle: "trailing",
  lineEnding: "auto",
  logicalOperatorNewline: "before",
  denseOperators: false,
  useTabs: false,
  newlineBeforeSemicolon: false,
  linesBetweenQueries: null,
  expressionWidth: null,
});

function readFormatterOptions(config, editorOptions = {}) {
  const configuredIndent = normalizeInteger(
    config.get("indentWidth", FORMATTER_DEFAULTS.indentWidth),
    FORMATTER_DEFAULTS.indentWidth,
  );
  const useEditorIndentation = config.get("useEditorIndentation", true) !== false;
  const editorIndent = normalizeInteger(editorOptions.tabSize, configuredIndent);

  return {
    dialect: readDialect(config),
    lineWidth: normalizeInteger(
      config.get("lineWidth", FORMATTER_DEFAULTS.lineWidth),
      FORMATTER_DEFAULTS.lineWidth,
    ),
    indentWidth: useEditorIndentation ? editorIndent : configuredIndent,
    keywordCase: readKeywordCase(config),
    dataTypeCase: enumValue(
      config.get("dataTypeCase"),
      ["upper", "lower", "preserve"],
      FORMATTER_DEFAULTS.dataTypeCase,
    ),
    functionCase: enumValue(
      config.get("functionCase"),
      ["upper", "lower", "preserve"],
      FORMATTER_DEFAULTS.functionCase,
    ),
    identifierCase: enumValue(
      config.get("identifierCase"),
      ["upper", "lower", "preserve"],
      FORMATTER_DEFAULTS.identifierCase,
    ),
    selectItemLayout: enumValue(
      config.get("selectItemLayout"),
      ["auto", "vertical"],
      FORMATTER_DEFAULTS.selectItemLayout,
    ),
    commaStyle: enumValue(
      config.get("commaStyle"),
      ["trailing", "leading"],
      FORMATTER_DEFAULTS.commaStyle,
    ),
    lineEnding: enumValue(
      config.get("lineEnding"),
      ["auto", "lf", "crlf"],
      FORMATTER_DEFAULTS.lineEnding,
    ),
    logicalOperatorNewline: enumValue(
      config.get("logicalOperatorNewline"),
      ["before", "after"],
      FORMATTER_DEFAULTS.logicalOperatorNewline,
    ),
    denseOperators: config.get("denseOperators", FORMATTER_DEFAULTS.denseOperators) === true,
    useTabs: config.get("useTabs", FORMATTER_DEFAULTS.useTabs) === true,
    newlineBeforeSemicolon:
      config.get("newlineBeforeSemicolon", FORMATTER_DEFAULTS.newlineBeforeSemicolon) === true,
    linesBetweenQueries: optionalNonNegativeInteger(config.get("linesBetweenQueries")),
    expressionWidth: optionalPositiveInteger(config.get("expressionWidth")),
  };
}

function readDialect(config) {
  const raw = String(config.get("dialect") ?? "").toLowerCase();
  if (DIALECTS.includes(raw)) {
    return raw;
  }
  return DIALECT_ALIASES[raw] ?? FORMATTER_DEFAULTS.dialect;
}

function readKeywordCase(config) {
  // Preserve the old boolean setting only when `keywordCase` has not been explicitly configured.
  // VS Code's inspect() distinguishes a schema default from a user/workspace/language override.
  const inspection = typeof config.inspect === "function" ? config.inspect("keywordCase") : null;
  const explicitlyConfigured = inspection
    ? [
        "globalValue",
        "workspaceValue",
        "workspaceFolderValue",
        "globalLanguageValue",
        "workspaceLanguageValue",
        "workspaceFolderLanguageValue",
      ].some((key) => inspection[key] !== undefined)
    : config.get("keywordCase") !== undefined;

  if (!explicitlyConfigured && config.get("uppercaseKeywords", true) === false) {
    return "preserve";
  }
  return enumValue(
    config.get("keywordCase"),
    ["upper", "lower", "preserve"],
    FORMATTER_DEFAULTS.keywordCase,
  );
}

function enumValue(value, allowed, fallback) {
  const normalized = String(value ?? "").toLowerCase();
  return allowed.includes(normalized) ? normalized : fallback;
}

function normalizeInteger(value, fallback) {
  const number = Number(value);
  return Number.isInteger(number) && number > 0 ? number : fallback;
}

function optionalPositiveInteger(value) {
  const number = Number(value);
  return Number.isInteger(number) && number > 0 ? number : null;
}

function optionalNonNegativeInteger(value) {
  const number = Number(value);
  return Number.isInteger(number) && number >= 0 ? number : null;
}

module.exports = {
  DIALECTS,
  FORMATTER_DEFAULTS,
  readFormatterOptions,
};
