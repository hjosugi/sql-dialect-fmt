<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt for Helix

Helix にはプラグインシステムがないため、このディレクトリはパッケージではなくドキュメントです。
[`languages.toml`](languages.toml) は、`sql-dialect-fmt-lsp` 言語サーバー（フォーマット、診断、
ホバー、セマンティックトークン、補完）とバンドルされた
[`tree-sitter-snowflake`](../../tree-sitter-snowflake) 文法を Helix に組み込む、コピーして
使えるスニペットです。

## セットアップ

1. 言語サーバーを `PATH` 上にインストールします：

   ```sh
   cargo install sql-dialect-fmt-lsp --locked
   ```

2. [`languages.toml`](languages.toml) から必要な部分を `~/.config/helix/languages.toml`
   （またはプロジェクトローカルの `.helix/languages.toml`）にコピーします。

3. `[[grammar]]` ブロックを追加した場合は、文法をビルドしてクエリをインストールします：

   ```sh
   hx --grammar fetch
   hx --grammar build

   # ハイライトクエリは Helix ランタイムディレクトリの言語ごとの場所に置きます：
   mkdir -p ~/.config/helix/runtime/queries/snowflake
   cp tree-sitter-snowflake/queries/{highlights,injections,locals}.scm \
     ~/.config/helix/runtime/queries/snowflake/
   ```

   クエリは意図的に一般的なキャプチャ名を使っているため、Helix はほとんどをそのまま
   マップできます（主な差分は `@number` — Helix のテーマは `@constant.numeric` を使います）。

4. `hx --health snowflake-sql` で結果を確認します。

スニペットは `*.snowsql` / `*.sfsql` 用の専用 `snowflake-sql` 言語を定義し、素の `*.sql` は
意図的に Helix 組み込みの `sql` 言語に委ねています。

## 素の `*.sql` でも使う

`snowflake-sql` 言語の `file-types` に `"sql"` を追加するか、組み込みの SQL 言語を保ったまま
サーバーだけをアタッチします：

```toml
[[language]]
name = "sql"
language-servers = ["sql-dialect-fmt-lsp"]
auto-format = true
```

## 代わりに CLI でフォーマットする

フォーマットだけが必要な場合（診断・ホバー・補完が不要な場合）は、言語サーバーの代わりに
CLI を言語フォーマッタとして使えます — stdin を読み stdout に書き出します：

```toml
[[language]]
name = "sql"
formatter = { command = "sql-dialect-fmt" }
auto-format = true
```

インストールは `cargo install sql-dialect-fmt --locked`、Homebrew
（`brew tap hjosugi/sql-dialect-fmt https://github.com/hjosugi/sql-dialect-fmt && brew install sql-dialect-fmt`）、
または[リリース tarball](https://github.com/hjosugi/sql-dialect-fmt/releases) で行えます。

## 設定

エディタ側の設定は `[language-server.sql-dialect-fmt-lsp.config.sqlDialectFmt]` に置きます
（`lineWidth`、`indentWidth`、`dialect`、`uppercaseKeywords`、`keywordCase`、`lineEnding`、
`lint.*`）。サーバーはオプションを**デフォルト → 最寄りの `sql-dialect-fmt.toml` →
エディタ設定**の順で重ねるため、プロジェクトの設定ファイルによって Helix と CI が一貫します。

## サポートとソース

- [問題を報告する](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [ソースコード](https://github.com/hjosugi/sql-dialect-fmt)
- ライセンス: [0BSD](../../LICENSE)
