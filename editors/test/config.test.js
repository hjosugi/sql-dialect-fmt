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
        selectItemLayout: "auto",
        commaStyle: "leading",
        lineEnding: "crlf",
      }),
      { tabSize: 8 },
    ),
    {
      dialect: "databricks",
      lineWidth: 120,
      indentWidth: 3,
      keywordCase: "lower",
      selectItemLayout: "auto",
      commaStyle: "leading",
      lineEnding: "crlf",
    },
  );
});

test("legacy uppercase setting remains compatible until keywordCase is explicitly set", () => {
  assert.equal(readFormatterOptions(config({ uppercaseKeywords: false })).keywordCase, "preserve");
  assert.equal(
    readFormatterOptions(config({ uppercaseKeywords: false, keywordCase: "lower" })).keywordCase,
    "lower",
  );
});
