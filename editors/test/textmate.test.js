"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const oniguruma = require("vscode-oniguruma");
const textmate = require("vscode-textmate");

const EDITORS_DIR = path.resolve(__dirname, "..");

const javascriptGrammar = {
  scopeName: "source.js",
  patterns: [
    {
      name: "comment.line.double-slash.js",
      match: "//.*$",
    },
    {
      name: "string.quoted.double.js",
      begin: '"',
      end: '"',
      patterns: [{ name: "constant.character.escape.js", match: "\\\\." }],
    },
    {
      name: "string.quoted.template.js",
      begin: "`",
      end: "`",
      patterns: [{ name: "constant.character.escape.js", match: "\\\\." }],
    },
    {
      name: "storage.type.js",
      match: "\\b(var|let|const)\\b",
    },
    {
      name: "keyword.control.js",
      match: "\\b(if|else|while|return|throw)\\b",
    },
    {
      name: "constant.numeric.js",
      match: "\\b\\d+(?:\\.\\d+)?\\b",
    },
    {
      name: "variable.other.readwrite.js",
      match: "\\b[A-Za-z_$][A-Za-z0-9_$]*\\b",
    },
  ],
};

let loadOniguruma;

function ensureOniguruma() {
  if (!loadOniguruma) {
    const wasm = fs.readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm"));
    loadOniguruma = oniguruma.loadWASM(
      wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength),
    );
  }
  return loadOniguruma;
}

async function loadSnowflakeGrammar() {
  await ensureOniguruma();
  const snowflakePath = path.join(EDITORS_DIR, "snowflake.tmLanguage.json");
  const snowflakeGrammar = textmate.parseRawGrammar(
    fs.readFileSync(snowflakePath, "utf8"),
    snowflakePath,
  );
  const registry = new textmate.Registry({
    onigLib: Promise.resolve({
      createOnigScanner(patterns) {
        return new oniguruma.OnigScanner(patterns);
      },
      createOnigString(source) {
        return new oniguruma.OnigString(source);
      },
    }),
    loadGrammar(scopeName) {
      if (scopeName === "source.snowflake-sql") {
        return snowflakeGrammar;
      }
      if (scopeName === "source.js") {
        return javascriptGrammar;
      }
      if (["source.python", "source.java", "source.scala"].includes(scopeName)) {
        // Stand-ins for grammars other VS Code extensions contribute. Without a
        // resolvable grammar vscode-textmate drops the embedding rule entirely,
        // which is not how VS Code behaves when the language is installed.
        return { scopeName, patterns: [] };
      }
      return null;
    },
  });
  return registry.loadGrammar("source.snowflake-sql");
}

function tokenize(grammar, source) {
  let ruleStack = textmate.INITIAL;
  const tokens = [];
  for (const [lineNumber, line] of source.split("\n").entries()) {
    const result = grammar.tokenizeLine(line, ruleStack);
    ruleStack = result.ruleStack;
    for (const token of result.tokens) {
      tokens.push({
        line: lineNumber + 1,
        text: line.slice(token.startIndex, token.endIndex),
        scopes: token.scopes,
      });
    }
  }
  return tokens;
}

test("JavaScript routine body is tokenized with source.js scopes", async () => {
  const grammar = await loadSnowflakeGrammar();
  const templatePath = path.resolve(
    EDITORS_DIR,
    "../crates/sql-dialect-fmt-test-fixtures/fixtures/regressions/",
    "javascript_routine_trailing_whitespace/input.sql",
  );
  const source = fs
    .readFileSync(templatePath, "utf8")
    .replace("__SQL_DIALECT_FMT_TRAILING_WHITESPACE__", "   \n\t\n   ");
  const tokens = tokenize(grammar, source);

  const jsVar = tokens.find((token) => token.text === "var");
  assert.ok(jsVar, "expected a JavaScript var token");
  assert.ok(jsVar.scopes.includes("source.js"), jsVar.scopes);
  assert.ok(jsVar.scopes.includes("storage.type.js"), jsVar.scopes);
  assert.ok(
    jsVar.scopes.includes("meta.embedded.block.javascript.snowflake"),
    jsVar.scopes,
  );
  assert.ok(!jsVar.scopes.includes("string.quoted.dollar.sql"), jsVar.scopes);

  const jsThrow = tokens.find((token) => token.text === "throw");
  assert.ok(jsThrow?.scopes.includes("keyword.control.js"), jsThrow?.scopes);
  const jsString = tokens.find((token) => token.text.includes("P_YEAR must be"));
  assert.ok(jsString?.scopes.includes("string.quoted.double.js"), jsString?.scopes);

  const sqlUse = tokens.find((token) => token.text.toUpperCase() === "USE");
  assert.ok(sqlUse?.scopes.includes("keyword.other.sql"), sqlUse?.scopes);
});

test("JavaScript injection survives a multiline routine header and delimiter", async () => {
  const grammar = await loadSnowflakeGrammar();
  const source = [
    "CREATE PROCEDURE p()",
    "RETURNS VARIANT",
    "LANGUAGE JAVASCRIPT",
    "EXECUTE AS CALLER",
    "AS",
    "$$",
    "const value={answer:42};",
    "return value;",
    "$$;",
    "SELECT 1;",
  ].join("\n");
  const tokens = tokenize(grammar, source);

  const jsConst = tokens.find((token) => token.text === "const");
  assert.ok(jsConst?.scopes.includes("source.js"), jsConst?.scopes);
  assert.ok(jsConst?.scopes.includes("storage.type.js"), jsConst?.scopes);
  const returnToken = tokens.find((token) => token.text === "return");
  assert.ok(returnToken?.scopes.includes("keyword.control.js"), returnToken?.scopes);
  const trailingSelect = tokens.find(
    (token) => token.line === 10 && token.text === "SELECT",
  );
  assert.ok(
    trailingSelect?.scopes.includes("keyword.other.sql"),
    trailingSelect?.scopes,
  );
  assert.ok(!trailingSelect?.scopes.includes("source.js"), trailingSelect?.scopes);
});

test("LANGUAGE SQL routine bodies are highlighted as SQL, not opaque strings", async () => {
  const grammar = await loadSnowflakeGrammar();
  const source = [
    "CREATE OR REPLACE PROCEDURE DJANGO_TEST.PUBLIC.PLUS (",
    "    obj OBJECT",
    ") RETURNS TABLE (total NUMBER(38, 0)) LANGUAGE SQL EXECUTE AS OWNER AS $$",
    "DECLARE",
    "    res RESULTSET;",
    "BEGIN",
    "    res := (EXECUTE IMMEDIATE 'select ' || (obj['a'] + obj['b']) || ' as a');",
    "    RETURN TABLE(res);",
    "END;",
    "$$;",
  ].join("\n");
  const tokens = tokenize(grammar, source);

  // The $$ body is SQL, not one big dollar-quoted string.
  const declare = tokens.find((token) => token.text === "DECLARE");
  assert.ok(declare?.scopes.includes("keyword.control.sql"), declare?.scopes);
  assert.ok(!declare?.scopes.includes("string.quoted.dollar.sql"), declare?.scopes);
  assert.ok(
    declare?.scopes.includes("meta.embedded.block.sql.snowflake"),
    declare?.scopes,
  );
  const begin = tokens.find((token) => token.text === "BEGIN");
  assert.ok(begin?.scopes.includes("keyword.control.sql"), begin?.scopes);

  // Snowflake Scripting declaration types and inner strings keep distinct scopes.
  const resultset = tokens.find((token) => token.text === "RESULTSET");
  assert.ok(resultset?.scopes.includes("support.type.sql"), resultset?.scopes);
  const innerString = tokens.find((token) => token.text.includes("select "));
  assert.ok(
    innerString?.scopes.includes("string.quoted.single.sql"),
    innerString?.scopes,
  );

  // The created object name pops as an entity name.
  const procName = tokens.find((token) => token.text === "DJANGO_TEST.PUBLIC.PLUS");
  assert.ok(procName?.scopes.includes("entity.name.function.sql"), procName?.scopes);

  // Types in the header stay types.
  const objectType = tokens.find((token) => token.text === "OBJECT");
  assert.ok(objectType?.scopes.includes("support.type.sql"), objectType?.scopes);
  const numberType = tokens.find((token) => token.text === "NUMBER");
  assert.ok(numberType?.scopes.includes("support.type.sql"), numberType?.scopes);
});

test("built-in functions, constants, and scripting variables get their own scopes", async () => {
  const grammar = await loadSnowflakeGrammar();
  const source = [
    "SELECT COALESCE(a, IFF(b, 1, 2)) AS c, LEFT(name, 3), my_udf(x), CURRENT_TIMESTAMP",
    "FROM t LEFT JOIN u ON t.id = u.id WHERE flag = TRUE AND other IS NOT NULL;",
    "BEGIN RETURN SQLROWCOUNT; EXCEPTION WHEN STATEMENT_ERROR THEN RAISE; END;",
  ].join("\n");
  const tokens = tokenize(grammar, source);

  const coalesce = tokens.find((token) => token.text === "COALESCE");
  assert.ok(coalesce?.scopes.includes("support.function.builtin.sql"), coalesce?.scopes);
  const iff = tokens.find((token) => token.text === "IFF");
  assert.ok(iff?.scopes.includes("support.function.builtin.sql"), iff?.scopes);

  // LEFT( is the string function; LEFT JOIN keeps the keyword scope.
  const lefts = tokens.filter((token) => token.text === "LEFT");
  assert.equal(lefts.length, 2, JSON.stringify(lefts));
  assert.ok(lefts[0].scopes.includes("support.function.builtin.sql"), lefts[0].scopes);
  assert.ok(lefts[1].scopes.includes("keyword.other.sql"), lefts[1].scopes);

  // Unknown callables still read as function calls; context functions need no parens.
  const udf = tokens.find((token) => token.text === "my_udf");
  assert.ok(udf?.scopes.includes("entity.name.function.sql"), udf?.scopes);
  const now = tokens.find((token) => token.text === "CURRENT_TIMESTAMP");
  assert.ok(now?.scopes.includes("support.function.builtin.sql"), now?.scopes);

  // TRUE/NULL are constants, not generic keywords.
  const trueTok = tokens.find((token) => token.text === "TRUE");
  assert.ok(trueTok?.scopes.includes("constant.language.sql"), trueTok?.scopes);
  const nullTok = tokens.find((token) => token.text === "NULL");
  assert.ok(nullTok?.scopes.includes("constant.language.sql"), nullTok?.scopes);

  // Snowflake Scripting status variables and built-in exceptions.
  const rowcount = tokens.find((token) => token.text === "SQLROWCOUNT");
  assert.ok(rowcount?.scopes.includes("variable.language.sql"), rowcount?.scopes);
  const stmtError = tokens.find((token) => token.text === "STATEMENT_ERROR");
  assert.ok(stmtError?.scopes.includes("support.type.exception.sql"), stmtError?.scopes);
});

test("PYTHON routine bodies are fenced off as embedded python", async () => {
  const grammar = await loadSnowflakeGrammar();
  const source = [
    "CREATE PROCEDURE py()",
    "RETURNS VARIANT",
    "LANGUAGE PYTHON",
    "RUNTIME_VERSION = '3.11'",
    "HANDLER = 'run'",
    "AS $$",
    "def run(session):",
    "    return None",
    "$$;",
    "SELECT 1;",
  ].join("\n");
  const tokens = tokenize(grammar, source);

  const def = tokens.find((token) => token.text.includes("def run"));
  assert.ok(
    def?.scopes.includes("meta.embedded.block.python.snowflake"),
    def?.scopes,
  );
  assert.ok(!def?.scopes.includes("keyword.other.sql"), def?.scopes);
  const trailingSelect = tokens.find(
    (token) => token.line === 10 && token.text === "SELECT",
  );
  assert.ok(trailingSelect?.scopes.includes("keyword.other.sql"), trailingSelect?.scopes);
});

test("template-literal ${...} placeholders are scoped as template expressions", async () => {
  const grammar = await loadSnowflakeGrammar();
  const source = "SELECT ${cfg.col} FROM ${cfg.t} WHERE id = ${id};";
  const tokens = tokenize(grammar, source);

  // The opening `${` and the interpolated body carry the template-expression scopes.
  const open = tokens.find((token) => token.text === "${");
  assert.ok(
    open?.scopes.includes("punctuation.definition.template-expression.begin.sql"),
    open?.scopes,
  );
  const body = tokens.find((token) => token.text.includes("cfg.col"));
  assert.ok(body?.scopes.includes("meta.template-expression.sql"), body?.scopes);
  assert.ok(body?.scopes.includes("variable.parameter.template.sql"), body?.scopes);

  // The surrounding SQL keywords keep their ordinary SQL scopes.
  const select = tokens.find((token) => token.text === "SELECT");
  assert.ok(select?.scopes.includes("keyword.other.sql"), select?.scopes);
  const from = tokens.find((token) => token.text === "FROM");
  assert.ok(from?.scopes.includes("keyword.other.sql"), from?.scopes);
});

test("nested braces and strings inside ${...} stay one template expression", async () => {
  const grammar = await loadSnowflakeGrammar();
  // Object literal with a nested array, plus a `}` hidden inside a string: none of these must
  // close the placeholder early, so the trailing SQL keeps its SQL scopes.
  const source = "SELECT ${ fn({a: 1, b: [2, 3]}, '}') } AS c FROM t;";
  const tokens = tokenize(grammar, source);

  const asKeyword = tokens.find((token) => token.text === "AS");
  assert.ok(asKeyword, "expected the trailing AS keyword to be tokenized");
  assert.ok(asKeyword.scopes.includes("keyword.other.sql"), asKeyword.scopes);
  assert.ok(
    !asKeyword.scopes.includes("meta.template-expression.sql"),
    "AS must be outside the placeholder: " + asKeyword.scopes,
  );

  // The `}` inside the single-quoted string is string content, not the placeholder terminator.
  const stringBrace = tokens.find(
    (token) => token.text === "}" && token.scopes.includes("string.quoted.single.sql"),
  );
  assert.ok(stringBrace, "expected a '}' scoped as string content inside the placeholder");
});
