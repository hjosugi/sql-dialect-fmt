# Contributing to snow-fmt

Thanks for helping make Snowflake SQL tooling better. This project is young, so
small, careful changes are especially valuable.

## Development Setup

Required:

- Rust stable
- Node.js, only when working on `tree-sitter-snowflake`

Run the core checks:

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Run Tree-sitter checks:

```sh
cd tree-sitter-snowflake
npm exec --package tree-sitter-cli@0.26.9 -- tree-sitter generate
npm exec --package tree-sitter-cli@0.26.9 -- tree-sitter test
```

`cargo test --workspace` must stay self-contained. Stable SQL fixtures belong in
`crates/snow-fmt-test-fixtures`; generated or large local corpora should stay
outside the repository and can be passed to the CLI with `--fixtures`.

## Project Shape

- `snow-fmt-syntax`: shared `SyntaxKind`, keyword lookup, rowan language type.
- `snow-fmt-encoding`: file byte decoding/re-encoding boundary.
- `snow-fmt-lexer`: lossless, allocation-light tokenizer.
- `snow-fmt-parser`: resilient CST parser. Parsing should not panic on broken SQL.
- `snow-fmt-highlight`: lexical highlight classification.
- `snow-fmt-hover`: editor/LSP-ready hover strings for Snowflake concepts.
- `snow-fmt-tree-sitter`: Rust bindings for the generated Tree-sitter grammar.
- `tree-sitter-snowflake`: grammar package and editor queries.

For the longer map, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Change Guidelines

- Prefer small PRs with one clear purpose.
- Preserve losslessness: joining token texts should recreate the source exactly.
- Add tests next to the layer you changed.
- Keep parser errors recoverable. A mid-edit SQL file should still produce useful output.
- Avoid large refactors unless they remove real complexity or unblock a planned phase.
- When adding Snowflake syntax, include a source link in the PR description.

## Good First Contributions

- Add hover text for a Snowflake type, task property, or procedure option.
- Add a focused lexer/parser regression test for a small Snowflake example.
- Improve Tree-sitter highlight captures.
- Clarify docs where you got confused.

## Pull Request Checklist

- [ ] `cargo fmt --all --check`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Tree-sitter grammar regenerated and tested, if `tree-sitter-snowflake/` changed
- [ ] Docs updated, if behavior or public API changed
