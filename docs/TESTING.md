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

## What to Test Where

Shared helpers:

- put mechanical invariants in `snow-fmt-test-support`
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

## Fixture Policy

`cargo test --workspace` must be self-contained. Keep stable, curated examples in
`crates/snow-fmt-test-fixtures`.

Curated SQL fixtures are stored in `snow-fmt-test-fixtures` and exposed through
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

Broader generated corpora should stay outside the repository. Use the CLI
`--fixtures` flag for one-off local checks rather than committing generated
fixture directories.

Formatter corpus checks have one additional always-on layer:
`crates/snow-fmt-formatter/tests/corpus_sample/`. These files are committed in
formatter-canonical form and are checked by `external_corpus.rs` for
idempotency, significant-token preservation, and clean reparse. Larger local or
private corpora should use `SNOW_FMT_EXTERNAL_CORPUS`; see `docs/CORPUS.md`.
