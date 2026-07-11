<!-- i18n: language-switcher -->
[English](PRIVACY.md) | [日本語](PRIVACY.ja.md)

# sql-dialect-fmt 拡張機能のプライバシーポリシー

最終更新日: 2026-07-11

このポリシーは以下に適用されます：

- `sql-dialect-fmt for Snowsight`、`extensions/chrome` の Chrome 拡張機能
- `Snowflake SQL (sql-dialect-fmt)`、`editors` の VS Code 拡張機能

## データ収集

これらの拡張機能は、ユーザーデータを収集、販売、送信、または共有しません。

Chrome 拡張機能は、ユーザーが拡張ボタン、ブラウザアクション、または `Alt+Shift+F` ショートカットからフォーマッタを実行したときにのみ、アクティブな Snowflake Snowsight または Databricks SQL エディタから SQL テキストを読み取ります。SQL は、拡張機能にバンドルされた WebAssembly モジュールによってローカルでフォーマットされ、アクティブなエディタに書き戻されます。拡張機能は SQL テキストを外部サーバーに送信しません。

VS Code 拡張機能は、Snowflake SQL 言語メタデータと TextMate 文法ファイルを提供します。テレメトリ、分析、ネットワークアップロード、またはリモートフォーマットは含まれていません。

## 権限

Chrome 拡張機能は、アクティブな SQL エディタを見つけ、選択された SQL またはエディタ全体の内容をフォーマットされた SQL で置き換えるために、Snowflake、Snowsight、および Databricks のホスト名へのアクセスを要求します。アクティブタブでユーザーのジェスチャーの後にのみブラウザアクションが実行されるように、`activeTab` を要求します。フォーマッタの設定を `chrome.storage.sync` を通じて保存するために `storage` を要求します。

## ストレージ

Chrome 拡張機能は、`chrome.storage.sync` を使用してフォーマッタの設定のみを保存します：SQL ダイアレクト、行幅、インデント幅、およびキーワードの大文字小文字。これらの設定は、ユーザーがブラウザの同期を有効にしている Chrome プロファイル間で同期される場合があります。SQL テキスト、認証情報、アカウント識別子、閲覧履歴、または分析イベントは保存しません。

## お問い合わせ

質問やセキュリティレポートについては、以下のリンクで問題をオープンしてください：

https://github.com/hjosugi/sql-dialect-fmt/issues