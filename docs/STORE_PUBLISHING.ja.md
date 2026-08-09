<!-- i18n: language-switcher -->
[English](STORE_PUBLISHING.md) | [日本語](STORE_PUBLISHING.ja.md)

# VS Code Marketplace 公開

この repository が配布する editor package は `editors/` から作る VS Code VSIX です。

## ローカル package

```sh
task vscode:package
```

検証済み output は `target/dist/sql-dialect-fmt-v<version>.vsix` です。

## Repository 設定

Marketplace authentication は次のどちらかを使います。

- PAT: secret `VSCE_PAT` と variable `VSCE_AUTH_MODE=pat`。
- Azure identity: variables `AZURE_CLIENT_ID`、`AZURE_TENANT_ID`、任意の
  `AZURE_SUBSCRIPTION_ID`、`VSCE_AUTH_MODE=azure`。

secret を表示せず設定する helper:

```sh
VSCE_PAT=... ./scripts/configure-extension-publishing.sh \
  --repo hjosugi/sql-dialect-fmt --vscode-auth pat
```

`VSCODE_MARKETPLACE_AUTO_PUBLISH=true` で release tag push 時の自動公開を有効化します。
`.github/workflows/extensions.yml` は任意 version の手動 package / publish に使えます。

release workflow は version consistency、go-task 経由の build、VSIX 内容検査、GitHub Release
添付を行い、既存 Marketplace listing を更新します。
