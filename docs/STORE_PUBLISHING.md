<!-- i18n: language-switcher -->
[English](STORE_PUBLISHING.md) | [日本語](STORE_PUBLISHING.ja.md)

# VS Code Marketplace Publishing

The repository publishes one editor package: the VS Code VSIX built from `editors/`.

## Local package

```sh
task vscode:package
```

The validated output is `target/dist/sql-dialect-fmt-v<version>.vsix`.

## Repository configuration

Use one Marketplace authentication mode:

- PAT: set secret `VSCE_PAT` and variable `VSCE_AUTH_MODE=pat`.
- Azure identity: set variables `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, optional
  `AZURE_SUBSCRIPTION_ID`, and `VSCE_AUTH_MODE=azure`.

The helper configures those values without printing secrets:

```sh
VSCE_PAT=... ./scripts/configure-extension-publishing.sh \
  --repo hjosugi/sql-dialect-fmt --vscode-auth pat
```

`VSCODE_MARKETPLACE_AUTO_PUBLISH=true` enables publishing on release tag pushes. The manual
`.github/workflows/extensions.yml` workflow can package or publish a selected version.

The release workflow validates version consistency, builds through go-task, validates the VSIX
contents, attaches it to the GitHub Release, and then updates the existing Marketplace listing.
