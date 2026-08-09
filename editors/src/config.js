"use strict";

const FORMATTER_DEFAULTS = Object.freeze({
  dialect: "snowflake",
  lineWidth: 80,
  indentWidth: 2,
  keywordCase: "upper",
  selectItemLayout: "vertical",
  commaStyle: "trailing",
  lineEnding: "auto",
});

function readFormatterOptions(config, editorOptions = {}) {
  const configuredIndent = normalizeInteger(
    config.get("indentWidth", FORMATTER_DEFAULTS.indentWidth),
    FORMATTER_DEFAULTS.indentWidth,
  );
  const useEditorIndentation = config.get("useEditorIndentation", true) !== false;
  const editorIndent = normalizeInteger(editorOptions.tabSize, configuredIndent);

  return {
    dialect: enumValue(config.get("dialect"), ["snowflake", "databricks"], "snowflake"),
    lineWidth: normalizeInteger(
      config.get("lineWidth", FORMATTER_DEFAULTS.lineWidth),
      FORMATTER_DEFAULTS.lineWidth,
    ),
    indentWidth: useEditorIndentation ? editorIndent : configuredIndent,
    keywordCase: readKeywordCase(config),
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
  };
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

module.exports = { FORMATTER_DEFAULTS, readFormatterOptions };
