# Contributing to sql-dialect-fmt

Thanks for helping make Snowflake SQL tooling better. This project is young, so
small, careful changes are especially valuable.

## Development Setup

Required:

- Rust stable
- [go-task](https://taskfile.dev/)
- Node.js, when working on the VS Code extension

Run the core checks:

```sh
task fmt:check
task check
task test
task clippy
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo bench -p sql-dialect-fmt-formatter --bench format -- --test
```

Tree-sitter sources are retained but paused outside the active workspace, CI, and release scope.

For VS Code extension changes, build the real Wasm artifact and run the bundled-provider and
TextMate integration tests:

```sh
task vscode:build
task vscode:package
```

`cargo test --workspace` must stay self-contained. Stable SQL fixtures belong in
`crates/sql-dialect-fmt-test-fixtures`; generated or large local corpora should stay
outside the repository and can be passed to the CLI with `--fixtures`.

Optional pre-commit setup:

```sh
pre-commit install
pre-commit run --all-files
```

## Project Shape

- `sql-dialect-fmt-syntax`: shared `SyntaxKind`, keyword lookup, rowan language type.
- `sql-dialect-fmt-encoding`: file byte decoding/re-encoding boundary.
- `sql-dialect-fmt-lexer`: lossless, allocation-light tokenizer.
- `sql-dialect-fmt-parser`: resilient CST parser. Parsing should not panic on broken SQL.
- `sql-dialect-fmt-highlight`: lexical highlight classification.
- `sql-dialect-fmt-hover`: editor/LSP-ready hover strings for Snowflake concepts.
- `sql-dialect-fmt-config`: shared `sql-dialect-fmt.toml` model and discovery for the CLI and LSP.
- `sql-dialect-fmt-tree-sitter` / `tree-sitter-snowflake`: retained, paused grammar sources.

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
- Improve formatter configuration or VS Code integration behavior.
- Clarify docs where you got confused.

## Pull Request Checklist

- [ ] `task fmt:check`
- [ ] `task check`
- [ ] `task test`
- [ ] `task clippy`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [ ] VS Code bundle/TextMate/Wasm integration tests and VSIX validation, if `editors/` changed
- [ ] Docs updated, if behavior or public API changed
