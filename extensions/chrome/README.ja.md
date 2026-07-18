<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt for SQL editors

Chrome拡張機能で、アクティブなSnowsightまたはDatabricks SQLエディタをリポジトリの
RustフォーマッタをWebAssemblyにコンパイルしたものでフォーマットします。

## ビルド

リポジトリのルートから:

```sh
./scripts/build-chrome-extension.sh
```

これにより、`wasm32-unknown-unknown`用の`sql-dialect-fmt-wasm`がビルドされ、コンパイルされたモジュールが
`extensions/chrome/vendor/sql_dialect_fmt_wasm.wasm`にコピーされます。

リリースZIPをビルドするには（同時にVS CodeのVSIXも）、次を実行します:

```sh
./scripts/package-extensions.sh
```

## ローカルにインストール

1. `chrome://extensions`を開きます。
2. 開発者モードを有効にします。
3. 「未パッケージの拡張機能を読み込む」を選択します。
4. `extensions/chrome`を選択します。

## 使用方法

SnowsightまたはDatabricks SQLエディタを開き、エディタにフォーカスを合わせてから、次のいずれかを使用します:

- 浮動する`sql-dialect-fmt`ボタン
- 拡張機能アクションボタン
- `Alt+Shift+F`

SQLの範囲が選択されている場合、その範囲のみがフォーマットされます。そうでない場合は、アクティブなエディタ全体が
フォーマットされます。

拡張機能のオプションページを開いて、SnowflakeまたはDatabricksモードを選択し、行幅、
インデント幅、およびキーワードの大文字小文字を調整します。

これらのフォーマッタの設定は、`chrome.storage.sync`を通じてのみ保存されます。SQLテキストは
アクティブなエディタから必要に応じて読み取られ、バンドルされたWebAssemblyモジュールでローカルにフォーマットされ、
外部サービスに保存または送信されることなく書き戻されます。プライバシーに関する詳細は
[プライバシーポリシー](../../docs/PRIVACY.md)を参照してください。