<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt

[![CI](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/ci.yml/badge.svg)](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/ci.yml)

Snowflake SQL と Databricks SQL のフォーマッタ＋シンタックスハイライタ（Rust 製）。`gofmt` / Prettier / Biome 流の opinionated・ほぼ設定なしの整形を目指します。

整形は **無破壊・べき等** を機械的に保証します（パースできない入力は無変更で素通し、整形しても有意トークンとコメントは保存、`format(format(x)) == format(x)`）。

## インストール

```sh
# crates.io から
cargo install sql-dialect-fmt --version 1.21.0 --locked

# このリポジトリから直接（`sql-dialect-fmt` バイナリが入る）
cargo install --git https://github.com/hjosugi/sql-dialect-fmt sql-dialect-fmt

# ローカルチェックアウトから
cargo install --path crates/sql-dialect-fmt-cli
# または: cargo build --release -p sql-dialect-fmt  →  target/release/sql-dialect-fmt

# cargo-binstall 対応リリースではバイナリ取得も可能
cargo binstall sql-dialect-fmt

# Homebrew。このリポジトリを tap として使う
brew tap hjosugi/sql-dialect-fmt https://github.com/hjosugi/sql-dialect-fmt
brew install sql-dialect-fmt
```

CI では同梱の composite action またはコンテナを使えます。

```yaml
- uses: hjosugi/sql-dialect-fmt@v1
  with:
    args: "sql/**/*.sql"
```

```sh
docker run --rm -v "$PWD:/work" -w /work ghcr.io/hjosugi/sql-dialect-fmt:1.21.0 --check .
```

## 使い方

```sh
sql-dialect-fmt query.sql                 # 整形して stdout へ
sql-dialect-fmt --write *.sql             # ファイルをその場で整形
sql-dialect-fmt --check src/**/*.sql      # 未整形なら非ゼロ終了（CI 向け）
sql-dialect-fmt --check --diff query.sql  # 未整形箇所を unified diff で表示
cat query.sql | sql-dialect-fmt           # stdin → stdout
cat query.sql | sql-dialect-fmt -         # `-` でも stdin を明示
sql-dialect-fmt --stdin-filepath src/query.sql < query.sql  # stdin に設定探索用パスを付与

# スタイル: --keyword-case upper|lower|preserve / --select-item-layout auto|vertical
#          --comma-style trailing|leading / --line-width N / --indent-width N
```

pre-commit 利用者は次の設定で `--write` または `--check` を使えます。

```yaml
repos:
  - repo: https://github.com/hjosugi/sql-dialect-fmt
    rev: v1.21.0
    hooks:
      - id: sql-dialect-fmt
```

## VS Code 拡張

`editors` の VS Code 拡張は Snowflake SQL のシンタックスハイライトに加えて**整形**にも対応しています。
`snowflake-sql` 用のフォーマッタを登録するので、**Format Document**・**Format Selection**・
`editor.formatOnSave` がそのまま使えます。Rust formatter を WebAssembly として
同梱し、すべてローカルで整形します（ネットワーク送信なし）。

```sh
./scripts/build-vscode-extension.sh
```

`editors/` で <kbd>F5</kbd> を押す（または生成した VSIX をインストールする）と、`.sql` ファイルで
**Format Document** が使えます。キーワードの大小、SELECT 項目の縦/自動配置、leading/trailing
comma、行幅、改行コード、エディタのインデント設定を指定できます。

## 開発

```sh
task test
task clippy
task vscode:build
task vscode:package
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
task fmt:check
```

### Formatter feature flags

基本 SQL formatter と VS Code 拡張を高速に反復できるよう、既定ビルドは Biome/Ruff を
含みません。埋め込み JavaScript/Python body 整形は明示的に有効化できます。

```sh
cargo test -p sql-dialect-fmt-formatter --features external-formatters
```

| feature | default | effect |
| --- | --- | --- |
| `external-formatters` | no | `embedded-javascript` と `embedded-python` を有効化 |
| `embedded-javascript` | no | `LANGUAGE JAVASCRIPT AS $$...$$` を Biome で整形 |
| `embedded-python` | no | `LANGUAGE PYTHON AS $$...$$` を Ruff で整形 |
| `embedded-brace-formatters` | no | 簡易 Java/Scala brace-aware formatter を明示的に有効化 |

## 状態

Snowflake は SELECT 一式・DML（INSERT/UPDATE/DELETE/MERGE）・COPY・主要 DDL/object DDL（Snowpipe の CREATE PIPE ... AS COPY INTO を含む）・Semantic View・CREATE PROCEDURE/FUNCTION（SQL/JavaScript/Python/Java/Scala body）までパース＋整形。非 SQL body の整形は opt-in で、通常は verbatim 保持します。Databricks は LATERAL VIEW、Delta DDL option、VERSION/TIMESTAMP AS OF、higher-order function lambda、SQL scripting block、backtick identifier を dialect mode でサポート。LSP/semantic tokens/hover、CLI、VS Code/WASM 拡張を active scope とし、Tree-sitter はソースを残したまま workspace/CI 外で保留します。詳細と計画は [ROADMAP.md](ROADMAP.md) を参照。

## クレート構成

| crate | 役割 |
| --- | --- |
| `sql-dialect-fmt-syntax` | `SyntaxKind`・キーワード認識・`rowan` 言語定義 |
| `sql-dialect-fmt-lexer` | 手書きロスレス Lexer |
| `sql-dialect-fmt-parser` | エラー回復で無停止のロスレス CST パーサ |
| `sql-dialect-fmt-formatter` | 汎用 Doc IR エンジン ＋ SQL 整形規則 |
| `sql-dialect-fmt-highlight` | トークン分類（シンタックスハイライト） |
| `sql-dialect-fmt-hover` | 型・手続き・タスクの hover テキスト |
| `sql-dialect-fmt-tree-sitter` | 保留中の Tree-sitter Rust binding（active workspace 外） |
| `sql-dialect-fmt-config` | `sql-dialect-fmt.toml` の共通モデルと探索（CLI / LSP 共用） |
| `sql-dialect-fmt-lsp` | Language Server（formatting / semanticTokens / 診断、stdio） |
| `sql-dialect-fmt-wasm` | VS Code 拡張に同梱する WebAssembly bridge |
| `sql-dialect-fmt` | CLI エントリポイント（crate path は `crates/sql-dialect-fmt-cli`） |

## ライセンス

0BSD. ほぼあらゆる目的で利用・複製・変更・配布できます。
