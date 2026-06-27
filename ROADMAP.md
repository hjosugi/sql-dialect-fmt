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
- **埋め込み言語**: SQL 本体の Doc エンジンとは独立に、delimiter-aware body token（現行 Snowflake は `$$…$$`）の本体のみ言語別サブフォーマッタで整形。SQL は自己再帰、JavaScript は `biome_js_formatter`、Python は `ruff_python_formatter`、Java/Scala は brace-aware lightweight formatter で処理し、解析不能時は verbatim フォールバック。
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
- ✅ 汎用 Doc エンジン `snow-fmt-formatter`（`FormatElement` 中核サブセット: `Text`/`Line(Soft/Space/Hard)`/`Group`/`Indent`/`LineSuffix`/`BreakParent`、`breakParent` 伝播）。`biome_formatter` 非依存 … [doc.rs](crates/snow-fmt-formatter/src/doc.rs)。残: `SourceCodeSlice`/`BestFitting`
- ✅ ビルダ（`text`/`concat`/`join`/`group`/`group_expanded`/`indent`/`line`/`soft_line`/`hard_line`/`line_suffix`/`space`/`empty`）＋ 幅対応プリンタ（Prettier 系 `fits`／flat-or-break）。残: `block_indent`/`if_group_breaks`/`best_fitting`／`Format`/`FormatRule` トレイト・`write!` マクロ
- ✅ 幅対応プリンタ（行幅で `group` を1行/折返し決定）… [doc.rs](crates/snow-fmt-formatter/src/doc.rs) `print`
- ✅ SQL 規則の `SELECT` パイプライン: 文の区切り/終端、各句を改行、SELECT 列が幅超過で1列1行に展開、句内空白の正規化、キーワード大文字化 … [sql.rs](crates/snow-fmt-formatter/src/sql.rs)
- ✅ 句の構造的整形: `JOIN` を1行ずつ（`FROM` 直下に整列）、`ORDER BY`/`GROUP BY` の項目を幅超過で1項目1行に折返し（`GROUP BY ALL` 等の項目なし形はそのまま）… [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_from`/`lower_keyword_item_list`
- ✅ 式の構造的整形: `CASE`（収まれば1行、溢れたら `WHEN`/`ELSE` を1アーム1行・`END` をデデント。単純 CASE のオペランド対応）… [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_case`
- ✅ サブクエリ／CTE の構造的整形: 括弧サブクエリは収まれば1行・溢れたら本文をインデント（多句 SELECT は hard_line で強制改行）。`WITH [RECURSIVE]` は CTE を1つ1行、本文を再帰整形 … [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_subquery`/`lower_with_query`
- ✅ 集合演算（`UNION [ALL] / EXCEPT / INTERSECT / MINUS`）: 各クエリと演算子をそれぞれ別行に、連鎖はフラット化 … [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_set_op`
- ✅ コメント付与（leading/trailing/dangling）: コメントを有意トークンに帰属（前トークンと同じ行＝trailing、改行後＝次トークンの leading、行コメントは `line_suffix`＋`break_parent` で行末へ）。各コメントを1度だけ出力し、**帰属先トークンを実際に描画できないコメントが1つでも残ればその文だけ verbatim にフォールバック**（無破壊を機械保証）。文頭 leading コメントは header group の外へ hoist、寛容文中の行コメントも次トークン前で改行確定して idempotent。Doc エンジンに `line_suffix`/`break_parent` を追加 … [doc.rs](crates/snow-fmt-formatter/src/doc.rs) / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `Comments`。残: ディレクティブ（`-- noqa`/`-- snow-fmt:`）の幅計算除外
- ✅ **magic trailing comma**（看板機能）: SELECT 列・関数引数 (`ARG_LIST`)・`VALUES` 行・列リスト (`COLUMN_LIST`)・`IN (...)`（`EXPR_LIST` を親の括弧と束ねて構造化）で実装済み（作者の末尾カンマ＝「展開固定」と解釈し幅に関わらず展開。展開したコレクションは祖先グループも改行させる＝Black 流。既存カンマを保持しトークンは合成しない＝無破壊）。Doc エンジンに**伝播する `group_expanded`**（Prettier `shouldBreak`）を追加 … [doc.rs](crates/snow-fmt-formatter/src/doc.rs) `group_expanded` / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `bracketed`/`lower_in_expr`
- ✅ 設定は最小（`line-length`・インデント幅・`keyword-case`）。`FormatOptions { line_width, indent_width, uppercase_keywords }` 実装済み。opinionated・ほぼ設定なし
- ✅ テスト: **stability-check（べき等 `format(format(x))==format(x)`）** ＋ ラウンドトリップ（有意トークン列の保存）＋ クリーン入力の再パース無エラー、内蔵 easy fixture 全 SQL で検証。複数行ゴールデンは **`insta` インラインスナップショット**（`cargo insta test --accept` で更新）。さらに parser/formatter の `proptest` property harness で Unicode/ASCII/token-salad の panic-safety・lossless CST・idempotency・token preservation を回帰ガード … [tests/format.rs](crates/snow-fmt-formatter/tests/format.rs) / [formatter proptest](crates/snow-fmt-formatter/tests/proptest_invariants.rs) / [parser proptest](crates/snow-fmt-parser/tests/proptest_invariants.rs)
- 📝 Phase 3 のスコープ境界: lexer とパーサが**完全に受理した入力のみ整形**し、`LexError`/`ParseError` が出る入力は無変更パススルー（無破壊・べき等を機械的に保証）。複数行 token 内の行末空白など、Doc printer の global trim が token 内部へ干渉しうる入力も無変更に倒す。Phase 2 文法の拡張に従ってカバレッジが自動的に広がる

## Phase 4 — Snowflake 固有のクエリ構文 ✅
*目的: 汎用 SQL フォーマッタが取りこぼす部分を制覇。*
- ✅ `QUALIFY`（Phase 2 で対応）, ウィンドウ関数（`OVER`, `PARTITION BY`, フレーム `ROWS/RANGE … PRECEDING/FOLLOWING`）, `WINDOW`句（フレームは Phase 2 で）。整形は当面インライン
- ✅ セミ構造化アクセス（`col:path.to.field`=`JSON_ACCESS`, `[idx]`=`INDEX_EXPR`, `::type`=`CAST_EXPR`。Phase 1–2b で対応）。`OBJECT_CONSTRUCT`/`ARRAY_CONSTRUCT` は通常の関数呼び出しとして整形
- ✅ **ordered-set 集約** `… WITHIN GROUP (ORDER BY …)`（`LISTAGG`/`ARRAY_AGG` 等。式の後置として `WITHIN_GROUP` ノード）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `expr_bp` / 新キーワード `WITHIN`
- ✅ `LATERAL FLATTEN` / `TABLE(FLATTEN(...))` ＋ テーブル関数（`my_udtf(args)`）／**名前付き引数** `f(name => val)`（`NAMED_ARG` ノード、`FLATTEN`/`TABLE` を callable 化）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `table_ref`/`arg`
- ✅ `PIVOT` / `UNPIVOT`（`<table> PIVOT(<agg>(col) FOR col IN (…))` を `table_ref` の後置として `PIVOT_CLAUSE` ノードで対応）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `pivot_clause` / 新キーワード `FOR`
- ✅ `GROUP BY ALL` / `CUBE(...)` / `ROLLUP(...)`（関数呼び出しとして整形）/ `GROUPING SETS ((...), ...)`（`GROUPING(col)` 関数と衝突しない **contextual keyword**（text ベース判定）で `GROUPING_SETS` ノードに）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `grouping_element` / [parser.rs](crates/snow-fmt-parser/src/parser.rs) `nth_contextual`
- ✅ `SAMPLE`/`TABLESAMPLE`（`[method] (n [ROWS]) [REPEATABLE/SEED(...)]` を `table_ref` 後置で寛容保持）, `MATCH_RECOGNIZE`（✅ 本体を構造化: PARTITION/ORDER/MEASURES/PER MATCH/AFTER MATCH SKIP/PATTERN/SUBSET/DEFINE を1句1行・contextual 大文字化・`PATTERN(...)` は verbatim）, ✅ `CONNECT BY`/`START WITH`（`PRIOR` 前置・`NOCYCLE`、各句1行）。`PIVOT` の `IN (val AS alias, ...)` も対応
- ✅ `ASOF JOIN`（`a ASOF JOIN b MATCH_CONDITION (...) [ON ...]`）, Time Travel `AT`/`BEFORE`（`t AT (TIMESTAMP|OFFSET|STATEMENT => ...)`、`table_ref` 後置・寛容保持）。contextual keyword `asof`/`match_condition`/`at`/`before` のエイリアス誤食いを `at_alias_blocker` で回避 … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `join`/`time_travel`/`at_alias_blocker`。✅ `CHANGES ( INFORMATION => … ) {AT|BEFORE}(…) [END(…)]`（`table_ref` 後置）

## Phase 5 — フロー/パイプ構文 `->>` ✅
*目的: 既存ツールがほぼ未対応の差別化点。*
- ✅ Lexer は Snowflake 公式 `->>` と互換 `|>` の両方を単一トークン化
- ✅ flow operator の文脈を公式ドキュメントで確認（任意の SQL statement chain、ステップ間にセミコロンなし、`FROM $n` で前ステップ参照） … <https://docs.snowflake.com/en/sql-reference/operators-flow>
- ✅ 文チェーンを `FLOW_STMT` ノードでパース（投機的 wrapper を `Marker::abandon`/`Tombstone` で単文時に破棄）、`FROM $1` を table source として許可 … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `statement_or_flow`/`table_ref`
- ✅ 整形規則（各ステップを通常整形し、`->>` を継続行の先頭に。ステップ間にセミコロンを入れない） … [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_flow`
- 🔧 残: パイプ／非パイプ混在と `SHOW` 始まりチェーンの追加ゴールデン網羅

## Phase 6 — DML ✅
- ✅ `INSERT`（単一 `INSERT [OVERWRITE] INTO t [(cols)] VALUES/<query>`、多テーブル `INSERT [OVERWRITE] {ALL|FIRST} (WHEN cond THEN INTO …)+ [ELSE INTO …] <query>` をパース＋構造的整形。新ノード `INTO_CLAUSE`/`INSERT_WHEN`、新キーワード `OVERWRITE`）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `multi_table_insert`
- ✅ `UPDATE`（`SET`/`FROM`/`WHERE`）, `DELETE`（`USING`/`WHERE`）, `MERGE`（`WHEN [NOT] MATCHED [AND] THEN UPDATE/DELETE/INSERT`）をパース＋構造的整形（各句1行）… パーサ [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) / 整形 [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_clausal`（INSERT/UPDATE/DELETE/MERGE/COPY/CREATE 共通）。新ノード `INSERT_STMT`/`UPDATE_STMT`/`DELETE_STMT`/`MERGE_STMT`/`SET_CLAUSE`/`ASSIGNMENT`/`MERGE_WHEN`、新キーワード `MATCHED`
- ✅ `COPY INTO`（ロード/アンロード両形。`COPY INTO <target> FROM <source>` ＋各オプション (`FILE_FORMAT = (...)`, `PATTERN`, `ON_ERROR`, `PARTITION BY (...)` 等) を1行ずつ。ステージパス `@stage/path` は verbatim 保持。認識済み option key は key position のみ大文字化し、値/識別子は不変）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `copy_stmt` / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_copy`/`lower_option_node`。新ノード `COPY_STMT`/`COPY_LOCATION`/`COPY_OPTION`。**コーパス 32→34**

## Phase 7 — DDL 🚧
- ✅ `CREATE [OR REPLACE] TABLE`（列定義、制約、`CLONE`, `CLUSTER BY`, CTAS/`AS SELECT`）/ `CREATE [SECURE] [MATERIALIZED] VIEW [(cols)] [options] AS <query>` / `DROP` / `ALTER` をパース＋整形。列定義は幅超過で1列1行、CTAS/VIEW body は構造的整形、`ALTER` は寛容インライン … パーサ [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `create_stmt`/`drop_stmt`/`alter_stmt` / 整形 [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_create`
- ✅ Object DDL: `CREATE SCHEMA`/`DATABASE`/`WAREHOUSE`/`SEQUENCE`/`STAGE`/`FILE FORMAT`/`STREAM`/`TASK`/`DYNAMIC TABLE` を構造化。`OBJECT_PROPERTY`/`STREAM_SOURCE`/`TASK_AFTER` を導入し、property は1行ずつ、`TASK`/`DYNAMIC TABLE` の `AS <query|dml>` body は構造的に整形。認識済み option key は key position のみ大文字化 … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `create_other`/`object_property` / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_create_object`。fixture `case_032` と exhaustive object DDL matrix で parse/format/idempotency/token preservation を検証
- ✅ `GRANT`/`REVOKE`（privilege list、`ON <object>`、`TO|FROM [ROLE|USER]`、`WITH GRANT OPTION`、`GRANT OPTION FOR`、`CASCADE`/`RESTRICT`）を構造化し、target/grantee を1行ずつ整形。新ノード `GRANT_STMT`/`REVOKE_STMT`/`PRIV_LIST`/`GRANT_TARGET`/`GRANTEE`、新キーワード `GRANT`/`REVOKE`。fixture `case_031` と matrix で golden/べき等/ラウンドトリップを検証 … パーサ [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `grant_stmt`/`revoke_stmt` / 整形 [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_grant`
- ✅ セッション/introspection 文 `USE`/`SHOW`/`DESCRIBE`(`DESC`)/`TRUNCATE`（`lenient_stmt` 共通ヘルパで寛容パース→新ノード `USE_STMT`/`SHOW_STMT`/`DESCRIBE_STMT`/`TRUNCATE_STMT`、インライン整形。新キーワード `USE`/`SHOW`/`DESCRIBE`/`TRUNCATE`。**バグ修正**: `USE ROLE r` が3文に分割され `;` が挿入されていた非ロスレス挙動を解消。`ORDER BY … DESC` は従来どおり）。fixture `case_034`、parser で回帰ガード … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `lenient_stmt`
- ✅ `COMMENT ON <object> IS '…'`（オブジェクト注釈。`comment` は `ON` の直前でのみ効く**コンテキストキーワード**にして、頻出する `comment` 列名を誤ってキーワード化しない。`COMMENT_STMT` ノード、`CONTEXTUAL_KEYWORD` 化で大文字化）。fixture `case_035`、parser で衝突回避を回帰ガード … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `comment_stmt`
- ✅ `UNDROP <object> <name>`（`UNDROP_STMT`、寛容インライン）。**バグ修正**: `UNDROP SCHEMA s` の複数文分割を解消。fixture `case_036`
- ✅ `CREATE [OR REPLACE|OR ALTER] MASKING POLICY` / `ROW ACCESS POLICY` / `TAG`（policy は `AS (...) RETURNS ... -> ...` を clean parse + inline formatting、tag は `ALLOWED_VALUES`/`COMMENT`/`PROPAGATE` を property region として整形）。公式 docs で syntax 確認済み。object DDL matrix で parse/format/idempotency/token preservation を検証 … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `create_policy`/`object_property`
- ⏳ Semantic View、細かい object option のさらなる構造化（🔎 新しめは要ドキュメント確認）

## Phase 8 — 手続き・関数・埋め込み言語 🚧 ＜第2の差別化点＞
- 🚧 `CREATE PROCEDURE`/`FUNCTION`（**骨格 + SQL/JS/Python/Java/Scala ボディ整形**: シグネチャ・`RETURNS`・`LANGUAGE`・各種オプションを寛容にトークン保持。`LANGUAGE SQL AS $$ … $$` は内部 SQL/Scripting を同じ formatter で再帰整形、`LANGUAGE JAVASCRIPT` は Biome (`biome_js_formatter`)、`LANGUAGE PYTHON` は Ruff (`ruff_python_formatter`)、Java/Scala は brace-aware lightweight formatter に委譲。解析不能時は安全に元 token を保持。quoted body は現状 **verbatim**。ヘッダは構造的整形・引数は1つ1行）… [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `create_routine` / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_create_routine`。残: UDTF の `TABLE(...)` 戻り、quoted body の扱い
- ✅ セッション `SET <var> = <expr>` / `SET (a, b) = (...)`、`EXECUTE IMMEDIATE <string|$$…$$|:var> [USING (...)]`（`SET_STMT`/`EXECUTE_STMT` ノード、新キーワード `IMMEDIATE`。式に `DOLLAR_STRING` を許可）。**コーパス clean 20→22件** … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `set_stmt`/`execute_stmt`
- ✅ `CALL proc(args)`（プロシージャ呼び出し。`CALL_STMT` ノード、呼び出しは通常の call 式として整形＝引数オーバーフロー時は1引数1行、`INTO :var` 等の末尾は寛容保持）。fixture `case_033` … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `call_stmt`
- ✅ トランザクション制御 `BEGIN`/`COMMIT`/`ROLLBACK`（`BEGIN TRANSACTION`/`BEGIN WORK`/`BEGIN;`、`COMMIT WORK`、`ROLLBACK TO SAVEPOINT …` 含む。`TRANSACTION_STMT`）。**バグ修正**: `COMMIT WORK`/`ROLLBACK TO SAVEPOINT …` の複数文分割を解消。fixture `case_036`/`case_037`。`BEGIN` はコンテキストキーワード `transaction`/`work` か直後 `;` でのみトランザクションと判定し、Scripting ブロック `BEGIN … END`（内部 `;` 区切り）は誤分割せず verbatim 維持 … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `at_begin_transaction`
- ✅ Snowflake Scripting ブロック本体の整形（トップレベル匿名ブロック）: `[DECLARE …] BEGIN … [EXCEPTION …] END` を構造化し、本体を1文1行・インデント。`IF`/`ELSEIF`/`ELSE`/`END IF`、`FOR`/`WHILE … DO … END`、`LOOP`/`END LOOP`、`REPEAT … UNTIL … END REPEAT`、`EXCEPTION … WHEN … THEN`、ネストブロックを構造的に整形。`LET`/`:=`/`RETURN`/その他は寛容文（`;` まで）として保持。本体内の SQL（`SELECT`/`INSERT`/… ）は通常の構造的整形に委譲。新キーワード `ELSEIF`/`WHILE`/`LOOP`/`REPEAT`/`UNTIL`/`DO`/`EXCEPTION`/`CURSOR`/`RESULTSET`、新ノード `BLOCK_STMT`/`DECLARE_SECTION`/`STMT_LIST`/`IF_STMT`/`LOOP_STMT`/`EXCEPTION_SECTION`/… 。**安全性**: 寛容文は `;` まで消費するので `LET x := (CASE WHEN … END)` の式内 `END` で誤分割しない。構文が崩れたブロックはパースエラー→**ブロック全体を verbatim**（無破壊）。`BEGIN … END` トランザクションとの曖昧性は維持。fixture `case_038`/`case_039`、parser で clean-parse/ノード種別/誤分割回避/malformed-verbatim を回帰ガード … [grammar.rs](crates/snow-fmt-parser/src/grammar.rs) `block_stmt`/`if_stmt`/`loop_stmt` / [sql.rs](crates/snow-fmt-formatter/src/sql.rs) `lower_block`。残: `CASE` 文の pretty-print（現状はバランス consume でインライン保持）
- 🚧 delimiter-aware body token の言語判定 → サブフォーマッタへ委譲 → 再インデント
  - ✅ **JavaScript**: Biome の `biome_js_formatter` を組み込み（top-level `return` は synthetic function body で処理、失敗時 verbatim）
  - ✅ **Python**: Ruff の `ruff_python_formatter` を組み込み（失敗時 verbatim）
  - ✅ **Java / Scala**: brace-aware lightweight formatter（不均衡 brace/comment/string・triple string は verbatim）
  - ✅ ネストした SQL（`LANGUAGE SQL`）: `$$ … $$` body を自分自身で再帰整形

## Phase 9 — ハイライト + LSP 🚧
- ✅ Lexical highlight 基盤（keyword/type/string/comment/operator/punctuation/range、内蔵 easy fixture 全 SQL でロスレス検証） … [crates/snow-fmt-highlight/](crates/snow-fmt-highlight/)
- ✅ Hover 基盤（Snowflake 型、`CREATE PROCEDURE` の signature/returns/language、`CREATE TASK` の compute/schedule/when、procedure/task property 説明） … [crates/snow-fmt-hover/](crates/snow-fmt-hover/)
- ✅ Tree-sitter grammar baseline（Neovim/Zed/GitHub 向け token grammar、highlight/locals/injections queries、Rust wrapper、内蔵 easy fixture 全 SQL + LF/CRLF/CR/mixed 改行で cargo test 統合） … [tree-sitter-snowflake/](tree-sitter-snowflake/) / [crates/snow-fmt-tree-sitter/](crates/snow-fmt-tree-sitter/)
- ✅ CST → セマンティックトークン（highlighter から LSP legend へ、UTF-16 桁・複数行トークン分割・デルタ符号化） … [crates/snow-fmt-lsp/](crates/snow-fmt-lsp/) `semantic_tokens`
- ✅ **LSP サーバ `snow-fmt-lsp`**（stdio・`lsp-server`/`lsp-types`、同期。`formatting`＝全文整形、`semanticTokens/full`、`publishDiagnostics`＝パースエラー、`hover`＝キーワード/型/シンボル説明（`snow-fmt-hover` 配線）、`foldingRange`＝文単位。**インクリメンタル同期**（範囲編集を splice）・初期化/シャットダウン。純粋関数はユニットテスト、サーバは stdio エンドツーエンド検証） … [crates/snow-fmt-lsp/](crates/snow-fmt-lsp/)
- ✅ 診断品質: lexer/parser diagnostics は byte span（token 全体、EOF は zero-width）を持ち、`SyntaxKind::INTO_KW` ではなく `expected INTO` / `expected '('` のような人間向け表示へ変換。LSP diagnostics は lexer error（未終端 literal/comment 等）も拾い、UTF-16 range へ変換 … [diagnostics.rs](crates/snow-fmt-parser/tests/diagnostics.rs) / [kind.rs](crates/snow-fmt-syntax/src/kind.rs) `describe`
- ✅ LSP のインクリメンタル更新（`apply_change`＝範囲 splice／全文置換、`TextDocumentSyncKind::INCREMENTAL`） … [crates/snow-fmt-lsp/](crates/snow-fmt-lsp/) `apply_change`
- ✅ TextMate 文法（素のエディタ向けベースライン。`source.sql.snowflake`、comment/string/`$$…$$`/number/type/keyword/variable/operator をスコープ。キーワード・型語彙は `snow-fmt-highlight::classify` と一致をテストで機械保証＝drift しない） … [editors/textmate/](editors/textmate/)
- ✅ Tree-sitter 文法の構造化（第一層: `;` 区切りの `statement` ノードで token 列をグルーピング、第二層: 括弧/即時関数呼び出しを軽量 `expression` node 化。`folds.scm`＝文単位の折りたたみ（LSP `foldingRange` と一致）。`injections.scm` は `LANGUAGE <name> ... AS $$...$$` / `EXECUTE IMMEDIATE $$...$$` を context-aware に注入。`source_file` ルート維持で既存 highlight/locals と互換、corpus＋Rust smoke で検証） … [tree-sitter-snowflake/](tree-sitter-snowflake/)
  - 残: indents

## Phase 10 — 仕上げ・周辺 🚧
- ⏳ 🔎 Cortex / AISQL 関数（`AI_COMPLETE`, `SNOWFLAKE.CORTEX.*` 等）の認識
- ✅ CLI `snow-fmt`（`--write`/`--check`/stdin、複数ファイル/ディレクトリ再帰、`snow-fmt.toml` discovery、`--no-config`、`--line-width`/`--indent-width`/`--no-uppercase`/`--uppercase`、エンコーディング保持、error UX、`cargo install` 可、v0.1.0） … [crates/snow-fmt-cli/](crates/snow-fmt-cli/)
- 🚧 複数ファイル**並列**整形（`rayon`）は未導入。Criterion ベンチマークは [benches/format.rs](crates/snow-fmt-formatter/benches/format.rs) で導入済み
- ✅ 大規模コーパスでのべき等性・無破壊（ラウンドトリップ）回帰（内蔵 easy fixture 全 SQL で機械ガード）。残: より大きな外部コーパス
- ⏳ エディタ拡張（VS Code）パッケージング

---

### 現状サマリ（2026-06）
**Phase 0–6 は完了**、Phase 7 は主要 DDL/object DDL/access control まで実用域、Phase 9 は LSP/diagnostics/editor grammar 基盤まで、Phase 8/10 が部分。コア整形（SELECT 一式・DML・基本 DDL・object DDL・COPY・Snowflake 固有クエリ）は無破壊・べき等を property test まで含めて機械保証しつつ実用段階。CLI `snow-fmt` v0.1.0 公開可。

**残りの主な未着手（価値順）**:
1. **Phase 8 埋め込み言語**: quoted body の扱い、UDTF の `RETURNS TABLE(...)` 周辺、Java/Scala formatter の限界ケース拡張。`$$…$$` は SQL/JS/Python/Java/Scala まで対応済みで、失敗時は verbatim 保持。
2. **Phase 7 DDL の残り**: Semantic View、細かい object option のさらなる構造化（🔎 新しめは要ドキュメント確認）。
3. **Phase 5/9 の網羅強化**: `->>` の `SHOW` chain など追加ゴールデン、Tree-sitter indents。
4. **Phase 10**: `rayon` 並列、Cortex/AISQL 関数認識、VS Code 拡張、外部大規模コーパス。

回帰ゲートは `cargo test --workspace`（golden=insta、full/sql-only、lexer/parser recovery、lexical highlight、Tree-sitter、formatter べき等/ラウンドトリップ）＋ `cargo clippy --workspace --all-targets` ＋ `cargo fmt --all --check`。
