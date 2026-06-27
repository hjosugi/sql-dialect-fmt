# sql-dialect-fmt 引き継ぎ (HANDOFF) — 2026-06-27

> 「きれいに再開する」ための引き継ぎメモ。**現状を緑のまま固定**し、次タスクを優先順位つきで残す。
> 関連: [README.md](README.md) / [ROADMAP.md](ROADMAP.md) / [docs/research/](docs/research/) /
> [spec/](spec/)（Snowflake 仕様トラッカー、cargo 対象外）。

## 0. いまの状態（検証済み・緑）
- `cargo build --workspace` / `cargo test --workspace` … **全テストバイナリ ok（失敗 0）**
- `cargo clippy --workspace --all-targets` … クリーン
- `cargo fmt --all --check` … OK
- **再開時はまず上記を再実行して緑を確認してから着手。**

## 0.5 このセッションの成果（Phase 3 → 4/6/7/8 横断）
**フォーマッタ `sql-dialect-fmt-formatter` をゼロから構築**し、CLI まで実用化した。

- **Doc IR エンジン**（`Text`/`Line(Soft/Space/Hard)`/`Group`/`Indent`/`LineSuffix`/`BreakParent`/`group_expanded`）＋幅対応プリンタ（Wadler→Prettier 系 `fits`）。
- **SQL 整形**: SELECT パイプライン、JOIN/ORDER BY/GROUP BY 構造化、CASE、サブクエリ/CTE、集合演算、**magic trailing comma**（看板機能）、**本物のコメント付与**（leading/trailing/dangling）。
- **構文拡張（パーサ）**: 集約 `DISTINCT`、`WITHIN GROUP`、`PIVOT/UNPIVOT`、`GROUPING SETS/CUBE/ROLLUP`、`LATERAL FLATTEN`/テーブル関数/名前付き引数、`MATCH_RECOGNIZE`、`ASOF JOIN`、time travel `AT/BEFORE`、`IS [NOT] DISTINCT FROM`、`FROM VALUES`、`WITH` を query primary に。
- **DML**: `INSERT`（単一/`OVERWRITE`/`ALL`/`FIRST`）, `UPDATE`, `DELETE`, `MERGE`。
- **DDL**: `CREATE TABLE/VIEW/CTAS`, `DROP`, `ALTER`(寛容), masking/row access policy、tag、`CREATE PROCEDURE/FUNCTION` 骨格（`RETURNS TABLE (...)` と `RETURNS ... NOT NULL` を含め、`LANGUAGE SQL` の `$$…$$` ボディは自己再帰整形、`LANGUAGE JAVASCRIPT` は Biome、`LANGUAGE PYTHON` は Ruff、Java/Scala は brace-aware formatter 委譲。quoted body は verbatim）。
- **COPY INTO**（ロード/アンロード、ステージパス verbatim、option key の key-position 大文字化）。
- **CLI `sql-dialect-fmt`**: `--write`/`--check`/stdin、複数ファイル/ディレクトリ再帰、`sql-dialect-fmt.toml` discovery、エンコーディング保持（v0.1.0、`cargo install` 可）。
- **診断品質**: lexer/parser error span（token 全体、EOF zero-width）、人間向け `SyntaxKind::describe`、LSP diagnostics に lexer error も反映。
- **無破壊・べき等・トークン/コメント保存**を内蔵 easy corpus 全件 + `proptest`（Unicode/ASCII/token-salad）で機械ガード。
- **コーパス clean パース 0 → 38 / 77**（残りは安全に無変更パススルー）。

### 既知の技術的負債（次のリファクタ対象）
- ✅ ~~`Lowerer` に `lower_insert/merge/copy/create…` の特殊ケースが増殖~~ → 単一の `lower_clausal` に統合済み。
- ✅ ~~contextual keyword が IDENT 扱いで小文字のまま~~ → `CONTEXTUAL_KEYWORD` ソフトキーワードタグ（`bump_as`）で大文字化＋`KEYWORD (…)` スペーシング統一。予約はしない（識別子としてはそのまま）。
- ✅ ~~`text_width` は char 数（CJK 幅が厳密でない）~~ → East Asian Width（TR11）で全角 2 幅に。
- ✅ ~~`MATCH_RECOGNIZE` が未構造化＝長い1行（measures/pattern/define が小文字）~~ → 本文を構造化（PARTITION/ORDER/MEASURES/PER MATCH/AFTER MATCH SKIP/PATTERN/SUBSET/DEFINE を1行ずつ）、contextual 大文字化、`PATTERN(...)` 本体は verbatim。ついでに `first()/last()/left()` 等の予約語関数呼び出しを解禁。
- 残る balanced-paren（`SAMPLE`/`TABLESAMPLE`/time travel `AT|BEFORE (...)`/COPY のステージ）は未構造化のインライン（短いので実害小）。
- ✅ ~~`INSERT INTO t(cols)` の `(` 前スペース不統一~~ → 列名リスト（`COLUMN_LIST`）は常に前スペース（`INSERT INTO t (a, b)`／`AS t (c1, c2)`／`USING (a, b)`）、関数呼び出し `ARG_LIST` は密着のまま。`CREATE TABLE t (…)` と一致。
- ✅ ~~`insta` スナップショット未導入~~ → 複数行のゴールデンテストを `insta::assert_snapshot!`（インライン）に移行。期待値はテスト内に残りつつ自動更新可。更新は `cargo insta test --accept`（要 `cargo install cargo-insta`）、検証は通常の `cargo test` で可。
- ✅ ~~COPY/object DDL option key が小文字のまま残る~~ → `lower_option_node` で key position のみ認識して大文字化。値・識別子・stage path は不変。
- ✅ ~~parser/lexer diagnostics が単点 offset + Debug 名~~ → byte span + human-readable message に刷新。LSP range も token 全体へ。
- ✅ ~~property/fuzz 系の panic-safety ゲート不足~~ → parser/formatter の `proptest` harness を追加し、既知の formatter 非べき等ケース（未終端 token、行コメントのセミコロン後流出）も本体側で修正。
- ※「コメントを含む文は丸ごと verbatim」は**誤り**だった: leading/trailing/inline コメントは通常経路で整形済み。verbatim はトークンに付与できない稀なコメントだけの安全網。

## 1. ゴール（ユーザー指示の要約）
- **最高の Snowflake SQL 解析器**を作る。最新の論文・実装も参照し「完璧な解析」を目指す。
- **すべてのクエリを最終的にパース**（まず最頻出クエリを完全対応）。高速に動くこと。
- **rich な hover** も可能なら出せるように（`sql-dialect-fmt-hover` を充実させる）。
- 例外的ケース（Unicode 例: 長芋、長い入力、改行差分 LF/CRLF/CR/混在、壊れた SQL）を網羅的にテスト。
- Snowflake 最新仕様を継続追跡（`spec/`、ローカル SQLite、cargo build には入れない、修正は手動でよい）。

## 2. クレート構成と役割
| crate | 役割 | 状態 |
|---|---|---|
| `sql-dialect-fmt-syntax` | `SyntaxKind`・`keyword_kind`・`T!`・rowan `Language` | ✅ 中核 |
| `sql-dialect-fmt-lexer` | ロスレス手書きレキサ（`->>`=FLOW_PIPE, `|>`, `::`, `$$..$$`, コメント3種, エスケープ） | ✅ 中核 |
| `sql-dialect-fmt-parser` | イベント方式パーサ→rowan CST、Pratt 式、SELECT 一式/DML/DDL/プロシージャ骨格/Snowflake 拡張。**決して失敗しない**・ロスレス・span 付き診断 | ✅ Phase 1–8 部分 |
| `sql-dialect-fmt-formatter` | 汎用 Doc IR エンジン ＋ SQL 整形規則（上記 §0.5）。べき等・無破壊（lexer/parser error はパススルー、未配置コメントは文単位 verbatim） | ✅ Phase 3 + 実用 |
| `sql-dialect-fmt-highlight` | CST/トークン分類（keyword/type/string/comment/operator/variable）を byte range 付きで。ロスレス検証 | ✅ 初期 |
| `sql-dialect-fmt-hover` | ホバー情報（**rich 化はこれから** — §4 参照） | 🚧 雛形 |
| `sql-dialect-fmt-tree-sitter` | エディタ用 tree-sitter grammar の Rust ラッパ（生成 C parser を build.rs でコンパイル、statement/folds、軽量 expression、context-aware injections まで） | 🚧 初期+ |
| `sql-dialect-fmt` | 実用 CLI（`--write`/`--check`/stdin、複数ファイル/ディレクトリ、`sql-dialect-fmt.toml`、`--dialect snowflake|databricks`、エンコーディング保持）。v0.1.0 | ✅ |
| `sql-dialect-fmt-encoding` | 文字コード/改行ユーティリティ | 🚧 |
| `sql-dialect-fmt-test-fixtures` | easy-test-cases を `include_str!` で内蔵（外部 `easy-test-cases/` 無しでも `cargo test` 通る） | ✅ |
| `sql-dialect-fmt-test-support` | テスト共有ユーティリティ | ✅ |

設計の真実の源は **rowan CST**。tree-sitter は競合させず、エディタ向けの寛容・高速な認識層という役割分担。

## 3. 次の優先タスク（順番）
1. **埋め込み言語の次段**: `$$…$$` body の言語判定は SQL 自己再帰 + JS Biome + Python Ruff + Java/Scala brace-aware まで完了。次は quoted body の扱い、Java/Scala の限界ケース拡張。
2. **DDL の残り**: Semantic View、細かい object option の構造化。新しめの仕様は Snowflake 公式 docs で確認してから入れる。
3. **rich hover / spec 連携**: §4 の通り、まず keyword/function hover を `spec/seed/features.json` 由来にする。
4. **editor 周辺**: tree-sitter indents、VS Code 拡張。
5. **仕上げ**: `rayon` 並列、外部大規模コーパス、crates.io/GitHub Release。

## 4. rich hover の設計案
- LSP `textDocument/hover` を `sql-dialect-fmt-hover` で実装。CST 上の位置 → 最小ノードを特定し、種別ごとに内容を返す:
  - 関数呼び出し: シグネチャ・説明（**知識源は `spec/` の features.json / SQLite を流用**できる。関数表を spec に追加）。
  - キーワード: 構文スニペット（`spec/seed/features.json` の `syntax` フィールドが使える）。
  - 識別子: 修飾名・別名解決（将来）。型キャスト先・semi-structured パスの説明。
- まず「キーワード/関数のホバー（spec 由来の syntax + status + doc URL）」から始めると、spec トラッカーと
  自然に連携して rich になる。LSP 本体（`sql-dialect-fmt-lsp`）は別 crate で後追い。

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
cargo test -p sql-dialect-fmt-syntax --features rowan
cargo fmt --all
# フォーマッタのゴールデンは insta インラインスナップショット。整形を意図的に変えたら:
cargo insta test --accept -p sql-dialect-fmt-formatter   # 要: cargo install cargo-insta
```
