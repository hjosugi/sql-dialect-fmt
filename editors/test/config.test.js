"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");

const { FORMATTER_DEFAULTS, readFormatterOptions } = require("../src/config");

function config(values = {}) {
  return {
    get(key, fallback) {
      return Object.hasOwn(values, key) ? values[key] : fallback;
    },
    inspect(key) {
      return Object.hasOwn(values, key) ? { workspaceValue: values[key] } : {};
    },
  };
}

test("defaults are conventional and editor indentation wins", () => {
  assert.deepEqual(readFormatterOptions(config(), { tabSize: 4, insertSpaces: true }), {
    ...FORMATTER_DEFAULTS,
    indentWidth: 4,
  });
});

test("every public style setting is normalized", () => {
  assert.deepEqual(
    readFormatterOptions(
      config({
        dialect: "databricks",
        lineWidth: 120,
        indentWidth: 3,
        useEditorIndentation: false,
        keywordCase: "lower",
        dataTypeCase: "upper",
        functionCase: "lower",
        identifierCase: "upper",
        selectItemLayout: "auto",
        commaStyle: "leading",
        lineEnding: "crlf",
        logicalOperatorNewline: "after",
        denseOperators: true,
        useTabs: true,
        newlineBeforeSemicolon: true,
        linesBetweenQueries: 2,
        expressionWidth: 40,
      }),
      { tabSize: 8 },
    ),
    {
      dialect: "databricks",
      lineWidth: 120,
      indentWidth: 3,
      keywordCase: "lower",
      dataTypeCase: "upper",
      functionCase: "lower",
      identifierCase: "upper",
      selectItemLayout: "auto",
      commaStyle: "leading",
      lineEnding: "crlf",
      logicalOperatorNewline: "after",
      denseOperators: true,
      useTabs: true,
      newlineBeforeSemicolon: true,
      linesBetweenQueries: 2,
      expressionWidth: 40,
    },
  );
});

test("dialect aliases normalize to canonical names", () => {
  assert.equal(readFormatterOptions(config({ dialect: "oracle" })).dialect, "plsql");
  assert.equal(readFormatterOptions(config({ dialect: "tsql" })).dialect, "transactsql");
  assert.equal(readFormatterOptions(config({ dialect: "POSTGRES" })).dialect, "postgresql");
  assert.equal(readFormatterOptions(config({ dialect: "nonsense" })).dialect, "snowflake");
});

test("optional numeric settings accept zero or positive values and reject the rest", () => {
  const options = readFormatterOptions(config({ linesBetweenQueries: 0, expressionWidth: 40 }));
  assert.equal(options.linesBetweenQueries, 0);
  assert.equal(options.expressionWidth, 40);
  const unset = readFormatterOptions(config({ linesBetweenQueries: -1, expressionWidth: "x" }));
  assert.equal(unset.linesBetweenQueries, null);
  assert.equal(unset.expressionWidth, null);
});

test("legacy uppercase setting remains compatible until keywordCase is explicitly set", () => {
  assert.equal(readFormatterOptions(config({ uppercaseKeywords: false })).keywordCase, "preserve");
  assert.equal(
    readFormatterOptions(config({ uppercaseKeywords: false, keywordCase: "lower" })).keywordCase,
    "lower",
  );
});
