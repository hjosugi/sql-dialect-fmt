# snow-fmt

[![CI](https://github.com/hjosugi/snow-fmt/actions/workflows/ci.yml/badge.svg)](https://github.com/hjosugi/snow-fmt/actions/workflows/ci.yml)

**Snowflake SQL の最高品質フォーマッタ＋シンタックスハイライタ（Rust 製・高速）。**

`gofmt` / Prettier / **Biome** の思想を Snowflake SQL に持ち込みます。特に既存ツールが弱い2点を第一級で扱うことを目指します:

- **フロー/パイプ構文 `->>`**（Snowflake 公式。`|>` は互換トークンとして保持）
- **手続き内の埋め込み言語**（現行 Snowflake は `$$ ... $$`、将来の delimiter 変更に備えた table-driven lexer）。JavaScript は Biome の整形器を組み込んで最高品質で整形。

> 言語ツールを初めて作る方へ: 仕組みの解説は **[GUIDE.md](GUIDE.md)** にまとめてあります（Lexer/Parser/CST/フォーマッタ/ハイライトを基礎から）。

## すぐ試す

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

Tree-sitter grammar を触る場合:

```sh
cd tree-sitter-snowflake
npm exec --package tree-sitter-cli@0.26.9 -- tree-sitter generate
npm exec --package tree-sitter-cli@0.26.9 -- tree-sitter test
```

## 状態

開発中。**Phase 0–3 まで実装** — ロスレス Lexer ＋ CST パーサ ＋ フォーマッタ v1。全体計画は **[ROADMAP.md](ROADMAP.md)** を参照。

| 機能 | 状態 |
| --- | --- |
| ロスレス Lexer（`->>`, `\|>`, `::`, `$$…$$`, コメント3種, エスケープ） | ✅ |
| `SyntaxKind` ＋ `rowan` 連携 | ✅ |
| Parser / CST（Pratt 式＋SELECT、ロスレス、エラー回復で無停止） | ✅ Phase 1–2 |
| Formatter（Doc IR、`snow-fmt-formatter`、コメント付与あり） | ✅ Phase 3（壊れた SQL は無変換） |
| 埋め込み JS 整形（Biome） | ⏳ Phase 8 |
| ハイライト / Hover / LSP / Tree-sitter | ✅ lexical highlight + hover + Tree-sitter grammar / ⏳ LSP |

## クレート構成

```
snow-fmt/
├── crates/
│   ├── snow-fmt-syntax/   SyntaxKind・キーワード認識・rowan 言語定義
│   │   └── src/{kind,keyword,lang}.rs
│   ├── snow-fmt-encoding/ UTF-8 / BOM / UTF-16 / opaque bytes boundary
│   ├── snow-fmt-lexer/    手書きロスレス Lexer
│   │   ├── src/{token,lexer}.rs
│   │   └── tests/corpus.rs   網羅・ファズ・不変条件テスト
│   ├── snow-fmt-parser/   rowan CST parser
│   ├── snow-fmt-formatter/    CST→Doc IR の幅対応プリンタ（フォーマッタ）
│   ├── snow-fmt-highlight/ Lexical highlight token classification
│   ├── snow-fmt-hover/    LSP/editor-ready hover text for types/procedures/tasks
│   ├── snow-fmt-tree-sitter/ Rust bindings for the bundled Tree-sitter grammar
│   ├── snow-fmt-cli/       CLI entry point（fixture golden bootstrap）
│   └── snow-fmt-test-fixtures/ Embedded golden fixtures for cargo test
├── tree-sitter-snowflake/ Tree-sitter grammar package + highlight queries
├── GUIDE.md               フォーマッタ/言語解析ツールの作り方（解説）
├── ROADMAP.md             段階的カバレッジ計画
└── docs/research/         既存プロジェクト調査（prior-art.md ほか）
```

将来追加予定: `snow-fmt-lsp`。

## 参加する

はじめて触るなら、まず [CONTRIBUTING.md](CONTRIBUTING.md) と [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) を読むのがおすすめです。
テスト方針は [docs/TESTING.md](docs/TESTING.md) にまとめています。
セキュリティ報告は [SECURITY.md](SECURITY.md)、行動規範は [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) を参照してください。

小さく始めやすい変更:

- Snowflake キーワード・型・hover の説明を増やす
- `crates/snow-fmt-test-fixtures` に小さな SQL fixture を追加する
- Tree-sitter query の capture を改善する
- Parser の recovery test を追加する

## 設計上の決定

- **言語**: Rust（高速・Biome エコシステム再利用のため）。
- **構文木**: `rowan` によるロスレス CST（rust-analyzer と同方式）。
- **フォーマッタ IR**: 自前の汎用 Doc エンジン（biome/ruff の `FormatElement` を模倣、`biome_formatter` 非依存、crate `snow-fmt-formatter`）。SQL 規則は別レイヤに分離。折返しは**幅駆動**（SQL は末尾カンマ不可のため magic trailing comma は不採用）。
- **埋め込み JS**: delimiter-aware body token（現行 Snowflake は `$$…$$`）の本体のみ Biome の `biome_js_formatter` で整形し再インデント（解析不能時は verbatim）。
- **ハイライト**: 自前パーサを真実の源に、LSP セマンティックトークンへ拡張。エディタ向け baseline として Tree-sitter grammar / queries を同梱。
- **スタイル**: gofmt / zig fmt 流の opinionated・ほぼ設定なし（`line-length` 程度）。
- **進め方**: 最頻出の構文から段階的にカバレッジ拡大。設計の根拠は [docs/research/prior-art.md](docs/research/prior-art.md) と [docs/research/snowflake-github-prior-art.md](docs/research/snowflake-github-prior-art.md)、計画は [ROADMAP.md](ROADMAP.md)。

## ライセンス

MIT OR Apache-2.0
