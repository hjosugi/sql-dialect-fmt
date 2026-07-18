<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# Snowflake SQL

Visual Studio Code 用の Snowflake SQL 構文ハイライト**とフォーマット**。[sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt) で使用されるのと同じエンジンに基づいています。

![Snowflake SQL構文ハイライト](images/syntax-highlighting.png)

## 機能

- ドキュメント・選択範囲のフォーマット（**Format Document** / **Format Selection**）、保存時フォーマット対応
- オプトイン方式の言語サーバー統合 — 診断、ホバー、補完、セマンティックハイライト、
  アウトライン、折りたたみ（[言語サーバー](#言語サーバーオプション)を参照）
- Snowflake SQLキーワードと組み込み型
- Snowflakeスクリプティングと`$$ ... $$`ルーチン本体
- 行コメント（`--`, `//`）とブロックコメント（`/* ... */`）
- 文字列、引用識別子、数値リテラル、および演算子
- 位置変数`$1`、セッション変数`$name`、バインド変数`:name`、および`?`
- `@stage`、`@~`、`@%table`、および名前空間付きステージ参照
- `.sql`、`.snowsql`、および`.sfsql`ファイルの関連付け

- **言語ID:** `snowflake-sql`
- **スコープ名:** `source.snowflake-sql`
- **ファイルタイプ:** `.sql`、`.snowsql`、`.sfsql`

## フォーマット

この拡張機能は `snowflake-sql` ドキュメント用のフォーマッタを登録するため、**Format Document**、**Format Selection**、および `"editor.formatOnSave"` がそのまま動作します。フォーマットは完全にローカルで実行されます。バンドルされた WebAssembly ビルドのフォーマッタは、CLI と Snowsight ブラウザ拡張機能を動かしているのと同じエンジンです。ネットワークには何も送信されません。

フォーマットは機械的に**ロスレスかつ冪等**です — パースできない入力は変更されずにそのまま通過し、`format(format(x)) == format(x)` が成り立ちます。

これらのファイルのデフォルトフォーマッタにするには、設定に以下を追加してください：

```json
"[snowflake-sql]": {
  "editor.defaultFormatter": "sql-dialect-fmt.snowflake-sql-sql-dialect-fmt",
  "editor.formatOnSave": true
}
```

### 設定

| 設定 | デフォルト | 説明 |
| --- | --- | --- |
| `sqlDialectFmt.dialect` | `snowflake` | SQLダイアレクト（`snowflake` または `databricks`）。 |
| `sqlDialectFmt.lineWidth` | `100` | 折り返し前の目標行幅。 |
| `sqlDialectFmt.indentWidth` | `4` | インデントレベルあたりのスペース数。 |
| `sqlDialectFmt.uppercaseKeywords` | `true` | SQLキーワードを大文字化する。 |
| `sqlDialectFmt.lsp.enabled` | `false` | `sql-dialect-fmt-lsp` 言語サーバーへのオプトイン（下記参照）。 |
| `sqlDialectFmt.lsp.path` | `""` | `sql-dialect-fmt-lsp` のパス。空なら `PATH` から検索。 |

キーワードと型の単語リストは、`sql-dialect-fmt-highlight`のテスト（`tests/textmate.rs`）によってフォーマッタのレクサー/ハイライターと連動して維持されています。文法がキーワードまたは型としてスコープするすべての単語は、`sql_dialect_fmt_highlight::classify`によって同じように分類されなければならないため、文法はツールチェーンの他の部分から逸脱することはできません。

## 言語サーバー（オプション）

ここまでの機能はすべて外部バイナリなしでそのまま動作します。フォーマット以外の機能 —
Lint 診断、ホバードキュメント、補完、セマンティックハイライト、ドキュメントシンボル
（アウトライン）、折りたたみ範囲、入力時フォーマット — については、この拡張機能から
[`sql-dialect-fmt-lsp`](https://crates.io/crates/sql-dialect-fmt-lsp) 言語サーバーを起動する
こともできます。これはオプトインで、デフォルトでは無効です：

1. サーバーをインストールします：`cargo install sql-dialect-fmt-lsp`。
2. `"sqlDialectFmt.lsp.enabled": true` を設定します。バイナリが `PATH` 上にない場合は
   `sqlDialectFmt.lsp.path` でパスを指定してください。

サーバーの実行中は **Format Document** / **Format Selection** / 保存時フォーマットもサーバーが
担当し（最寄りの `sql-dialect-fmt.toml` をエディタ設定の下にレイヤーします）、内蔵の
WebAssembly フォーマッタは登録解除されるため、両者が競合することはありません。サーバーを
有効にしてもバイナリが見つからない、または起動に失敗した場合、拡張機能は理由を
**sql-dialect-fmt** 出力チャンネルに記録し、バンドルされた WebAssembly フォーマッタに静かに
フォールバックします — 拡張機能が動作するためにバイナリのインストールが必須になることは
ありません。

`sqlDialectFmt.*` 設定はサーバーに転送され、サーバーは個別の診断を切り替える
`sqlDialectFmt.lint.*` 設定も受け付けます。バンドルされたフォーマッタと同様に、サーバーは
stdio 上で LSP を話すローカルプロセスであり、ネットワークには一切アクセスしません。

## 使用方法

1. 拡張機能をインストールします。
2. `.sql`、`.snowsql`、または`.sfsql`ファイルを開きます。
3. 必要に応じて、**言語モードの変更**を選択し、**Snowflake SQL**を選択します。
4. **Format Document**（`Shift+Alt+F`）または **Format Selection** を実行します。

この拡張機能は構文ハイライト、言語メタデータ、およびローカルフォーマッタを提供します。SQLをSnowflakeに対して実行したり、アカウントに接続したりすることはありません。CLIフォーマッティングやその他の統合については、[メインプロジェクトのREADME](https://github.com/hjosugi/sql-dialect-fmt#readme)を参照してください。

## プライバシー

この拡張機能は、テレメトリーや分析を実行せず、ネットワークリクエストを行わず、リモートフォーマッティングも行いません。フォーマットはバンドルされた WebAssembly モジュールによってローカルで実行され、SQL がマシンの外に出ることはありません。この拡張機能が提供するのは、静的な言語設定、TextMate文法、ローカルフォーマッタ、そして — オプトインした場合のみ — 同じくマシン上で完結するローカル `sql-dialect-fmt-lsp` プロセスのクライアントです。[プライバシーポリシー](https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md)を参照してください。

## その他のエディタ

このディレクトリには他のエディタ向けの統合もあります。いずれも同じ `sql-dialect-fmt-lsp`
言語サーバー（`cargo install sql-dialect-fmt-lsp`）で動作します：

- [`nvim/`](nvim/) — 小さな Neovim プラグイン：`snowflake-sql` ファイルタイプ、LSP セットアップ、
  および CLI ベースのフォーマット用 conform.nvim / null-ls レシピ。
- [`zed/`](zed/) — Zed 拡張機能（開発インストール）：バンドルされた tree-sitter 文法と
  言語サーバーによる Snowflake SQL 言語。
- [`helix/`](helix/) — Helix 用のドキュメント化された `languages.toml` スニペット
  （プラグインシステムなし）。

## サポートとソース

- [問題を報告する](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [ソースコード](https://github.com/hjosugi/sql-dialect-fmt)
- ライセンス: [0BSD](LICENSE.md)
