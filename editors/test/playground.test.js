"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const playgroundPath = path.join(__dirname, "..", "..", "docs-site", "theme", "playground.js");
const { callFormatter, enumCode, normalizeInteger, optionalInteger, validateApi } = require(playgroundPath);

test("playground falls back to the v1 Wasm ABI when v2 is unavailable", () => {
  let args;
  const api = {
    sql_dialect_fmt_format_with_options(...values) {
      args = values;
      return 0;
    },
  };
  const status = callFormatter(api, 12, 34, {
    lineWidth: 80,
    indentWidth: 2,
    keywordCase: "lower",
    selectItemLayout: "vertical",
    commaStyle: "leading",
    lineEnding: "crlf",
    dialect: "databricks",
  });

  assert.equal(status, 0);
  assert.deepEqual(args, [12, 34, 80, 2, 1, 1, 1, 2, 1]);
});

test("playground sends the complete formatter option set through the Wasm v2 ABI", () => {
  let args;
  const api = {
    sql_dialect_fmt_format_with_options_v2(...values) {
      args = values;
      return 0;
    },
  };
  const status = callFormatter(api, 12, 34, {
    lineWidth: 80,
    indentWidth: 2,
    keywordCase: "lower",
    dataTypeCase: "upper",
    functionCase: "preserve",
    identifierCase: "lower",
    selectItemLayout: "vertical",
    commaStyle: "leading",
    lineEnding: "crlf",
    dialect: "databricks",
    logicalOperatorNewline: "after",
    denseOperators: true,
    useTabs: true,
    newlineBeforeSemicolon: true,
    linesBetweenQueries: 2,
    expressionWidth: undefined,
  });

  assert.equal(status, 0);
  assert.deepEqual(args, [
    12,
    34,
    80,
    2,
    1, // keywordCase lower
    1, // selectItemLayout vertical
    1, // commaStyle leading
    2, // lineEnding crlf
    1, // dialect databricks
    0, // dataTypeCase upper
    2, // functionCase preserve
    1, // identifierCase lower
    1, // logicalOperatorNewline after
    1 | 2 | 4, // denseOperators + useTabs + newlineBeforeSemicolon
    2, // linesBetweenQueries
    0xffffffff, // expressionWidth unset
  ]);
});

test("playground UI exposes every public style option with conventional defaults", () => {
  const source = fs.readFileSync(playgroundPath, "utf8");
  for (const control of [
    'id="playground-keyword-case"',
    'id="playground-data-type-case"',
    'id="playground-function-case"',
    'id="playground-identifier-case"',
    'id="playground-select-layout"',
    'id="playground-comma-style"',
    'id="playground-line-ending"',
    'id="playground-logical-operator-newline"',
    'id="playground-dense-operators"',
    'id="playground-use-tabs"',
    'id="playground-newline-before-semicolon"',
    'id="playground-lines-between-queries"',
    'id="playground-expression-width"',
    'id="playground-dialect"',
  ]) {
    assert.match(source, new RegExp(control));
  }
  assert.match(source, /id="playground-line-width"[^>]+value="80"/);
  assert.match(source, /id="playground-indent-width"[^>]+value="2"/);
  assert.equal(normalizeInteger("0", 80), 80);
  assert.equal(optionalInteger("0"), 0);
  assert.equal(optionalInteger(""), 0xffffffff);
  assert.equal(enumCode("oracle", ["snowflake", "databricks"]), 0);
});

test("playground rejects incomplete Wasm builds before formatting", () => {
  assert.throws(
    () => validateApi({ memory: new WebAssembly.Memory({ initial: 1 }) }),
    /missing required export sql_dialect_fmt_alloc/,
  );
});
