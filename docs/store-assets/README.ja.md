<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# VS Code Marketplace asset

配布する Marketplace 画像は `editors/images/` にあります。

- `icon.png`: 256×256、alpha付きPNG
- `syntax-highlighting.png`: 1280×800 PNG

`source/vscode-syntax-demo.html` と `source/tokens.css` はsyntax highlighting screenshotを
再現するためのsourceです。架空のworkspace名とdemo SQLだけを使用しています。

寸法、alpha、package参照は `python3 scripts/check-store-assets.py` で検査できます。
