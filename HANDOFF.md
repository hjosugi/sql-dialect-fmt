# snow-fmt 引き継ぎ (HANDOFF) — 2026-06-21 夜

> このファイルは「翌朝きれいに再開する」ための引き継ぎメモです。前セッション（および並行ローカル
> セッション）が limit に達したため、**現状を緑のまま固定**し、次にやることを優先順位つきで残します。
> 関連: [README.md](README.md) / [ROADMAP.md](ROADMAP.md) / [GUIDE.md](GUIDE.md)（学習用・gitignore） /
> [docs/research/](docs/research/) / [spec/](spec/)（Snowflake 仕様トラッカー、cargo 対象外）。

## 0. いまの状態（検証済み・緑）
- `cargo build --workspace` … OK
- `cargo test --workspace` … **全テストバイナリ ok（157 passed / 0 failed）**。`snow-fmt-formatter`（Phase 3、コメント付与まで）追加済み。
- `cargo clippy --workspace --all-targets` … クリーン（本セッション確認）
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
| `snow-fmt-formatter` | CST→Doc IR（Wadler/Prettier 式 `group`/`indent`/`line`/`line_suffix`、自前エンジン）→幅対応プリンタ。SELECT 一式/JOIN/CTE/集合演算/CASE/ウィンドウ/semi-structured を整形。**コメント付与あり**（leading/trailing/dangling）。壊れた SQL は無変換。**idempotent**・トークン/コメント保存をテストで担保 | ✅ Phase 3 v2 |
| `snow-fmt-highlight` | CST/トークン分類（keyword/type/string/comment/operator/variable）を byte range 付きで。ロスレス検証 | ✅ 初期 |
| `snow-fmt-hover` | ホバー情報（**rich 化はこれから** — §4 参照） | 🚧 雛形 |
| `snow-fmt-tree-sitter` | エディタ用 tree-sitter grammar の Rust ラッパ（生成 C parser を build.rs でコンパイル） | 🚧 初期 |
| `snow-fmt-cli` | `--fixtures` 指定時のみ golden 変換を行う安全な bootstrap CLI | 🚧 初期 |
| `snow-fmt-encoding` | 文字コード/改行ユーティリティ | 🚧 |
| `snow-fmt-test-fixtures` | easy-test-cases を `include_str!` で内蔵（外部 `easy-test-cases/` 無しでも `cargo test` 通る） | ✅ |
| `snow-fmt-test-support` | テスト共有ユーティリティ | ✅ |

設計の真実の源は **rowan CST**。tree-sitter は競合させず、エディタ向けの寛容・高速な認識層という役割分担。

## 3. 翌朝の優先タスク（順番）
1. **パーサ高頻度ギャップ** — ✅ **本セッションで実装済み**（[tests/phase2b.rs](crates/snow-fmt-parser/tests/phase2b.rs)）:
   - ✅ `CASE [x] WHEN .. THEN .. ELSE .. END`（`CASE_EXPR`/`CASE_WHEN`）
   - ✅ `CAST(x AS t)` / `TRY_CAST(x AS t)` 関数形（`::` キャストと両対応）
   - ✅ セミ構造化パス `col:path.to.field`・`[idx]`・`::cast` 連鎖（`JSON_ACCESS`）
   - ✅ `VALUES (..),(..)`（文／サブクエリ／派生テーブル列別名 `AS v(c1,c2)` も）
   - ✅ **フロー/パイプ `->>` 実装済み**（公式は `->>`、`|>` ではない。docs で確認: statement を `->>` で連結、
     `$n` は直前 n 番目の結果を **FROM 句でのみ**参照）。parser: `statement` を `single_statement (->> single_statement)*`
     に拡張し `FLOW_STMT` に包む／`FROM $1` を NAME_REF として受理（[grammar.rs](crates/snow-fmt-parser/src/grammar.rs)、
     [tests/flow.rs](crates/snow-fmt-parser/tests/flow.rs)）。formatter: 連結を1段インデント＋各ステップ行頭 `->> `。
   - 実装メモ: 文法 [grammar.rs](crates/snow-fmt-parser/src/grammar.rs)、ノードは [kind.rs](crates/snow-fmt-syntax/src/kind.rs)
     の `__LAST` 直前に追加。キーワードを足したら [keyword.rs](crates/snow-fmt-syntax/src/keyword.rs) の match と
     KEYWORDS テストの両方を更新。各追加に網羅テスト。
2. **Phase 3: フォーマッタ** — ✅ **v1 実装済み**（`snow-fmt-formatter`）。自前 Doc IR エンジン
   （`group`/`indent`/`line`/`soft`/`hard`、break 伝播＋`fits`、`biome_formatter` 非依存）＋幅対応プリンタ
   ＋ CST→Doc の SQL ルール（[sql.rs](crates/snow-fmt-formatter/src/sql.rs)）。**idempotency** とトークン保存を
   コーパス（EASY_CASES + 厳選）で担保（[tests/corpus.rs](crates/snow-fmt-formatter/tests/corpus.rs)）。
   公開 API は `format` / `format_with(FormatOptions{line_width,indent_width,keyword_case})`。
   - ✅ **コメント付与 実装済み**（[comments.rs](crates/snow-fmt-formatter/src/comments.rs)）: Prettier/Ruff 流に
     各コメントを**ノード**へ leading/trailing/dangling で割当（`locate` は各ノードの *meaningful range*=非trivia
     先頭〜末尾で判定し、CST が trivia を次トークンの leading に置く問題を回避）。Doc IR に `LineSuffix`/`BreakParent`
     を追加（trailing は `line_suffix`+`break_parent` で行末へ）。`format_with` は**自己検証**（出力が valid SQL・
     全コメント保存・再フォーマットで不変）に失敗したら原文へフォールバック。コメント無しは高速パス。
   - ✅ **埋め込み言語（CREATE FUNCTION/PROCEDURE）実装済み**。parser: `CREATE [OR REPLACE] [mods]
     {FUNCTION|PROCEDURE} name(params) <RETURNS/LANGUAGE/options> AS <$$body$$|'body'>` →
     `CREATE_FUNCTION`/`PARAM_LIST`/`RETURNS_CLAUSE`/`LANGUAGE_CLAUSE`/`FUNC_OPTION`/`FUNC_BODY`
     （[grammar.rs](crates/snow-fmt-parser/src/grammar.rs)、[tests/create_function.rs](crates/snow-fmt-parser/tests/create_function.rs)）。
     formatter: ヘッダ＋各オプション行＋`AS body`。**`EmbeddedFormatter` trait（seam）**＋`format_with_embedded`
     で `$$` JS 本体を委譲（[embedded.rs](crates/snow-fmt-formatter/src/embedded.rs)）。コア `format()` は純粋（外部実行なし）。
     **CLI(`Profile::Full`) が `CliEmbeddedFormatter` で外部ツールにシェルアウト**（JS=`npx @biomejs/biome`、
     Python=ruff/black。未インストール時は verbatim）= 「biome 入っていれば使う」を実現。
     highlight: `LANGUAGE <x>` を検出して `$$` 本体を **JS injection 領域**（`Injection{language,range}`）として出力。
   - ✅ **tree-sitter 言語別 injection 実装済み**（[tree-sitter-snowflake/](tree-sitter-snowflake/)）: token grammar を最小限
     構造化（`create_statement`/`language_clause`、アンカーは `kw_create`/`kw_language` token＝`@keyword` 着色、`;`→`terminator`）。
     `injections.scm` が `(create_statement (language_clause name:(_) @injection.language) (dollar_string) @injection.content (#offset! …))`
     で LANGUAGE と `$$` 本体を相関させ言語別着色（`#offset!` で `$$` を除去）。parser.c 再生成は `npm exec --package tree-sitter-cli@0.26.9 -- tree-sitter generate`。
     Rust 検証: [tests/smoke.rs](crates/snow-fmt-tree-sitter/tests/smoke.rs)（JS 本体が javascript として注入、LANGUAGE 無しは非注入）。
   - ⏳ **残（次の増分、優先順）**:
     a. 型名・関数名のケーシング方針、未対応構文（PIVOT/UNPIVOT/FLATTEN 等）の専用ルール化（現状は verbatim フォールバック）。
     b. 複数行ブロックコメントの内部再インデント、dangling コメントの配置改善、`insta` スナップショット、`format-dev` 類の類似度コーパス・ゲート。
   - 注: SQL は SELECT リスト等に**末尾カンマを許さない**ため、JS/Python の magic trailing comma は採用せず
     （採用すると無効 SQL を生む）。折返しは幅駆動のみ。設計根拠は [docs/research/prior-art.md](docs/research/prior-art.md)。
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
