"use strict";

// vscode-languageclient still calls `Minimatch#makeRe()`, which the patched minimatch 11 line
// removed; src/minimatch-compat.js restores it. These tests run the client's own glob code (loaded
// through its `$test` export, with a stubbed `vscode`) down every path that reaches minimatch, so
// they fail when the override is installed without a working shim.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const test = require("node:test");

const CLIENT_COMMON = "vscode-languageclient/$test/common";
const SQL_FILTERS = [{ scheme: "file", pattern: { glob: "**/*.sql" } }];
const EMPTY_FILTERS = [{ scheme: "file", pattern: { glob: "" } }];

// The client subscribes with `event(listener, thisArg)`; keep each feature's listener to fire it.
const createFilesListeners = new Map();
const vscode = {
  FileType: { File: 1, Directory: 2 },
  workspace: {
    onDidCreateFiles(listener, feature) {
      createFilesListeners.set(feature, listener);
      return { dispose() {} };
    },
  },
};

function loadClient() {
  const originalLoad = Module._load;
  Module._load = function load(request, parent, isMain) {
    if (request === "vscode") {
      return vscode;
    }
    return originalLoad.call(this, request, parent, isMain);
  };
  try {
    require("../src/minimatch-compat");
    return {
      ...require(`${CLIENT_COMMON}/utils/globPattern`),
      ...require(`${CLIENT_COMMON}/fileOperations`),
    };
  } finally {
    Module._load = originalLoad;
  }
}

const { DidCreateFilesFeature, matchGlobPattern } = loadClient();

function file(fsPath) {
  return { scheme: "file", fsPath };
}

/** A real `workspace/didCreateFiles` feature wired to a recording client. */
function createFilesFeature() {
  const sent = [];
  const warnings = [];
  const feature = new DidCreateFilesFeature({
    code2ProtocolConverter: {
      asDidCreateFilesParams: (event) => event.files.map((uri) => uri.fsPath),
    },
    middleware: {},
    sendNotification: async (_type, params) => {
      sent.push(params);
    },
    warn: (message) => warnings.push(message),
    info() {},
  });
  return {
    feature,
    sent,
    warnings,
    createFiles(...paths) {
      return createFilesListeners.get(feature).call(feature, { files: paths.map(file) });
    },
  };
}

test("glob filters for pull diagnostics and notebooks match through minimatch", () => {
  const resource = file("/workspace/models/orders.sql");
  assert.equal(matchGlobPattern("**/*.sql", resource), true);
  assert.equal(matchGlobPattern("**/*.py", resource), false);
  // A pattern that can match nothing is rejected, exactly as with minimatch 10.
  assert.equal(matchGlobPattern("", resource), false);
  assert.equal(matchGlobPattern("#models/*.sql", resource), false);
});

test("dynamically registered file-operation filters select the matching files", async () => {
  const { feature, sent, createFiles } = createFilesFeature();
  feature.register({ id: "dynamic", registerOptions: { filters: SQL_FILTERS } });
  await createFiles("/workspace/models/orders.sql", "/workspace/README.md");
  assert.deepEqual(sent, [["/workspace/models/orders.sql"]]);

  assert.throws(
    () => feature.register({ id: "empty", registerOptions: { filters: EMPTY_FILTERS } }),
    /Invalid pattern/,
  );
});

test("static workspace.fileOperations filters select the matching files", async () => {
  const { feature, sent, warnings, createFiles } = createFilesFeature();
  feature.initialize({ workspace: { fileOperations: { didCreate: { filters: SQL_FILTERS } } } });
  assert.deepEqual(warnings, []);
  await createFiles("/workspace/models/orders.sql", "/workspace/README.md");
  assert.deepEqual(sent, [["/workspace/models/orders.sql"]]);

  const rejected = createFilesFeature();
  rejected.feature.initialize({
    workspace: { fileOperations: { didCreate: { filters: EMPTY_FILTERS } } },
  });
  assert.equal(rejected.feature.filterSize(), 0);
  assert.equal(rejected.warnings.length, 1);
  assert.match(rejected.warnings[0], /Invalid pattern/);
});

test("the client only uses makeRe() as the validity check the shim answers", () => {
  // The shim returns a boolean, not a RegExp. If a client release starts using the result in any
  // other way, this fails and src/minimatch-compat.js has to be revisited.
  const lib = path.resolve(path.dirname(require.resolve(`${CLIENT_COMMON}/fileOperations`)), "..");
  for (const entry of fs.readdirSync(lib, { recursive: true })) {
    if (!entry.endsWith(".js")) {
      continue;
    }
    for (const line of fs.readFileSync(path.join(lib, entry), "utf8").split("\n")) {
      assert.ok(
        !line.replace(/!\w+\.makeRe\(\)/g, "").includes("makeRe"),
        `${entry} uses makeRe() beyond a validity check: ${line.trim()}`,
      );
    }
  }
});
