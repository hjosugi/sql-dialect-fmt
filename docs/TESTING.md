<!-- i18n: language-switcher -->
[English](TESTING.md) | [日本語](TESTING.ja.md)

# Testing

The test suite is intentionally layered. When a change fails, the failing crate
should point to the layer that regressed.

## Standard Gates

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Tree-sitter grammar checks:

```sh
cd tree-sitter-snowflake
npm exec --package tree-sitter-cli@0.26.9 -- tree-sitter generate
npm exec --package tree-sitter-cli@0.26.9 -- tree-sitter test
```

VS Code extension checks:

```sh
./scripts/build-vscode-extension.sh
python3 scripts/check-vsix-package.py path/to/sql-dialect-fmt.vsix
```

The build script compiles the real Wasm formatter, bundles the extension host, and runs the
TextMate and bundle-to-Wasm integration tests. The package validator runs after creating a VSIX and
guards its manifest, embedded-language mapping, required assets, single JavaScript bundle, and
absence of `node_modules`.

## Fuzzing

Coverage-guided fuzzing lives in the excluded `fuzz/` crate so normal workspace
checks stay fast and self-contained. The scheduled `Fuzz` workflow runs the same
targets weekly and uploads `fuzz/artifacts/` plus the generated corpus for any
failing target.

```sh
cargo install cargo-fuzz --locked
cargo +nightly fuzz run lexer_roundtrip
cargo +nightly fuzz run parser_lossless
cargo +nightly fuzz run formatter_idempotent
```

For a bounded local smoke run:

```sh
cargo +nightly fuzz run parser_lossless -- -max_total_time=60
```

## What to Test Where

Shared helpers:

- put mechanical invariants in `sql-dialect-fmt-test-support`
- keep individual test files focused on named case tables and expected behavior
- prefer context-bearing helpers for fixture loops so failures name the fixture

Encoding:

- UTF-8, UTF-8 BOM, and UTF-16 LE/BE BOM round-trip byte-for-byte
- invalid or unsupported bytes remain opaque and are not rewritten
- CLI tests should cover encoding preservation at the file boundary

Lexer:

- every byte is covered by exactly one token
- token text joins back to the original input
- unterminated strings/comments produce diagnostics, not panics
- delimiter changes stay table-driven and do not swallow variables/operators
- LF, CRLF, old Mac CR, and mixed line endings

Parser:

- CST shape for supported grammar
- resilient recovery for incomplete SQL
- lossless round trip for broken and valid input
- long inputs that could expose quadratic behavior

Highlight:

- stable capture categories for editor adapters
- byte ranges over Unicode and mixed newlines
- Snowflake-specific operators and types
- TextMate injection tests with `vscode-textmate` and `vscode-oniguruma`, including entry into and
  exit from embedded JavaScript

Hover:

- range selection for the hovered token or declaration name
- concise summaries for procedures, tasks, types, and properties
- broken mid-edit SQL should not panic

Tree-sitter:

- corpus examples for public grammar behavior
- query compilation
- real capture execution for important highlight scopes
- generated `src/parser.c` and `src/node-types.json` committed with grammar changes
- body delimiter rule changes reflected in corpus and generated parser files

VS Code:

- the bundled extension registers document and range formatting providers
- the provider sends a realistic fixture through the packaged Wasm ABI and compares exact output
- embedded JavaScript uses VS Code's `source.js` scope without leaking into the following SQL
- the VSIX contains one JavaScript bundle, the Wasm module, grammar, manifest, and no dependency
  tree

## Fixture Policy

`cargo test --workspace` must be self-contained. Keep stable, curated examples in
`crates/sql-dialect-fmt-test-fixtures`.

Curated SQL fixtures are stored in `sql-dialect-fmt-test-fixtures` and exposed through
`EASY_CASES`. This corpus is the always-on minimum gate, not the whole quality
bar. The current minimum count is kept in `MINIMUM_EMBEDDED_EASY_CASES` so new
fixtures do not require updating every consumer test:

- CLI tests verify golden fixture discovery and profile mapping.
- Lexer/highlight/tree-sitter tests require every embedded fixture to be clean
  and lossless.
- Parser fixture tests require every embedded fixture to recover losslessly; add
  focused `clean` parser tests as grammar support lands.
- Add narrow table-driven tests beside the crate that owns the behavior when a
  bug is more specific than a fixture.

Do not treat the presence of an `expected.sql` file in `EASY_CASES` as proof that every consumer
compares formatter output byte-for-byte. For an output regression, add a focused regression fixture
whose expected bytes are asserted by the owning formatter test. If the bug crosses a distribution
boundary, exercise the same fixture through the relevant CLI, LSP stdio, raw Wasm ABI, or bundled
editor provider as well.

Broader generated corpora should stay outside the repository. Use the CLI
`--fixtures` flag for one-off local checks rather than committing generated
fixture directories.

Formatter corpus checks have one additional always-on layer:
`crates/sql-dialect-fmt-formatter/tests/corpus_sample/`. These files are committed in
formatter-canonical form and are checked by `external_corpus.rs` for
idempotency, significant-token preservation, and clean reparse. Larger local or
private corpora should use `SQL_DIALECT_FMT_EXTERNAL_CORPUS`; see `docs/CORPUS.md`.
