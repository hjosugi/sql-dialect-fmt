# snow-fmt

**Snowflake SQL の最高品質フォーマッタ＋シンタックスハイライタ（Rust 製・高速）。**

`gofmt` / Prettier / **Biome** の思想を Snowflake SQL に持ち込みます。特に既存ツールが弱い2点を第一級で扱うことを目指します:

- **パイプ構文 `|>`**（GoogleSQL 由来、Snowflake 採用）
- **手続き内の埋め込み言語**（`$$ ... $$` の中の JavaScript / Python / SQL）。JavaScript は Biome の整形器を組み込んで最高品質で整形。

> 言語ツールを初めて作る方へ: 仕組みの解説は **[GUIDE.md](GUIDE.md)** にまとめてあります（Lexer/Parser/CST/フォーマッタ/ハイライトを基礎から）。

## 状態

開発初期。**Phase 0（基盤）完了** — ロスレス Lexer ＋構文種別 ＋テスト基盤まで。全体計画は **[ROADMAP.md](ROADMAP.md)** を参照。

| 機能 | 状態 |
| --- | --- |
| ロスレス Lexer（`\|>`, `::`, `$$…$$`, コメント3種, エスケープ） | ✅ |
| `SyntaxKind` ＋ `rowan` 連携 | ✅ |
| Parser / CST（Pratt 式＋SELECT、ロスレス、エラー回復で無停止） | ✅ Phase 1 |
| Formatter（Doc IR） | ⏳ Phase 3 |
| 埋め込み JS 整形（Biome） | ⏳ Phase 8 |
| ハイライト / LSP | ⏳ Phase 9 |

## クレート構成

```
snow-fmt/
├── crates/
│   ├── snow-fmt-syntax/   SyntaxKind・キーワード認識・rowan 言語定義
│   │   └── src/{kind,keyword,lang}.rs
│   └── snow-fmt-lexer/    手書きロスレス Lexer
│       ├── src/{token,lexer}.rs
│       └── tests/corpus.rs   網羅・ファズ・不変条件テスト
├── GUIDE.md               フォーマッタ/言語解析ツールの作り方（解説）
├── ROADMAP.md             段階的カバレッジ計画
└── docs/research/         既存プロジェクト調査（prior-art.md ほか）
```

将来追加予定: `snow-fmt-parser` / `snow-fmt-formatter` / `snow-fmt-highlight` / `snow-fmt-cli` / `snow-fmt-lsp`。

## ビルドとテスト

```sh
cargo test --workspace                       # 全テスト
cargo test -p snow-fmt-syntax --features rowan   # rowan 連携も検証
cargo clippy --workspace --all-targets       # lint
cargo fmt --all                              # 整形
```

Rust 安定版（edition 2021）。標準では依存ゼロ（`rowan` は `rowan` フィーチャ有効時のみ）。

## 設計上の決定

- **言語**: Rust（高速・Biome エコシステム再利用のため）。
- **構文木**: `rowan` によるロスレス CST（rust-analyzer と同方式）。
- **フォーマッタ IR**: 自前の汎用 Doc エンジン（biome/ruff の `FormatElement` を模倣、`biome_formatter` 非依存）。SQL 規則は別レイヤに分離。**magic trailing comma** を看板機能に。
- **埋め込み JS**: `$$…$$` 本体のみ Biome の `biome_js_formatter` で整形し再インデント（解析不能時は verbatim）。
- **ハイライト**: 自前パーサを真実の源に、LSP セマンティックトークン → TextMate →（将来）tree-sitter。
- **スタイル**: gofmt / zig fmt 流の opinionated・ほぼ設定なし（`line-length` 程度）。
- **進め方**: 最頻出の構文から段階的にカバレッジ拡大。設計の根拠は [docs/research/prior-art.md](docs/research/prior-art.md)、計画は [ROADMAP.md](ROADMAP.md)。

## ライセンス

MIT OR Apache-2.0
