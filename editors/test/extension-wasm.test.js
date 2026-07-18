"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const test = require("node:test");

const EDITORS_DIR = path.resolve(__dirname, "..");

function vscodeMock() {
  const documentProviders = [];
  const rangeProviders = [];

  class Disposable {
    constructor(dispose = () => {}) {
      this.dispose = dispose;
    }
  }

  class Range {
    constructor(start, end) {
      this.start = start;
      this.end = end;
    }
  }

  return {
    api: {
      Disposable,
      Range,
      TextEdit: {
        replace(range, newText) {
          return { range, newText };
        },
      },
      window: {
        createOutputChannel() {
          return { appendLine() {}, dispose() {} };
        },
        showErrorMessage(message) {
          throw new Error(`unexpected VS Code error: ${message}`);
        },
      },
      workspace: {
        getConfiguration() {
          return {
            get(_key, fallback) {
              return fallback;
            },
          };
        },
        onDidChangeConfiguration() {
          return new Disposable();
        },
      },
      languages: {
        registerDocumentFormattingEditProvider(_selector, provider) {
          documentProviders.push(provider);
          return new Disposable();
        },
        registerDocumentRangeFormattingEditProvider(_selector, provider) {
          rangeProviders.push(provider);
          return new Disposable();
        },
      },
    },
    documentProviders,
    rangeProviders,
  };
}

async function waitFor(predicate) {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (predicate()) {
      return;
    }
    await new Promise((resolve) => setImmediate(resolve));
  }
  assert.fail("extension activation did not register its formatter");
}

test("bundled VS Code provider formats the realistic regression through packaged WASM", async () => {
  const mock = vscodeMock();
  const originalLoad = Module._load;
  Module._load = function load(request, parent, isMain) {
    if (request === "vscode") {
      return mock.api;
    }
    return originalLoad.call(this, request, parent, isMain);
  };

  let extension;
  try {
    const bundlePath = path.join(EDITORS_DIR, "dist", "extension.js");
    delete require.cache[require.resolve(bundlePath)];
    extension = require(bundlePath);
  } finally {
    Module._load = originalLoad;
  }

  const context = {
    subscriptions: [],
    asAbsolutePath(relativePath) {
      return path.join(EDITORS_DIR, relativePath);
    },
  };
  extension.activate(context);
  await waitFor(() => mock.documentProviders.length === 1);
  assert.equal(mock.rangeProviders.length, 1);

  const fixtureDir = path.resolve(
    EDITORS_DIR,
    "../crates/sql-dialect-fmt-test-fixtures/fixtures/regressions/",
    "javascript_routine_trailing_whitespace",
  );
  const input = fs
    .readFileSync(path.join(fixtureDir, "input.sql"), "utf8")
    .replace("__SQL_DIALECT_FMT_TRAILING_WHITESPACE__", "   \n\t\n   ");
  const expected = fs.readFileSync(path.join(fixtureDir, "expected.sql"), "utf8");
  const document = {
    getText() {
      return input;
    },
    positionAt(offset) {
      return { offset };
    },
  };

  const edits = await mock.documentProviders[0].provideDocumentFormattingEdits(document);
  assert.equal(edits.length, 1, "formatter should replace the broken input");
  assert.equal(edits[0].newText, expected);
  assert.equal(edits[0].range.start.offset, 0);
  assert.equal(edits[0].range.end.offset, input.length);

  await extension.deactivate();
  for (const subscription of context.subscriptions) {
    subscription.dispose?.();
  }
});
