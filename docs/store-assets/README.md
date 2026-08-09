<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# VS Code Marketplace assets

The shipped Marketplace artwork lives in `editors/images/`:

- `icon.png`: 256×256 PNG with alpha
- `syntax-highlighting.png`: 1280×800 PNG

`source/vscode-syntax-demo.html` and `source/tokens.css` are the deterministic source for the
syntax-highlighting screenshot. They contain only fictional workspace names and demo SQL.

Run `python3 scripts/check-store-assets.py` to validate dimensions, alpha, and package references.
