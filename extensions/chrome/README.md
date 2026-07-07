# sql-dialect-fmt for SQL editors

Chrome extension that formats the active Snowsight or Databricks SQL editor with the repository's
Rust formatter compiled to WebAssembly.

## Build

From the repository root:

```sh
./scripts/build-chrome-extension.sh
```

This builds `sql-dialect-fmt-wasm` for `wasm32-unknown-unknown` and copies the compiled module to
`extensions/chrome/vendor/sql_dialect_fmt_wasm.wasm`.

To build the release zip (and the VS Code VSIX at the same time), run:

```sh
./scripts/package-extensions.sh
```

## Install Locally

1. Open `chrome://extensions`.
2. Enable Developer mode.
3. Choose Load unpacked.
4. Select `extensions/chrome`.

## Use

Open Snowsight or a Databricks SQL editor, focus the editor, then use one of:

- the floating `sql-dialect-fmt` button
- the extension action button
- `Alt+Shift+F`

If a SQL range is selected, only that range is formatted. Otherwise the whole active editor is
formatted.

Open the extension options page to choose Snowflake or Databricks mode and adjust line width,
indent width, and keyword casing.
