# snow-fmt

[![CI](https://github.com/hjosugi/snow-fmt/actions/workflows/ci.yml/badge.svg)](https://github.com/hjosugi/snow-fmt/actions/workflows/ci.yml)

Snowflake SQL のフォーマッタ＋シンタックスハイライタ（Rust 製）。`gofmt` / Prettier / Biome 流の opinionated・ほぼ設定なしの整形を目指します。

整形は **無破壊・べき等** を機械的に保証します（パースできない入力は無変更で素通し、整形しても有意トークンとコメントは保存、`format(format(x)) == format(x)`）。

## 使い方

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

## 状態

SELECT 一式・DML（INSERT/UPDATE/DELETE/MERGE）・DDL（CREATE TABLE/VIEW/CTAS, DROP, ALTER）・CREATE PROCEDURE/FUNCTION の骨格までパース＋整形。看板機能は **magic trailing comma**。詳細と計画は [ROADMAP.md](ROADMAP.md) を参照。

## クレート構成

| crate | 役割 |
| --- | --- |
| `snow-fmt-syntax` | `SyntaxKind`・キーワード認識・`rowan` 言語定義 |
| `snow-fmt-lexer` | 手書きロスレス Lexer |
| `snow-fmt-parser` | エラー回復で無停止のロスレス CST パーサ |
| `snow-fmt-formatter` | 汎用 Doc IR エンジン ＋ SQL 整形規則 |
| `snow-fmt-highlight` | トークン分類（シンタックスハイライト） |
| `snow-fmt-hover` | 型・手続き・タスクの hover テキスト |
| `snow-fmt-tree-sitter` | 同梱 Tree-sitter grammar の Rust バインディング |
| `snow-fmt-cli` | CLI エントリポイント |

## ライセンス

MIT OR Apache-2.0
