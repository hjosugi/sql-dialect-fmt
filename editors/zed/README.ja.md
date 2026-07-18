<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt for Zed

[sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt) の [Zed](https://zed.dev)
拡張機能です。**Snowflake SQL** 言語（バンドルされた
[`tree-sitter-snowflake`](../../tree-sitter-snowflake) 文法による tree-sitter ハイライト）を
宣言し、`sql-dialect-fmt-lsp` 言語サーバー（フォーマット、範囲フォーマット、入力時フォーマット、
診断、ホバー、セマンティックトークン、補完）を起動します。

デフォルトでは `*.snowsql` と `*.sfsql` を関連付けます。素の `*.sql` は他の SQL 拡張機能に
委ねます。Zed の設定でオプトインできます：

```json
{
  "file_types": {
    "Snowflake SQL": ["sql"]
  }
}
```

## インストール

1. 言語サーバーのバイナリを `PATH` 上にインストールします：

   ```sh
   cargo install sql-dialect-fmt-lsp --locked
   ```

2. **開発拡張機能**としてインストールします（まだ Zed の拡張機能レジストリには公開されて
   いません）：コマンドパレットから `zed: install dev extension` を実行し、このディレクトリ
   （リポジトリのチェックアウト内の `editors/zed`）を選択します。Zed は Rust のグルーコードを
   WebAssembly にコンパイルし、文法を取得するため、ビルドには `wasm32-wasip2` ターゲット付きの
   Rust ツールチェーンが必要です。

フォーマットは Zed の標準コマンド（`editor: format`、保存時フォーマット）を使用します。
サーバーはファイルごとに最寄りの `sql-dialect-fmt.toml` を発見するため、プロジェクトは CI の
CLI と同じ結果でフォーマットされます。

## 文法

`extension.toml` は `snowflake` 文法をこのリポジトリに向けています：

```toml
[grammars.snowflake]
repository = "https://github.com/hjosugi/sql-dialect-fmt"
rev = "9cd8a8c0da6f937a9d6ce417d188772bdbd5637f"
path = "tree-sitter-snowflake"
```

文法をローカルで開発する場合は、`repository` をチェックアウトの `file://` URL に、`rev` を
テストしたいコミットに向けてください。文法が変わったら `rev` を更新します。

## 設定

Zed はサーバーごとの設定を言語サーバーに引き渡します。サーバーはオプションをトップレベル
または `sqlDialectFmt` セクションのどちらでも受け付けます：

```json
{
  "lsp": {
    "sql-dialect-fmt-lsp": {
      "settings": {
        "sqlDialectFmt": {
          "lineWidth": 100,
          "indentWidth": 4,
          "dialect": "snowflake",
          "uppercaseKeywords": true
        }
      }
    }
  }
}
```

`binary` の上書きにも対応しています。例：
`"binary": { "path": "/path/to/sql-dialect-fmt-lsp" }`

設定は**デフォルト → 最寄りの `sql-dialect-fmt.toml` → エディタ設定**の順で重ねられます。

## サポートとソース

- [問題を報告する](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [ソースコード](https://github.com/hjosugi/sql-dialect-fmt)
- ライセンス: [0BSD](../../LICENSE)
