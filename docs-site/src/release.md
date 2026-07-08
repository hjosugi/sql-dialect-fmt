# Release And Distribution

The project uses one workspace version in the root `Cargo.toml`. Published crates inherit that
version and internal dependency versions are centralized in `[workspace.dependencies]`.

## Release Gate

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo bench -p sql-dialect-fmt-formatter --bench format -- --test
cargo fmt --all --check
scripts/run-external-corpus.sh --sample
scripts/conformance-report.py --path crates/sql-dialect-fmt-formatter/tests/corpus_sample \
  --out target/conformance-report.md
```

## Assets

`scripts/package-extensions.sh` builds the Chrome extension zip and VS Code VSIX under
`target/dist/`. Version tags create the GitHub Release, publish release binaries, and push the GHCR
image. Store publishing remains gated by repository variables and secrets documented in
`docs/STORE_PUBLISHING.md`.

## Docs Site

```sh
scripts/build-docs-site.sh
```

The docs workflow builds this mdBook, copies the WebAssembly formatter into the site output, and
deploys GitHub Pages from `main`.
