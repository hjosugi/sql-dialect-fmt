"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const path = require("node:path");
const test = require("node:test");

const EDITORS_DIR = path.resolve(__dirname, "..");

function harness() {
  const settings = { "lsp.enabled": false, "lsp.path": process.execPath };
  const documentProviders = [];
  const rangeProviders = [];
  const logs = [];
  const clients = [];
  let configurationListener;
  let failNextStart = false;

  class Disposable {
    constructor(dispose = () => {}) {
      this.dispose = dispose;
    }
  }

  function registration(list, provider) {
    list.push(provider);
    return new Disposable(() => {
      const index = list.indexOf(provider);
      if (index >= 0) {
        list.splice(index, 1);
      }
    });
  }

  class LanguageClient {
    constructor() {
      this.started = false;
      this.disposed = false;
      this.stopped = false;
      clients.push(this);
    }

    async start() {
      if (failNextStart) {
        failNextStart = false;
        throw new Error("synthetic startup failure");
      }
      this.started = true;
    }

    async dispose() {
      this.disposed = true;
    }

    async stop() {
      this.stopped = true;
    }
  }

  const vscode = {
    Disposable,
    workspace: {
      getConfiguration() {
        return {
          get(key, fallback) {
            return Object.hasOwn(settings, key) ? settings[key] : fallback;
          },
          inspect(key) {
            return Object.hasOwn(settings, key) ? { workspaceValue: settings[key] } : {};
          },
        };
      },
      onDidChangeConfiguration(listener) {
        configurationListener = listener;
        return new Disposable();
      },
    },
    window: {
      createOutputChannel() {
        return { appendLine: (message) => logs.push(message), dispose() {} };
      },
      showErrorMessage(message) {
        throw new Error(`unexpected VS Code error: ${message}`);
      },
    },
    commands: {
      registerCommand() {
        return new Disposable();
      },
      executeCommand() {},
    },
    languages: {
      registerDocumentFormattingEditProvider(_selector, provider) {
        return registration(documentProviders, provider);
      },
      registerDocumentRangeFormattingEditProvider(_selector, provider) {
        return registration(rangeProviders, provider);
      },
    },
  };

  const languageclient = {
    LanguageClient,
    RevealOutputChannelOn: { Never: 0 },
    ErrorAction: { Continue: 1 },
    CloseAction: { Restart: 1, DoNotRestart: 2 },
  };

  return {
    clients,
    documentProviders,
    failStart() {
      failNextStart = true;
    },
    languageclient,
    logs,
    rangeProviders,
    settings,
    triggerConfigurationChange() {
      assert.ok(configurationListener, "extension did not register a configuration listener");
      configurationListener({ affectsConfiguration: () => true });
    },
    vscode,
  };
}

async function waitFor(predicate, message) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (predicate()) {
      return;
    }
    await new Promise((resolve) => setImmediate(resolve));
  }
  assert.fail(message);
}

test("extension serializes Wasm/LSP transitions and falls back after startup failure", async () => {
  const mock = harness();
  const originalLoad = Module._load;
  Module._load = function load(request, parent, isMain) {
    if (request === "vscode") {
      return mock.vscode;
    }
    if (request === "vscode-languageclient/node") {
      return mock.languageclient;
    }
    return originalLoad.call(this, request, parent, isMain);
  };

  let extension;
  try {
    const extensionPath = path.join(EDITORS_DIR, "extension.js");
    delete require.cache[require.resolve(extensionPath)];
    extension = require(extensionPath);

    const context = {
      subscriptions: [],
      asAbsolutePath(relativePath) {
        return path.join(EDITORS_DIR, relativePath);
      },
    };
    extension.activate(context);
    await waitFor(
      () => mock.documentProviders.length === 1 && mock.rangeProviders.length === 1,
      "Wasm providers were not registered",
    );

    mock.settings["lsp.enabled"] = true;
    mock.triggerConfigurationChange();
    await waitFor(
      () => mock.clients.length === 1 && mock.clients[0].started,
      "language client did not start",
    );
    assert.equal(mock.documentProviders.length, 0);
    assert.equal(mock.rangeProviders.length, 0);

    mock.settings["lsp.enabled"] = false;
    mock.triggerConfigurationChange();
    await waitFor(
      () => mock.clients[0].disposed && mock.documentProviders.length === 1,
      "disabling LSP did not restore Wasm",
    );
    assert.equal(mock.rangeProviders.length, 1);

    mock.failStart();
    mock.settings["lsp.enabled"] = true;
    mock.triggerConfigurationChange();
    await waitFor(
      () => mock.clients.length === 2 && mock.clients[1].disposed,
      "failed language client was not disposed",
    );
    assert.equal(mock.documentProviders.length, 1, "Wasm fallback disappeared after LSP failure");
    assert.equal(mock.rangeProviders.length, 1);
    assert.ok(mock.logs.some((line) => line.includes("synthetic startup failure")));

    mock.settings["lsp.enabled"] = false;
    await extension.deactivate();
    for (const subscription of context.subscriptions) {
      subscription.dispose();
    }
  } finally {
    Module._load = originalLoad;
  }
});
