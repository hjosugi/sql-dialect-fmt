"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const pkg = require("../package.json");

test("every dialect grammar has a language and every language is activated", () => {
  const languageIds = pkg.contributes.languages.map((language) => language.id);
  const languages = new Set(languageIds);

  for (const grammar of pkg.contributes.grammars) {
    assert.ok(
      languages.has(grammar.language),
      `grammar language ${grammar.language} has no language contribution`,
    );
    const file = path.join(__dirname, "..", grammar.path.replace(/^\.\//, ""));
    assert.ok(fs.existsSync(file), `missing grammar file ${grammar.path}`);
  }

  const events = new Set(pkg.activationEvents);
  for (const id of languageIds) {
    assert.ok(events.has(`onLanguage:${id}`), `missing activation event for ${id}`);
  }

  for (const file of pkg.files) {
    if (file.endsWith(".tmLanguage.json")) {
      assert.ok(fs.existsSync(path.join(__dirname, "..", file)), `files[] lists missing ${file}`);
    }
  }
});

test("the dialect-neutral grammar parses and is shared by the dialect languages", () => {
  const neutral = JSON.parse(
    fs.readFileSync(path.join(__dirname, "..", "sql.tmLanguage.json"), "utf8"),
  );
  assert.equal(neutral.scopeName, "source.sql.dialect-fmt");
  const shared = pkg.contributes.grammars.filter(
    (grammar) => grammar.path === "./sql.tmLanguage.json",
  );
  assert.ok(shared.length >= 20, "expected the dialect languages to share the neutral grammar");
});
