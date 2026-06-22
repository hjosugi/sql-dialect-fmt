# snow-fmt 引き継ぎ (HANDOFF) — 2026-06-21 夜

> このファイルは「翌朝きれいに再開する」ための引き継ぎメモです。前セッション（および並行ローカル
> セッション）が limit に達したため、**現状を緑のまま固定**し、次にやることを優先順位つきで残します。
> 関連: [README.md](README.md) / [ROADMAP.md](ROADMAP.md) / [GUIDE.md](GUIDE.md)（学習用・gitignore） /
> [docs/research/](docs/research/) / [spec/](spec/)（Snowflake 仕様トラッカー、cargo 対象外）。

## 0. いまの状態（検証済み・緑）
- `cargo build --workspace` … OK
- `cargo test --workspace` … **全テストバイナリ ok（失敗 0）**
- `cargo clippy --workspace --all-targets` … クリーン（前セッション報告 + 本セッション確認）
- `cargo test -p snow-fmt-syntax --features rowan` … OK
- `tree-sitter-snowflake/` … `grammar.js` + `src/parser.c`（生成済み）+ `queries/` あり
- **触ると壊れうるので、再開時はまず上記を再実行して緑を確認してから着手すること。**

## 1. ゴール（ユーザー指示の要約）
- **最高の Snowflake SQL 解析器**を作る。最新の論文・実装も参照し「完璧な解析」を目指す。
- **すべてのクエリを最終的にパース**（まず最頻出クエリを完全対応）。高速に動くこと。
- **rich な hover** も可能なら出せるように（`snow-fmt-hover` を充実させる）。
- 例外的ケース（Unicode 例: 長芋、長い入力、改行差分 LF/CRLF/CR/混在、壊れた SQL）を網羅的にテスト。
- Snowflake 最新仕様を継続追跡（`spec/`、ローカル SQLite、cargo build には入れない、修正は手動でよい）。

## 2. クレート構成と役割
| crate | 役割 | 状態 |
|---|---|---|
| `snow-fmt-syntax` | `SyntaxKind`・`keyword_kind`・`T!`・rowan `Language` | ✅ 中核 |
| `snow-fmt-lexer` | ロスレス手書きレキサ（`->>`=FLOW_PIPE, `|>`, `::`, `$$..$$`, コメント3種, エスケープ） | ✅ 中核 |
| `snow-fmt-parser` | イベント方式パーサ→rowan CST、Pratt 式、SELECT 一式/JOIN/サブクエリ/集合演算/CTE/述語/ウィンドウ。**決して失敗しない**・ロスレス | ✅ Phase 1–2 |
| `snow-fmt-formatter` | 汎用 Doc IR エンジン（`Text`/`Line`/`Group`/`Indent`＋幅対応プリンタ）＋ `SELECT` 整形規則。べき等・無破壊（パース失敗入力はパススルー、コメント/`ERROR` は verbatim） | 🚧 Phase 3 初期 |
| `snow-fmt-highlight` | CST/トークン分類（keyword/type/string/comment/operator/variable）を byte range 付きで。ロスレス検証 | ✅ 初期 |
| `snow-fmt-hover` | ホバー情報（**rich 化はこれから** — §4 参照） | 🚧 雛形 |
| `snow-fmt-tree-sitter` | エディタ用 tree-sitter grammar の Rust ラッパ（生成 C parser を build.rs でコンパイル） | 🚧 初期 |
| `snow-fmt-cli` | `--fixtures` 指定時のみ golden 変換を行う安全な bootstrap CLI | 🚧 初期 |
| `snow-fmt-encoding` | 文字コード/改行ユーティリティ | 🚧 |
| `snow-fmt-test-fixtures` | easy-test-cases を `include_str!` で内蔵（外部 `easy-test-cases/` 無しでも `cargo test` 通る） | ✅ |
| `snow-fmt-test-support` | テスト共有ユーティリティ | ✅ |

設計の真実の源は **rowan CST**。tree-sitter は競合させず、エディタ向けの寛容・高速な認識層という役割分担。

## 3. 翌朝の優先タスク（順番）
1. **パーサの高頻度ギャップを埋める**（ユーザー明示・未対応。`spec/` でも `todo`）:
   - `CASE [x] WHEN .. THEN .. ELSE .. END`（最頻出。`primary()` に追加。ノード `CASE_EXPR`/`CASE_WHEN`）
   - `CAST(x AS t)` / `TRY_CAST(x AS t)` 関数形（`::` キャストは対応済み）
   - セミ構造化パス `col:path.to.field`（`expr_bp` の後置に `COLON` を追加。ノード `JSON_ACCESS`）
   - `VALUES (..),(..)`（`query_primary` と文として。ノード `VALUES_CLAUSE`/`VALUES_ROW`）
   - パイプ構文 `|>`（**最新の演算子一覧を docs で要確認** → §5）
   - 実装メモ: 文法は [crates/snow-fmt-parser/src/grammar.rs](crates/snow-fmt-parser/src/grammar.rs)、ノード追加は
     [crates/snow-fmt-syntax/src/kind.rs](crates/snow-fmt-syntax/src/kind.rs)（`__LAST` の直前に追加 → キーワードを足したら
     [keyword.rs](crates/snow-fmt-syntax/src/keyword.rs) の match と KEYWORDS テストの両方を更新）。各追加に網羅テスト。
2. **Phase 3: フォーマッタ（最大の未実装中核）**。自前の汎用 Doc IR エンジン（biome/ruff の `FormatElement`
   を模倣、`biome_formatter` 非依存）＋ 幅対応プリンタ。SELECT 整形・magic trailing comma・コメント付与。
   idempotency（`format(format(x))==format(x)`）と reparse 等価のテスト。詳細は [docs/research/prior-art.md](docs/research/prior-art.md)。
3. **rich hover**（§4）。
4. tree-sitter の corpus テストと `queries/highlights.scm` 拡充。
5. `spec/` を docs ソースで更新し直す（現状はキュレーションのシード）。

## 4. rich hover の設計案
- LSP `textDocument/hover` を `snow-fmt-hover` で実装。CST 上の位置 → 最小ノードを特定し、種別ごとに内容を返す:
  - 関数呼び出し: シグネチャ・説明（**知識源は `spec/` の features.json / SQLite を流用**できる。関数表を spec に追加）。
  - キーワード: 構文スニペット（`spec/seed/features.json` の `syntax` フィールドが使える）。
  - 識別子: 修飾名・別名解決（将来）。型キャスト先・semi-structured パスの説明。
- まず「キーワード/関数のホバー（spec 由来の syntax + status + doc URL）」から始めると、spec トラッカーと
  自然に連携して rich になる。LSP 本体（`snow-fmt-lsp`）は別 crate で後追い。

## 5. 「完璧な解析」のための参照（最新研究・実装）
- 回復的構文解析: matklad *Resilient LL Parsing*（2023）/ *Simple but Powerful Pratt Parsing*（2020）。
- エラー回復: Diekmann & Tratt *Don't Panic! Better, Fewer, Syntax Errors for LR Parsers*（CPCT+, 2020）。
- 増分解析（エディタ/tree-sitter の理論的背景）: Wagner & Graham *Efficient and Flexible Incremental Parsing*（1998）。
- Pretty-printing: Wadler *A prettier printer*（2003）/ Bernardy *A Pretty But Not Greedy Printer*（2017）/ Prettier の Doc アルゴリズム。
- CST/コメント: rust-analyzer + rowan、Biome、Prettier の comment attachment。
- Snowflake 一次情報: flow 演算子 `->>` <https://docs.snowflake.com/en/sql-reference/operators-flow> 、
  release notes（例 9.13）<https://docs.snowflake.com/en/release-notes/2025/9_13> 、pipe `|>` の最新演算子一覧（要確認）。
- 既存実装の調査: [docs/research/prior-art.md](docs/research/prior-art.md) と
  [docs/research/snowflake-github-prior-art.md](docs/research/snowflake-github-prior-art.md)（SQLFluff/SQLGlot/tree-sitter-sql/sql-formatter 等）。

## 6. Snowflake 仕様トラッカー（`spec/`、cargo 対象外）
```sh
python3 spec/snowflake_spec.py coverage   # parsed/total を確認し、次の着手対象を選ぶ
python3 spec/snowflake_spec.py import spec/seed/features.json --note "YYYY-MM refresh"  # 差分を記録
python3 spec/snowflake_spec.py changes    # 変更履歴
```
シード時点: 79 機能中 **36 parse / 42 todo / 1 partial**。`features.json` を編集→`import` で変化を追跡。

## 7. 並行作業の注意（重要）
- このリポジトリは**複数エージェントが同時編集**しうる（IDE のローカルセッション + Claude）。同一ファイルの
  同時編集は破壊的。**1 度に 1 エージェント**で。再開時はまず §0 の緑確認から。
- バックグラウンド調査エージェントは過去にスコープを逸脱した（[[feedback-agent-scope]]）。委譲する場合は
  read-only か「単一ファイル Write のみ」に厳密制約し、完了後にビルド/テストで検証する。

## 8. 検証コマンド（再開時にまず実行）
```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo test -p snow-fmt-syntax --features rowan
cargo fmt --all
```
