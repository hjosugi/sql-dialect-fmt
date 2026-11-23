"use strict";

// Compatibility shim between vscode-languageclient and the patched minimatch 11 line.
//
// Why it exists
//   minimatch 10 is affected by GHSA-g3hv-75wg-m249 and the fix exists only in the 11 line.
//   vscode-languageclient (10.1.2 through 11.0.1) still declares `minimatch: ^10.2.6`, so
//   `overrides.minimatch` in package.json forces the patched line for the bundled client.
//   minimatch 11 matches with `Matcher` objects instead of regular expressions and removed
//   `Minimatch#makeRe()`, which the client still calls in `lib/common/utils/globPattern.js`
//   (pull-diagnostics and notebook filters) and `lib/common/fileOperations.js` (file-operation
//   filters). Without this shim those paths throw `TypeError: matcher.makeRe is not a function`.
//
// What it covers
//   The client never uses the returned RegExp: both call sites only test `!matcher.makeRe()` to
//   reject a pattern that can match nothing. minimatch 10 returns `false` there when
//   `Minimatch#set` is empty, as it is for an empty pattern or a `#` comment. (Its only other
//   `false`, a RegExp that fails to compile, is marked impossible upstream and cannot occur
//   without regular expressions.) `set` is still a documented public property in minimatch 11 and
//   is filled by the same rules, so the shim answers that question from `set` and returns a
//   boolean. It deliberately does not rebuild a regular expression: matching stays in minimatch's
//   own `match()`.
//
// When to delete it
//   As soon as a vscode-languageclient release accepts minimatch >= 11 and no longer calls
//   `makeRe()`. Remove this module, its `require` in extension.js, test/minimatch-compat.test.js,
//   and `overrides.minimatch` in package.json together.

// The same hoisted copy the client resolves: the override leaves exactly one minimatch installed.
const { Minimatch } = require("minimatch");

if (typeof Minimatch.prototype.makeRe !== "function") {
  Object.defineProperty(Minimatch.prototype, "makeRe", {
    configurable: true,
    writable: true,
    value: function makeRe() {
      return this.set.length > 0;
    },
  });
}
