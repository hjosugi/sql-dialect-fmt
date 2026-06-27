# Releasing sql-dialect-fmt

sql-dialect-fmt ships as a set of crates that share **one workspace version**, declared
once in the root `Cargo.toml` under `[workspace.package]` and inherited by every
crate via `version.workspace = true`. Bumping that single line versions the whole
workspace coherently.

## Crate publication map

Published to crates.io (in dependency order):

| order | crate | depends on |
| --- | --- | --- |
| 1 | `snow-fmt-syntax` | — |
| 2 | `snow-fmt-lexer` | syntax |
| 3 | `snow-fmt-parser` | syntax, lexer |
| 4 | `snow-fmt-formatter` | syntax, parser |
| 5 | `snow-fmt-highlight` | syntax, lexer |
| 6 | `snow-fmt-hover` | syntax, lexer |
| 7 | `snow-fmt-encoding` | — |
| 8 | `sql-dialect-fmt` | encoding, formatter |
| 9 | `snow-fmt-lsp` | formatter, highlight, hover, parser, syntax |

**Not published** (`publish = false`):

- `snow-fmt-test-fixtures` — embedded golden fixtures used only by tests.
- `snow-fmt-test-support` — shared assertion helpers used only by tests.
- `snow-fmt-tree-sitter` — its `build.rs` compiles the bundled tree-sitter C
  parser/scanner from `../../tree-sitter-snowflake`, which lives outside the crate
  directory and is therefore not included in the `cargo package` tarball, so
  `cargo publish` verification cannot rebuild it.

## Release procedure

1. **Bump the workspace version.** Edit `version` under `[workspace.package]` in the
   root `Cargo.toml`. Because every crate uses `version.workspace = true`, and every
   internal path dependency pins `version = "X.Y.Z"` to match, update those pins to
   the new version as well (search for the previous version string across all
   `*/Cargo.toml`).

2. **Update the changelog.** Move the `## [Unreleased]` notes in `CHANGELOG.md` into a
   new `## [X.Y.Z] - YYYY-MM-DD` section and refresh the compare links.

3. **Run the green gate** from the workspace root:

   ```sh
   cargo test --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   cargo fmt --all --check
   ```

4. **Dry-run packaging** of each publishable crate, in dependency order:

   ```sh
   cargo publish --dry-run -p snow-fmt-syntax
   cargo publish --dry-run -p snow-fmt-parser
   cargo publish --dry-run -p snow-fmt-formatter
   cargo publish --dry-run -p sql-dialect-fmt
   # (and the rest below)
   ```

   (`cargo package -p <crate>` produces the tarball without the dry-run upload check.)

5. **Commit and tag:**

   ```sh
   git commit -am "release: vX.Y.Z"
   git tag vX.Y.Z
   git push && git push --tags
   ```

6. **Publish in dependency order.** Each `cargo publish` must complete and the new
   version must be indexed before publishing a dependent crate:

   ```sh
   cargo publish -p snow-fmt-syntax
   cargo publish -p snow-fmt-lexer
   cargo publish -p snow-fmt-parser
   cargo publish -p snow-fmt-formatter
   cargo publish -p snow-fmt-highlight
   cargo publish -p snow-fmt-hover
   cargo publish -p snow-fmt-encoding
   cargo publish -p sql-dialect-fmt
   cargo publish -p snow-fmt-lsp
   ```

   The canonical order is **syntax → lexer → parser → formatter → highlight → hover →
   cli / lsp** (with `encoding` published any time before `cli`).

## Notes

- License is `MIT OR Apache-2.0`; both `LICENSE-MIT` and `LICENSE-APACHE` live at the
  repo root.
- Crate metadata (`description`, `keywords`, `categories`, `readme`, `repository`,
  `homepage`, `license`) is inherited from `[workspace.package]` where possible; the
  per-crate `description` is the only required field set locally.
