<!-- i18n: language-switcher -->
[English](PRIVACY.md) | [日本語](PRIVACY.ja.md)

# プライバシー

このポリシーは VS Code 拡張 `Snowflake SQL (sql-dialect-fmt)` を対象とします。

フォーマットは同梱 WebAssembly formatter により VS Code extension host 内でローカル実行
されます。SQL、設定、ファイル名、認証情報、account identifier、利用分析を maintainer や
外部 formatting service へ送信しません。

任意設定 `sqlDialectFmt.lsp.enabled` を有効にした場合、ローカルにインストールされた
`sql-dialect-fmt-lsp` を stdio で起動します。language server も document をローカル処理し、
拡張のための network request は行いません。

拡張自身は SQL text を保存しません。user/workspace settings の永続化は VS Code 通常動作に
従います。

プライバシーまたは security concern は repository の security policy から報告してください。
