"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const playgroundPath = path.join(__dirname, "..", "..", "docs-site", "theme", "playground.js");
const { callFormatter, normalizeInteger, validateApi } = require(playgroundPath);

test("playground sends the complete formatter option set through the stable Wasm ABI", () => {
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

test("playground UI exposes every public style option with conventional defaults", () => {
  const source = fs.readFileSync(playgroundPath, "utf8");
  for (const control of [
    'id="playground-keyword-case"',
    'id="playground-select-layout"',
    'id="playground-comma-style"',
    'id="playground-line-ending"',
  ]) {
    assert.match(source, new RegExp(control));
  }
  assert.match(source, /id="playground-line-width"[^>]+value="80"/);
  assert.match(source, /id="playground-indent-width"[^>]+value="2"/);
  assert.equal(normalizeInteger("0", 80), 80);
});

test("playground rejects incomplete Wasm builds before formatting", () => {
  assert.throws(
    () => validateApi({ memory: new WebAssembly.Memory({ initial: 1 }) }),
    /missing required export sql_dialect_fmt_alloc/,
  );
});
