<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt

[![CI](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/ci.yml/badge.svg)](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/ci.yml)

Snowflake SQL と Databricks SQL のフォーマッタ＋シンタックスハイライタ（Rust 製）。`gofmt` / Prettier / Biome 流の opinionated・ほぼ設定なしの整形を目指します。

整形は **無破壊・べき等** を機械的に保証します（パースできない入力は無変更で素通し、整形しても有意トークンとコメントは保存、`format(format(x)) == format(x)`）。

## インストール

```sh
# crates.io から
cargo install sql-dialect-fmt --version 1.13.0 --locked

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
docker run --rm -v "$PWD:/work" -w /work ghcr.io/hjosugi/sql-dialect-fmt:1.13.0 --check .
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

# オプション: --dialect snowflake|databricks / --line-width N（既定100、1以上） / --indent-width N（既定4、1以上） / --no-uppercase
```

pre-commit 利用者は次の設定で `--write` または `--check` を使えます。

```yaml
repos:
  - repo: https://github.com/hjosugi/sql-dialect-fmt
    rev: v1.13.0
    hooks:
      - id: sql-dialect-fmt
```

## Snowsight / Databricks Chrome 拡張

ブラウザ上で使う Chrome 拡張を `extensions/chrome` に置いています。Rust formatter を
WebAssembly にして同梱するので、ローカルサーバは不要です。

```sh
./scripts/build-chrome-extension.sh
```

その後、Chrome の `chrome://extensions` で Developer mode を有効にし、`extensions/chrome`
を Load unpacked してください。Snowsight または Databricks の SQL editor にフォーカスして、右下の
`sql-dialect-fmt` ボタン、拡張アイコン、または `Alt+Shift+F` で整形できます。

Release 用の Chrome zip と VS Code VSIX はまとめて作れます。

```sh
./scripts/package-extensions.sh
```

## 開発

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo fmt --all --check
```

### Formatter feature flags

`sql-dialect-fmt-formatter` は既定で `external-formatters` を有効にし、埋め込み
JavaScript/Python body をそれぞれ Biome/Ruff で整形します。最小ビルドでは
Biome/Ruff 依存を外せます。

```sh
cargo test -p sql-dialect-fmt-formatter --no-default-features
```

| feature | default | effect |
| --- | --- | --- |
| `external-formatters` | yes | `embedded-javascript` と `embedded-python` を有効化 |
| `embedded-javascript` | yes | `LANGUAGE JAVASCRIPT AS $$...$$` を Biome で整形 |
| `embedded-python` | yes | `LANGUAGE PYTHON AS $$...$$` を Ruff で整形 |
| `embedded-brace-formatters` | no | 簡易 Java/Scala brace-aware formatter を明示的に有効化 |

## 状態

Snowflake は SELECT 一式・DML（INSERT/UPDATE/DELETE/MERGE）・COPY・主要 DDL/object DDL・Semantic View・CREATE PROCEDURE/FUNCTION（SQL/JavaScript/Python/Java/Scala body）までパース＋整形。JavaScript/Python body formatting は既定で有効、Java/Scala body formatting は `embedded-brace-formatters` で opt-in し、通常は verbatim 保持します。Databricks は LATERAL VIEW、Delta DDL option、VERSION/TIMESTAMP AS OF、higher-order function lambda、SQL scripting block、backtick identifier を dialect mode でサポート。LSP/semantic tokens/hover、Tree-sitter grammar、CLI、Snowsight/Databricks 用 Chrome/WASM 拡張も入っています。看板機能は **magic trailing comma**。詳細と計画は [ROADMAP.md](ROADMAP.md) を参照。

## クレート構成

| crate | 役割 |
| --- | --- |
| `sql-dialect-fmt-syntax` | `SyntaxKind`・キーワード認識・`rowan` 言語定義 |
| `sql-dialect-fmt-lexer` | 手書きロスレス Lexer |
| `sql-dialect-fmt-parser` | エラー回復で無停止のロスレス CST パーサ |
| `sql-dialect-fmt-formatter` | 汎用 Doc IR エンジン ＋ SQL 整形規則 |
| `sql-dialect-fmt-highlight` | トークン分類（シンタックスハイライト） |
| `sql-dialect-fmt-hover` | 型・手続き・タスクの hover テキスト |
| `sql-dialect-fmt-tree-sitter` | 同梱 Tree-sitter grammar の Rust バインディング |
| `sql-dialect-fmt-lsp` | Language Server（formatting / semanticTokens / 診断、stdio） |
| `sql-dialect-fmt-wasm` | Chrome 拡張向けの WebAssembly bridge |
| `sql-dialect-fmt` | CLI エントリポイント（crate path は `crates/sql-dialect-fmt-cli`） |

## ライセンス

0BSD. ほぼあらゆる目的で利用・複製・変更・配布できます。
