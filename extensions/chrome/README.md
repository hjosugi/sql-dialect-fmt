# snow-fmt for Snowsight

Chrome extension that formats the active Snowsight SQL editor with the repository's Rust
formatter compiled to WebAssembly.

## Build

From the repository root:

```sh
./scripts/build-chrome-extension.sh
```

This builds `snow-fmt-wasm` for `wasm32-unknown-unknown` and copies the compiled module to
`extensions/chrome/vendor/snow_fmt_wasm.wasm`.

## Install Locally

1. Open `chrome://extensions`.
2. Enable Developer mode.
3. Choose Load unpacked.
4. Select `extensions/chrome`.

## Use

Open Snowsight, focus a worksheet editor, then use one of:

- the floating `snow-fmt` button
- the extension action button
- `Alt+Shift+F`

If a SQL range is selected, only that range is formatted. Otherwise the whole active editor is
formatted.
