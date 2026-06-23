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
- ✅ Phase 2 で消化済み: `JOIN`/集合演算/サブクエリ/CTE、`IS [NOT] NULL`・`NOT IN` 等の複合演算子。トリビアの厳密な node 帰属は現状ロスレス（素朴配置で実害なし）

## Phase 2 — コア SELECT 文 ✅
*目的: 最頻出のクエリを完全にパース。ここが土台。*
- ✅ `SELECT`（`DISTINCT`, `TOP`, 列エイリアス `AS`）, `FROM`
- ✅ `WHERE`, `GROUP BY`, `HAVING`, `ORDER BY`, `LIMIT`/`OFFSET`/`FETCH`
- ✅ `JOIN`（INNER/LEFT/RIGHT/FULL/CROSS/NATURAL, `ON`/`USING`）, サブクエリ
- ✅ `WITH`（CTE, `RECURSIVE`）, 集合演算（`UNION [ALL]`/`EXCEPT`/`INTERSECT`/`MINUS`）
- ✅ 関数呼び出し（集約 `DISTINCT`/`ALL` 量化子 `COUNT(DISTINCT x)`、予約語名の関数 `first()/last()/left()`）, `CASE`, `CAST`/`::`, 名前付き引数 `=>`, ラムダ `->`, 修飾名 `a.b.c`

## Phase 3 — フォーマッタ基盤 ✅
*目的: Phase 2 までの構文を綺麗に出力する。SQL 規則の前に自前 Doc エンジンを立ち上げる。クレート [crates/snow-fmt-formatter/](crates/snow-fmt-formatter/)。*
- ✅ 汎用 Doc エンジン `snow-fmt-formatter`（`FormatElement` 中核サブセット: `Text`/`Line(Soft/Space/Hard)`/`Group`/`Indent`、`breakParent` 伝播）。`biome_formatter` 非依存 … [doc.rs](crates/snow-fmt-formatter/src/doc.rs)。残: `SourceCodeSlice`/`LineSuffix`/`BestFitting`
- ✅ ビルダ（`text`/`concat`/`join`/`group`/`indent`/`line`/`soft_line`/`hard_line`/`space`/`empty`）＋ 幅対応プリンタ（Prettier 系 `fits`／flat-or-break）。残: `block_indent`/`line_suffix`/`if_group_breaks`/`best_fitting`／`Format`/`FormatRule` トレイト・`write!` マクロ
- ✅ 幅対応プリンタ（行幅で `group` を1行/折返し決定）… [doc.rs](crates/snow-fmt-formatter/src/doc.rs) `print`
- ✅ SQL 規則の `SELECT` パイプライン: 文の区切り/終端、各句を改行、SELECT 列が幅超過で1列1行に展開、句内空白の正規化、キーワード大文字化 … [sql.rs](crates/snow-fmt-formatter/src/sql.rs)
- ✅ 句の構造的整形: `JOIN` を1行ずつ（`FROM` 直下に整列）、`ORDER BY`/`GROUP BY` の項目を幅超過で1項目1行に折返し（`GROUP BY ALL` 等の項目なし形はそのまま）… [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_from`/`lower_keyword_item_list`
- ✅ 式の構造的整形: `CASE`（収まれば1行、溢れたら `WHEN`/`ELSE` を1アーム1行・`END` をデデント。単純 CASE のオペランド対応）… [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_case`
- ✅ サブクエリ／CTE の構造的整形: 括弧サブクエリは収まれば1行・溢れたら本文をインデント（多句 SELECT は hard_line で強制改行）。`WITH [RECURSIVE]` は CTE を1つ1行、本文を再帰整形 … [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_subquery`/`lower_with_query`
- ✅ 集合演算（`UNION [ALL] / EXCEPT / INTERSECT / MINUS`）: 各クエリと演算子をそれぞれ別行に、連鎖はフラット化 … [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_set_op`
- ✅ コメント付与（leading/trailing/dangling）: コメントを有意トークンに帰属（前トークンと同じ行＝trailing、改行後＝次トークンの leading、行コメントは `line_suffix`＋`break_parent` で行末へ）。各コメントを1度だけ出力し、**帰属先トークンを実際に描画できないコメントが1つでも残ればその文だけ verbatim にフォールバック**（無破壊を機械保証）。Doc エンジンに `line_suffix`/`break_parent` を追加 … [doc.rs](crates/snow-fmt-formatter/src/doc.rs) / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `Comments`。残: ディレクティブ（`-- noqa`/`-- snow-fmt:`）の幅計算除外、リーディング文コメントが SELECT を展開させる微調整
- ✅ **magic trailing comma**（看板機能）: SELECT 列・関数引数 (`ARG_LIST`)・`VALUES` 行・列リスト (`COLUMN_LIST`)・`IN (...)`（`EXPR_LIST` を親の括弧と束ねて構造化）で実装済み（作者の末尾カンマ＝「展開固定」と解釈し幅に関わらず展開。展開したコレクションは祖先グループも改行させる＝Black 流。既存カンマを保持しトークンは合成しない＝無破壊）。Doc エンジンに**伝播する `group_expanded`**（Prettier `shouldBreak`）を追加 … [doc.rs](crates/snow-fmt-formatter/src/doc.rs) `group_expanded` / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `bracketed`/`lower_in_expr`
- 🚧 設定は最小（`line-length`・インデント幅・`keyword-case`）。`FormatOptions { line_width, indent_width, uppercase_keywords }` 実装済み。opinionated・ほぼ設定なし
- ✅ テスト: **stability-check（べき等 `format(format(x))==format(x)`）** ＋ ラウンドトリップ（有意トークン列の保存）＋ クリーン入力の再パース無エラー、内蔵 easy fixture 全 SQL で検証。複数行ゴールデンは **`insta` インラインスナップショット**（`cargo insta test --accept` で更新） … [tests/format.rs](crates/snow-fmt-formatter/tests/format.rs)
- 📝 Phase 3 のスコープ境界: パーサが**完全に受理した入力のみ整形**し、`ParseError` が出る入力は無変更パススルー（無破壊・べき等を機械的に保証）。Phase 2 文法の拡張に従ってカバレッジが自動的に広がる

## Phase 4 — Snowflake 固有のクエリ構文 ✅
*目的: 汎用 SQL フォーマッタが取りこぼす部分を制覇。*
- ✅ `QUALIFY`（Phase 2 で対応）, ウィンドウ関数（`OVER`, `PARTITION BY`, フレーム `ROWS/RANGE … PRECEDING/FOLLOWING`）, `WINDOW`句（フレームは Phase 2 で）。整形は当面インライン
- ✅ セミ構造化アクセス（`col:path.to.field`=`JSON_ACCESS`, `[idx]`=`INDEX_EXPR`, `::type`=`CAST_EXPR`。Phase 1–2b で対応）。`OBJECT_CONSTRUCT`/`ARRAY_CONSTRUCT` は通常の関数呼び出しとして整形
- ✅ **ordered-set 集約** `… WITHIN GROUP (ORDER BY …)`（`LISTAGG`/`ARRAY_AGG` 等。式の後置として `WITHIN_GROUP` ノード）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `expr_bp` / 新キーワード `WITHIN`
- ✅ `LATERAL FLATTEN` / `TABLE(FLATTEN(...))` ＋ テーブル関数（`my_udtf(args)`）／**名前付き引数** `f(name => val)`（`NAMED_ARG` ノード、`FLATTEN`/`TABLE` を callable 化）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `table_ref`/`arg`
- ✅ `PIVOT` / `UNPIVOT`（`<table> PIVOT(<agg>(col) FOR col IN (…))` を `table_ref` の後置として `PIVOT_CLAUSE` ノードで対応）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `pivot_clause` / 新キーワード `FOR`
- ✅ `GROUP BY ALL` / `CUBE(...)` / `ROLLUP(...)`（関数呼び出しとして整形）/ `GROUPING SETS ((...), ...)`（`GROUPING(col)` 関数と衝突しない **contextual keyword**（text ベース判定）で `GROUPING_SETS` ノードに）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `grouping_element` / [parser.rs](crates/snow-fmt-parser/src/parser.rs) `nth_contextual`
- ✅ `SAMPLE`/`TABLESAMPLE`（`[method] (n [ROWS]) [REPEATABLE/SEED(...)]` を `table_ref` 後置で寛容保持）, `MATCH_RECOGNIZE`（✅ 本体を構造化: PARTITION/ORDER/MEASURES/PER MATCH/AFTER MATCH SKIP/PATTERN/SUBSET/DEFINE を1句1行・contextual 大文字化・`PATTERN(...)` は verbatim）, ✅ `CONNECT BY`/`START WITH`（`PRIOR` 前置・`NOCYCLE`、各句1行）。`PIVOT` の `IN (val AS alias, ...)` も対応
- 🚧 `ASOF JOIN`（✅ `a ASOF JOIN b MATCH_CONDITION (...) [ON ...]`）, Time Travel `AT`/`BEFORE`（✅ `t AT (TIMESTAMP|OFFSET|STATEMENT => ...)`、`table_ref` 後置・寛容保持）。contextual keyword `asof`/`match_condition`/`at`/`before` のエイリアス誤食いを `at_alias_blocker` で回避 … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `join`/`time_travel`/`at_alias_blocker`。✅ `CHANGES ( INFORMATION => … ) {AT|BEFORE}(…) [END(…)]`（`table_ref` 後置）

## Phase 5 — フロー/パイプ構文 `->>` ✅
*目的: 既存ツールがほぼ未対応の差別化点。*
- ✅ Lexer は Snowflake 公式 `->>` と互換 `|>` の両方を単一トークン化
- ✅ flow operator の文脈を公式ドキュメントで確認（任意の SQL statement chain、ステップ間にセミコロンなし、`FROM $n` で前ステップ参照） … <https://docs.snowflake.com/en/sql-reference/operators-flow>
- ✅ 文チェーンを `FLOW_STMT` ノードでパース（投機的 wrapper を `Marker::abandon`/`Tombstone` で単文時に破棄）、`FROM $1` を table source として許可 … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `statement_or_flow`/`table_ref`
- ✅ 整形規則（各ステップを通常整形し、`->>` を継続行の先頭に。ステップ間にセミコロンを入れない） … [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_flow`
- 🔧 残: SHOW 始まりのチェーン（`SHOW` 文未対応のため現状は素通し）、パイプ／非パイプ混在の網羅

## Phase 6 — DML ✅
- ✅ `INSERT`（単一 `INSERT [OVERWRITE] INTO t [(cols)] VALUES/<query>`、多テーブル `INSERT [OVERWRITE] {ALL|FIRST} (WHEN cond THEN INTO …)+ [ELSE INTO …] <query>` をパース＋構造的整形。新ノード `INTO_CLAUSE`/`INSERT_WHEN`、新キーワード `OVERWRITE`）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `multi_table_insert`
- ✅ `UPDATE`（`SET`/`FROM`/`WHERE`）, `DELETE`（`USING`/`WHERE`）, `MERGE`（`WHEN [NOT] MATCHED [AND] THEN UPDATE/DELETE/INSERT`）をパース＋構造的整形（各句1行）… パーサ [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) / 整形 [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_clausal`（INSERT/UPDATE/DELETE/MERGE/COPY/CREATE 共通）。新ノード `INSERT_STMT`/`UPDATE_STMT`/`DELETE_STMT`/`MERGE_STMT`/`SET_CLAUSE`/`ASSIGNMENT`/`MERGE_WHEN`、新キーワード `MATCHED`
- ✅ `COPY INTO`（ロード/アンロード両形。`COPY INTO <target> FROM <source>` ＋各オプション (`FILE_FORMAT = (...)`, `PATTERN`, `ON_ERROR`, `PARTITION BY (...)` 等) を1行ずつ。ステージパス `@stage/path` は verbatim 保持）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `copy_stmt` / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_copy`。新ノード `COPY_STMT`/`COPY_LOCATION`/`COPY_OPTION`。**コーパス 32→34**

## Phase 7 — DDL 🚧
- 🚧 `CREATE [OR REPLACE] TABLE`（`AS SELECT`(CTAS)・列定義 `( ... )` を寛容パース＋整形済み。残: `CLONE`, 制約の構造化, `CLUSTER BY` 等オプションの構造化）, `DROP`（`IF EXISTS`/`CASCADE`）, `ALTER`（寛容にトークン列としてパース→インライン整形）… パーサ [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `create_stmt`/`drop_stmt`/`alter_stmt` / 整形 [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_create`。新ノード `CREATE_STMT`/`DROP_STMT`/`ALTER_STMT`/`COLUMN_DEF_LIST`/`COLUMN_DEF`、新キーワード `DROP`/`ALTER`
- 🚧 `CREATE [OR REPLACE] [SECURE] [MATERIALIZED] VIEW [(cols)] [options] AS <query>` をパース＋整形（修飾子・オプションは寛容にトークン保持）。残: `SEQUENCE`, `FILE FORMAT`, `STAGE`, `SCHEMA`/`DATABASE`/`WAREHOUSE`
- ⏳ 🔎 `STREAM`, `TASK`, `DYNAMIC TABLE`（新しめ。構文要確認）
- ⏳ マスキング/行アクセスポリシー, タグ, `GRANT`/`REVOKE`

## Phase 8 — 手続き・関数・埋め込み言語 🚧 ＜第2の差別化点＞
- 🚧 `CREATE PROCEDURE`/`FUNCTION`（**骨格**: シグネチャ・`RETURNS`・`LANGUAGE`・各種オプションを寛容にトークン保持、ボディは区切りトークン `$$ … $$` / `'…'` を **verbatim** 保持。ヘッダは構造的整形・引数は1つ1行）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `create_routine` / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_create`。**コーパス clean 0→20件** に。残: UDTF の `TABLE(...)` 戻り、区切りなし scripting ボディ（現状はエラー→素通しで誤分割を防止）
- ✅ セッション `SET <var> = <expr>` / `SET (a, b) = (...)`、`EXECUTE IMMEDIATE <string|$$…$$|:var> [USING (...)]`（`SET_STMT`/`EXECUTE_STMT` ノード、新キーワード `IMMEDIATE`。式に `DOLLAR_STRING` を許可）。**コーパス clean 20→22件** … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `set_stmt`/`execute_stmt`
- ⏳ Snowflake Scripting（`DECLARE`/`BEGIN`/`EXCEPTION`/`END`, `LET`, `:=`, `FOR`/`WHILE`/`REPEAT`/`LOOP`, `IF`/`CASE`, カーソル, `RESULTSET`, `RETURN`）— ボディ内部の整形（現状は verbatim 保持）
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

## Phase 10 — 仕上げ・周辺 🚧
- ⏳ 🔎 Cortex / AISQL 関数（`AI_COMPLETE`, `SNOWFLAKE.CORTEX.*` 等）の認識
- ✅ CLI `snow-fmt`（`--write`/`--check`/stdin、`--line-width`/`--indent-width`/`--no-uppercase`、エンコーディング保持、`cargo install` 可、v0.1.0） … [crates/snow-fmt-cli/](crates/snow-fmt-cli/)。残: 設定ファイル（`snow-fmt.toml`）
- ⏳ 複数ファイル並列整形（`rayon`）, ベンチマーク
- ✅ 大規模コーパスでのべき等性・無破壊（ラウンドトリップ）回帰（内蔵 easy fixture 全 SQL で機械ガード）。残: より大きな外部コーパス
- ⏳ エディタ拡張（VS Code）パッケージング

---

### 現状サマリ（2026-06）
**Phase 0–6 は実質完了**、Phase 9 は基盤（highlight/hover/tree-sitter baseline）まで、Phase 7/8/10 が部分。コア整形（SELECT 一式・DML・基本 DDL・COPY・Snowflake 固有クエリ）は無破壊・べき等を機械保証しつつ実用段階。CLI `snow-fmt` v0.1.0 公開可。

**残りの主な未着手（価値順）**:
1. **Phase 8 scripting / 埋め込み言語**: `DECLARE/BEGIN/END`・`LET`・制御構文のボディ内部整形、`$$…$$` の言語判定→サブフォーマッタ委譲（JS=biome、SQL=自己再帰）。現状は verbatim 保持で無破壊。
2. **Phase 7 DDL の拡張**: `STREAM`/`TASK`/`DYNAMIC TABLE`/`SEQUENCE`/`STAGE`/`FILE FORMAT`/ポリシー/`GRANT`（🔎 新しめは要ドキュメント確認）。インライン巨大1行になるものは無理に取り込まず素通しのまま。
3. **Phase 5 フロー演算子 `->>`** の整形規則（🔎 仕様確認）。
4. **Phase 9 LSP**（`textDocument/formatting`・`semanticTokens`・診断）と Tree-sitter 文法の構造化。
5. **Phase 10**: `snow-fmt.toml`、`rayon` 並列、Cortex/AISQL 関数認識、VS Code 拡張、`CHANGES`。

回帰ゲートは `cargo test --workspace`（golden=insta、full/sql-only、lexer/parser recovery、lexical highlight、Tree-sitter、formatter べき等/ラウンドトリップ）＋ `cargo clippy --workspace --all-targets` ＋ `cargo fmt --all --check`。
