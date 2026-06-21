# snow-fmt 対応ロードマップ（段階的カバレッジ）

方針: **一気に全部やらない。** 実コードでの頻度が高く価値の大きい領域から順に、フェーズ単位で文法カバレッジを広げる。各フェーズは「パースできる → 整形できる → テストが緑」で完了とする。

> ⚠️ **最新仕様の検証について**: Snowflake は更新が速い。公式のフロー/パイプ演算子は `->>`（`|>` は互換トークンとして保持）、Cortex/AISQL 関数名、Dynamic Tables などの「新しめ」の項目は、実装直前に必ず公式ドキュメント（https://docs.snowflake.com/ ）で裏取りすること。本ロードマップの該当箇所には 🔎 を付けた。並行して走らせた既存ツール調査の結果は [docs/research/prior-art.md](docs/research/prior-art.md) に出力される（アーキテクチャ改善に反映）。

凡例: ✅ 完了 / ⏳ 未着手 / 🔎 実装前に最新ドキュメント要確認

---

## アーキテクチャ指針（[docs/research/prior-art.md](docs/research/prior-art.md) で検証済み）

既存ツール調査により、以下の青写真を採用（Biome / rust-analyzer / Ruff / Prettier で実証済み）:

- **CST**: `rowan` の green/red ツリー。パーサは木を直接作らず **イベント列**（`Start`/`Token`/`Finish`/`Error`）を吐き、別パスで `GreenNodeBuilder` に流し込み、その際にトリビア（空白・コメント）を付与する（文法はクリーン、木はロスレス）。
- **パーサ**: 手書き再帰下降 ＋ `Marker`/`CompletedMarker`（`complete`/`abandon`/`precede`）。**「パースは失敗しない」**＝ `(SyntaxNode, Vec<SyntaxError>)` を返し、未知トークンは `ERROR` ノード、`TokenSet`(FOLLOW集合)で回復、無限ループは fuel カウンタで防ぐ。式は Pratt parsing。参考: matklad *Resilient LL Parsing* / *Pratt parsing*。
- **フォーマッタ**: **自前の汎用 Doc エンジン** `snow-fmt-formatter` を作る。`biome_formatter` には直接依存せず、その `FormatElement` 設計（Prettier → rome → biome/ruff の系譜）を模倣。SQL 固有の整形規則は別レイヤに分離。**magic trailing comma**（末尾カンマを「展開して」の意思表示として尊重）を看板機能に。
- **埋め込み JS**: SQL 本体の Doc エンジンとは独立に、delimiter-aware body token（現行 Snowflake は `$$…$$`）の本体のみ `biome_js_formatter` で整形し、`markAsRoot`/`dedentToRoot` 方式で配置列へ再インデント。解析不能時は verbatim フォールバック。
- **テスト**: `insta` スナップショット ＋ **stability-check（べき等）** ＋ ロスレス往復 ＋ 実コーパスでの**類似度スコア**ゲート ＋ ファズ。フィクスチャは sqlparser-rs の Snowflake テスト（Apache-2.0）を流用。
- **スタイル**: gofmt / zig fmt に倣い **opinionated・ほぼ設定なし**（`line-length`、必要なら `keyword-case` 程度）。

---

## Phase 0 — 基盤 ✅

- ✅ Cargo ワークスペース、`release` プロファイル最適化
- ✅ `SyntaxKind`（トークン種別、`rowan` 連携、`u16` 変換） … [crates/snow-fmt-syntax/](crates/snow-fmt-syntax/)
- ✅ 手書きロスレス Lexer（`->>`, `|>`, `::`, delimiter-aware body token（現行 `$$…$$`）, 3種コメント, 文字列エスケープ, 数値, 変数, ステージ`@`） … [crates/snow-fmt-lexer/](crates/snow-fmt-lexer/)
- ✅ キーワード認識（アロケーションフリー） … [keyword.rs](crates/snow-fmt-syntax/src/keyword.rs)
- ✅ テスト基盤（ロスレス不変条件・ファズ・網羅・完全性） … [tests/corpus.rs](crates/snow-fmt-lexer/tests/corpus.rs)
- ✅ 解説ドキュメント [GUIDE.md](GUIDE.md)
- ✅ 既存ツール調査 [docs/research/prior-art.md](docs/research/prior-art.md)（Biome / rust-analyzer / Ruff / Prettier / tree-sitter / sqlfluff / sqlparser-rs …）
- ✅ `T!` マクロ（句読点→`SyntaxKind`） … [macros.rs](crates/snow-fmt-syntax/src/macros.rs)

## Phase 1 — パーサ基盤 + CST ✅
*目的達成: イベント方式の手書きパーサで最小の式と SELECT を解析し、ロスレス CST を構築。クレート [crates/snow-fmt-parser/](crates/snow-fmt-parser/)。*
- ✅ `SnowflakeLang` rowan `Language` ／ `T!` マクロ
- ✅ パーサ = **イベント列**（`Open`/`Close`/`Advance`）→ `GreenNodeBuilder`。トリビアは構築時に挿入しロスレス（往復テスト済み）
- ✅ `Marker`/`CompletedMarker`（`complete`/`precede`）＋ `DropBomb`
- ✅ エラー回復: `ERROR` ノード＋ fuel カウンタ＋ **`Parse { green, Vec<ParseError> }` で決して失敗しない**（正式な `TokenSet` FOLLOW 集合は Phase 2 で導入）
- ✅ 式パーサ（Pratt: `OR`/`AND`/比較/`||`/加減/乗除、前置 `NOT`・単項 `±`、後置 呼び出し/添字/`::`キャスト、修飾名 `a.b.c`・`t.*`）
- ✅ 薄い型付き AST（`AstNode` トレイト＋ `SourceFile`/`SelectStmt`/… の `cast`/アクセサ）
- ✅ テスト: ロスレス往復（正常＋壊れた入力）／構造／回復／優先順位／敵対的入力（[tests/parser.rs](crates/snow-fmt-parser/tests/parser.rs)）。`insta` スナップショットは Phase 3 で導入
- 🔧 既知の Phase 2 送り: `JOIN`/集合演算/サブクエリ/CTE、`IS [NOT] NULL`・`NOT IN` 等の複合演算子、トリビアの厳密な node 帰属（現状ロスレスだが素朴配置）

## Phase 2 — コア SELECT 文 ⏳
*目的: 最頻出のクエリを完全にパース。ここが土台。*
- ⏳ `SELECT`（`DISTINCT`, `TOP`, 列エイリアス `AS`）, `FROM`
- ⏳ `WHERE`, `GROUP BY`, `HAVING`, `ORDER BY`, `LIMIT`/`OFFSET`/`FETCH`
- ⏳ `JOIN`（INNER/LEFT/RIGHT/FULL/CROSS/NATURAL, `ON`/`USING`）, サブクエリ
- ⏳ `WITH`（CTE, `RECURSIVE`）, 集合演算（`UNION [ALL]`/`EXCEPT`/`INTERSECT`/`MINUS`）
- ⏳ 関数呼び出し, `CASE`, `CAST`/`::`, 名前付き引数 `=>`, ラムダ `->`, 修飾名 `a.b.c`

## Phase 3 — フォーマッタ基盤 ⏳
*目的: Phase 2 までの構文を綺麗に出力する。SQL 規則の前に自前 Doc エンジンを立ち上げる。*
- ⏳ 汎用 Doc エンジン `snow-fmt-formatter`（`FormatElement`: `Text`/`SourceCodeSlice`/`Line(Soft/Hard/Empty/SoftOrSpace)`/`Group`/`Indent`/`LineSuffix`/`BestFitting`）。`biome_formatter` には依存しない
- ⏳ ビルダ（`group`/`indent`/`block_indent`/`soft_block_indent`/4種改行/`line_suffix`/`if_group_breaks`/`best_fitting`）＋ `Format`/`FormatRule` トレイト＋ `write!`/`format!` マクロ
- ⏳ 幅対応プリンタ（行幅で `group` を1行/折返し決定）
- ⏳ コメント付与（leading/trailing/dangling。末尾は `line_suffix`。ディレクティブ `-- noqa`/`-- snow-fmt:` は幅計算から除外）
- ⏳ **magic trailing comma**（SELECT列/`IN(...)`/`VALUES`/引数で末尾カンマ→展開固定）
- ⏳ 設定は最小（`line-length`・インデント幅・`keyword-case`）。opinionated・ほぼ設定なし
- ⏳ テスト: `insta` スナップショット ＋ **stability-check（べき等）** ＋ ラウンドトリップ。フィクスチャは sqlparser-rs(Apache-2.0) から流用

## Phase 4 — Snowflake 固有のクエリ構文 ⏳
*目的: 汎用 SQL フォーマッタが取りこぼす部分を制覇。*
- ⏳ `QUALIFY`, ウィンドウ関数（`OVER`, `PARTITION BY`, フレーム `ROWS/RANGE … PRECEDING/FOLLOWING`）, `WINDOW`句
- ⏳ セミ構造化アクセス（`col:path.to.field`, `[idx]`, `::type`, `OBJECT_CONSTRUCT`/`ARRAY_CONSTRUCT`）
- ⏳ `LATERAL FLATTEN` / `TABLE(FLATTEN(...))`
- ⏳ `PIVOT` / `UNPIVOT`
- ⏳ `GROUP BY ALL` / `CUBE` / `ROLLUP` / `GROUPING SETS`
- ⏳ `SAMPLE`/`TABLESAMPLE`, `MATCH_RECOGNIZE`, `CONNECT BY`/`START WITH`
- ⏳ 🔎 `ASOF JOIN`, Time Travel（`AT`/`BEFORE`）, `CHANGES`

## Phase 5 — フロー/パイプ構文 `->>` ⏳ 🔎
*目的: 既存ツールがほぼ未対応の差別化点。*
- ✅ Lexer は Snowflake 公式 `->>` と互換 `|>` の両方を単一トークン化
- ⏳ 🔎 flow operator の文脈（任意の SQL statement chain、`FROM $n` 参照、制限事項）を公式ドキュメントで継続確認
- ⏳ 整形規則（`->>` ステップを1行ずつ、インデント揃え）
- ⏳ パイプ／非パイプ混在の扱い

## Phase 6 — DML ⏳
- ⏳ `INSERT`（単一・`INSERT ALL`/`FIRST` の多テーブル）
- ⏳ `UPDATE`, `DELETE`, `MERGE`（`WHEN MATCHED`/`NOT MATCHED`）
- ⏳ `COPY INTO`（`FILE_FORMAT`, 各種オプション, ステージ/URL）

## Phase 7 — DDL ⏳
- ⏳ `CREATE TABLE`（`CLONE`, `AS SELECT`, 制約, クラスタリング）, `ALTER`, `DROP`
- ⏳ `VIEW` / `MATERIALIZED VIEW`, `SEQUENCE`, `FILE FORMAT`, `STAGE`, `SCHEMA`/`DATABASE`/`WAREHOUSE`
- ⏳ 🔎 `STREAM`, `TASK`, `DYNAMIC TABLE`（新しめ。構文要確認）
- ⏳ マスキング/行アクセスポリシー, タグ, `GRANT`/`REVOKE`

## Phase 8 — 手続き・関数・埋め込み言語 ⏳ ＜第2の差別化点＞
- ⏳ `CREATE PROCEDURE`/`FUNCTION`/`UDTF`（`LANGUAGE` 別: SQL/JS/Python/Java/Scala, `RETURNS`, `HANDLER`, `IMPORTS`, `PACKAGES`）
- ⏳ Snowflake Scripting（`DECLARE`/`BEGIN`/`EXCEPTION`/`END`, `LET`, `:=`, `FOR`/`WHILE`/`REPEAT`/`LOOP`, `IF`/`CASE`, カーソル, `RESULTSET`, `RETURN`）
- ⏳ delimiter-aware body token の言語判定 → サブフォーマッタへ委譲 → 再インデント
  - ⏳ **JavaScript**: Biome の `biome_js_formatter` を組み込み
  - ⏳ Python: 整形方針を決定（外部 ruff か、当面は無加工パススルー）
  - ⏳ ネストした SQL（`LANGUAGE SQL`）: 自分自身で再帰整形

## Phase 9 — ハイライト + LSP ⏳
- ✅ Lexical highlight 基盤（keyword/type/string/comment/operator/punctuation/range、内蔵 easy fixture 全 SQL でロスレス検証） … [crates/snow-fmt-highlight/](crates/snow-fmt-highlight/)
- ✅ Hover 基盤（Snowflake 型、`CREATE PROCEDURE` の signature/returns/language、`CREATE TASK` の compute/schedule/when、procedure/task property 説明） … [crates/snow-fmt-hover/](crates/snow-fmt-hover/)
- ✅ Tree-sitter grammar baseline（Neovim/Zed/GitHub 向け token grammar、highlight/locals/injections queries、Rust wrapper、内蔵 easy fixture 全 SQL + LF/CRLF/CR/mixed 改行で cargo test 統合） … [tree-sitter-snowflake/](tree-sitter-snowflake/) / [crates/snow-fmt-tree-sitter/](crates/snow-fmt-tree-sitter/)
- ⏳ CST → セマンティックトークン
- ⏳ LSP サーバ（`textDocument/formatting`, `semanticTokens`, 診断）
- ⏳ TextMate 文法（素のエディタ向けベースライン）
- ⏳ LSP のインクリメンタル更新
- ⏳ Tree-sitter 文法の構造化（statement/expression ノード、context-aware injections、folds/indents/hover 連携）

## Phase 10 — 仕上げ・周辺 ⏳
- ⏳ 🔎 Cortex / AISQL 関数（`AI_COMPLETE`, `SNOWFLAKE.CORTEX.*` 等）の認識
- ⏳ CLI（`snow-fmt format` / `check`）, 設定ファイル（`snow-fmt.toml`）
- ⏳ 複数ファイル並列整形（`rayon`）, ベンチマーク
- ⏳ 大規模コーパスでのべき等性・無破壊（ラウンドトリップ）回帰
- ⏳ エディタ拡張（VS Code）パッケージング

---

### 次の一手
**Phase 3（フォーマッタ Doc IR）** を立ち上げつつ、Phase 2 文法を内蔵 fixture の頻出構文から広げる。現時点では Rust テストに golden/full/sql-only、lexer/parser recovery、lexical highlight、Tree-sitter の全 SQL ロスレス検証が組み込まれているため、`cargo test --workspace` を自己完結した回帰ゲートにして進められる。
