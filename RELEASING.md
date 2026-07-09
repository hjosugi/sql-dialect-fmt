# Releasing sql-dialect-fmt

sql-dialect-fmt ships as a set of crates that share **one workspace version**, declared
once in the root `Cargo.toml` under `[workspace.package]` and inherited by every
crate via `version.workspace = true`. Bumping that single line versions the whole
workspace coherently.

## Crate publication map

Published to crates.io (in dependency order):

| order | crate | depends on |
| --- | --- | --- |
| 1 | `sql-dialect-fmt-syntax` | — |
| 2 | `sql-dialect-fmt-text` | — |
| 3 | `sql-dialect-fmt-lexer` | syntax, text |
| 4 | `sql-dialect-fmt-parser` | syntax, lexer, text |
| 5 | `sql-dialect-fmt-formatter` | syntax, lexer, parser |
| 6 | `sql-dialect-fmt-highlight` | syntax, lexer, text |
| 7 | `sql-dialect-fmt-hover` | syntax, lexer |
| 8 | `sql-dialect-fmt-encoding` | — |
| 9 | `sql-dialect-fmt` | encoding, formatter, parser, text |
| 10 | `sql-dialect-fmt-lsp` | formatter, highlight, hover, parser, text |
| 11 | `sql-dialect-fmt-wasm` | formatter |

**Not published** (`publish = false`):

- `sql-dialect-fmt-test-fixtures` — embedded golden fixtures used only by tests.
- `sql-dialect-fmt-test-support` — shared assertion helpers used only by tests.
- `sql-dialect-fmt-tree-sitter` — its `build.rs` compiles the bundled tree-sitter C
  parser/scanner from `../../tree-sitter-snowflake`, which lives outside the crate
  directory and is therefore not included in the `cargo package` tarball, so
  `cargo publish` verification cannot rebuild it.

## Release procedure

1. **Bump the release version.** Use the updater so the workspace version, internal dependency
   versions, `Cargo.lock`, extension package versions, Homebrew formula tag, and install examples
   stay in sync:

   ```sh
   scripts/update-version.py X.Y.Z
   ```

   If you want the helper to create the changelog release heading and compare links too, pass
   `--changelog`:

   ```sh
   scripts/update-version.py X.Y.Z --changelog --date YYYY-MM-DD
   ```

2. **Update the changelog.** Move the `## [Unreleased]` notes in `CHANGELOG.md` into a
   new `## [X.Y.Z] - YYYY-MM-DD` section and refresh the compare links.

3. **Run the green gate** from the workspace root:

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

4. **Package release assets**:

   ```sh
   scripts/package-extensions.sh
   ```

   This builds the Snowsight Chrome extension zip and the VS Code VSIX under `target/dist/`.
   The GitHub Release workflow uploads those alongside the CLI tarball and checksum.

5. **Dry-run packaging** of dependency-free publishable crates:

   ```sh
   cargo publish --dry-run -p sql-dialect-fmt-syntax
   cargo publish --dry-run -p sql-dialect-fmt-text
   cargo publish --dry-run -p sql-dialect-fmt-encoding
   ```

   For a new workspace version, dependent crate dry-runs cannot resolve internal `X.Y.Z`
   dependencies until those predecessor crates are actually published and indexed. The ordered
   publish helper below relies on `cargo publish` verification for dependent crates after each
   predecessor appears in the index.

   (`cargo package -p <crate>` produces the tarball without the dry-run upload check.)

6. **Commit and tag:**

   ```sh
   git commit -am "release: vX.Y.Z"
   git tag vX.Y.Z
   git push && git push --tags
   ```

7. **Publish in dependency order.** Each `cargo publish` must complete and the new
   version must be indexed before publishing a dependent crate. The helper publishes the
   canonical order and waits for each just-published crate to resolve through Cargo before moving
   on:

   ```sh
   scripts/publish-crates.sh
   ```

   The canonical order is **syntax → text → lexer → parser → formatter → highlight → hover →
   cli / lsp / wasm** (with `encoding` published any time before `cli`, and `wasm`
   any time after `formatter`).

8. **Store publishing** is automated after one-time store setup. Follow
   [docs/STORE_PUBLISHING.md](docs/STORE_PUBLISHING.md) for the exact no-decision setup runbook.

   The `Release` workflow packages the Chrome zip and VSIX on `v*.*.*` tags and can publish to
   the stores automatically on tag push once the repository variables below are enabled. The
   `Extension Packages` workflow remains available for manual package/publish runs.

   One-time VS Code Marketplace setup:

   - Create/verify the Marketplace publisher used by `editors/package.json`.
   - Prefer Microsoft Entra ID workload identity for new automation. Configure
     `VSCE_AUTH_MODE=azure` plus `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, and
     optional `AZURE_SUBSCRIPTION_ID` as repository variables or secrets, and grant that managed
     identity Contributor access to the Marketplace publisher.
   - For the simpler PAT path, set secret `VSCE_PAT` and leave `VSCE_AUTH_MODE` unset or `pat`.
     Global Azure DevOps PATs retire on 2026-12-01, so treat this as the short path rather than the
     long-term one.
   - Enable automatic VS Code publishing with repository variable
     `VSCODE_MARKETPLACE_AUTO_PUBLISH=true`, or set `EXTENSIONS_AUTO_PUBLISH=true` to publish both
     stores from tag pushes.

   One-time Chrome Web Store setup:

   - Register the developer account, create the item, and complete the first listing/privacy/
     distribution fields in the Chrome Web Store dashboard.
   - Enable the Chrome Web Store API in a Google Cloud project, configure OAuth consent, and create
     an OAuth client/refresh token with Chrome Web Store scope.
   - Set variables `CHROME_PUBLISHER_ID` and `CHROME_EXTENSION_ID`.
   - Set secrets `CHROME_CLIENT_ID`, `CHROME_CLIENT_SECRET`, and `CHROME_REFRESH_TOKEN`.
   - Enable automatic Chrome publishing with repository variable `CHROME_WEBSTORE_AUTO_PUBLISH=true`,
     or set `EXTENSIONS_AUTO_PUBLISH=true` to publish both stores from tag pushes.

   To minimize GitHub UI work after the store-side setup is done, export the credentials locally
   and let the helper write repository variables/secrets with `gh`:

   ```sh
   # PAT path, both stores, tag-push auto-publish enabled.
   export VSCE_PAT=...
   export CHROME_PUBLISHER_ID=...
   export CHROME_EXTENSION_ID=...
   export CHROME_CLIENT_ID=...
   export CHROME_CLIENT_SECRET=...
   export CHROME_REFRESH_TOKEN=...
   scripts/configure-extension-publishing.sh --target all --vscode-auth pat

   # Long-term VS Code auth path. Run instead of the PAT command after the Entra identity
   # has been authorized as a Marketplace publisher contributor.
   export AZURE_CLIENT_ID=...
   export AZURE_TENANT_ID=...
   export AZURE_SUBSCRIPTION_ID=... # optional
   scripts/configure-extension-publishing.sh --target vscode --vscode-auth azure
   ```

   Manual fallback remains available from the `Extension Packages` workflow: choose `publish=true`
   and `publish_target=all|vscode|chrome`.

## Notes

- License is `0BSD`; the license text lives in `LICENSE` at the repo root.
- Crate metadata (`description`, `keywords`, `categories`, `readme`, `repository`,
  `homepage`, `license`) is inherited from `[workspace.package]` where possible; the
  per-crate `description` is the only required field set locally.
