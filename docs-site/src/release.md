# Release And Distribution

The project uses one workspace version in the root `Cargo.toml`. Published crates inherit that
version and internal dependency versions are centralized in `[workspace.dependencies]`.

## Release Gate

```sh
task test
task clippy
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo bench -p sql-dialect-fmt-formatter --bench format -- --test
task fmt:check
scripts/run-external-corpus.sh --sample
scripts/conformance-report.py --path crates/sql-dialect-fmt-formatter/tests/corpus_sample \
  --out target/conformance-report.md
```

## Assets

`task vscode:package` builds and validates the VS Code VSIX under `target/dist/`. Version tags
create the GitHub Release, publish release binaries, and push the GHCR
image. Store publishing remains gated by repository variables and secrets documented in
`docs/STORE_PUBLISHING.md`.

## Docs Site

```sh
scripts/build-docs-site.sh
```

The docs workflow builds this mdBook, copies the WebAssembly formatter into the site output, and
deploys GitHub Pages from `main`.
