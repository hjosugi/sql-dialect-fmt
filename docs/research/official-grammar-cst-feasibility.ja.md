<!-- i18n: language-switcher -->
[English](official-grammar-cst-feasibility.md) | [日本語](official-grammar-cst-feasibility.ja.md)

# 公式 grammar からの Pure Rust CST parser 生成の feasibility

最終確認: 2026-09-24。追跡 issue: [#121](https://github.com/hjosugi/sql-dialect-fmt/issues/121)。
初回調査: 2026-07（[issue コメント](https://github.com/hjosugi/sql-dialect-fmt/issues/121)）。

## 問い

機械可読な公式 grammar を generator に入力して、本プロジェクトが必要とする Pure Rust の
lossless CST parser を生成し、手書きの event/rowan parser を置き換えられるか。今できないなら、
何が変われば可能になるか。

生成 parser は、現行 parser が保証していることをすべて満たす必要がある。

1. **byte 単位で lossless な CST。** 空白やコメントを含む全 byte が木の token になる
   （`event.rs` が build 時に trivia を再挿入する）。
2. **total なエラー回復。** どんな入力にも木と diagnostic を返し、壊れた範囲も捨てずに保持する
   （`parser.rs`、fuel 付き）。
3. **位置依存の contextual keyword。** grammar 上の位置が決めた場所でだけ keyword として扱う
   （`contextual.rs`: `AT`、`MATCH_RECOGNIZE`、`GROUPING SETS`、`COMMENT ON` など）。
4. **Pure Rust で Wasm に載り、CLI・LSP・VS Code 拡張・Wasm build に十分な速度と保守性。**

## 結論

- **再評価トリガーは今も未成立。** 2026-09-24 時点で、Snowflake も Databricks も自社 parser の
  元になっている grammar を公開していない。[ROADMAP.md](../../ROADMAP.md)（研究開発 5）の
  トリガーは変わらない。
- **手書き event/rowan parser を source of truth として維持し**、第三者 grammar は conformance
  oracle として使い続ける（`scripts/grammar-oracle-report.py`、週次の `External grammar oracles`
  job）。直近の定期実行（2026-09-21）は成功で、grammars-v4 Snowflake examples 51 件と Spark SQL
  test inputs 373 件のすべてが lossless / べき等 harness を通過した。
- **公式 grammar が出ても、そのまま generator に入れれば済むわけではない。** 唯一の成功例
  `postgresql-cst-parser` は、PostgreSQL の実装 grammar のために lexer generator・parser
  generator・automata の crate を自作している。公開 grammar は必要条件であって十分条件ではない。

## 1. 機械可読な grammar を公開している dialect

### 本プロジェクトが整形する dialect

| ソース | dialect | 形式 | ライセンス | 実装 grammar か | 活動状況 | 備考 |
| --- | --- | --- | --- | --- | --- | --- |
| [Snowflake SQL reference](https://docs.snowflake.com/en/sql-reference) | Snowflake | HTML の構文ブロック | docs | いいえ | 継続更新 | 唯一の公式記述。grammar file ではなく、Snowflake の parser の生成元でもない。 |
| [Snowflake-Labs/lezer-snowsql](https://github.com/Snowflake-Labs/lezer-snowsql) | Snowflake | Lezer grammar + 828 行の JS tokenizer | Apache-2.0 | いいえ | 最終 push 2023-12-04 | Snowflake Labs の editor 用 grammar で、サポート対象の製品ではない。README では `SELECT`・`ALTER`・`INSERT`・`DELETE`・`MERGE`・`SET`・`CALL` が未実装のまま。 |
| [antlr/grammars-v4 `sql/snowflake`](https://github.com/antlr/grammars-v4/tree/master/sql/snowflake) | Snowflake | ANTLR4 | MIT（file header） | いいえ（docs からのコミュニティ作成） | 活発（`7e08234262`、2026-09-21） | parser rule 699、lexer rule 980。action なし（grammars-v4 の方針）。既に oracle として使用中。 |
| [fivetran/zetasql-snowflake](https://github.com/fivetran/zetasql-snowflake) | Snowflake | ZetaSQL fork | Apache-2.0 | いいえ | 最終 push 2023-09-25 | 停滞した analyzer fork。 |
| [bytebase/omni](https://github.com/bytebase/omni) | Snowflake ほか | 手書きの Go 再帰下降 | MIT | いいえ | 活発 | grammar ではなくコード。 |
| [Apache Spark `SqlBaseParser.g4` / `SqlBaseLexer.g4`](https://github.com/apache/spark/tree/master/sql/api/src/main/antlr4/org/apache/spark/sql/catalyst/parser) | Databricks（Spark 経由） | Java action 付き ANTLR4 | Apache-2.0 | Apache Spark としては実装そのもの。Databricks Runtime は fork で、その拡張は含まれない | 活発（`4cfbd41196`、2026-09-24） | parser rule 313、lexer rule 527。既に oracle として使用中。 |
| [sqlfluff dialects](https://github.com/sqlfluff/sqlfluff/tree/main/src/sqlfluff/dialects) | Snowflake、Databricks | Python の segment DSL | MIT | いいえ | 活発 | Python コードとしてのみ読める。keyword / segment checklist に使用中。 |

### 実装 grammar を公開している dialect（比較用）

| dialect | grammar | ライセンス | 示していること |
| --- | --- | --- | --- |
| PostgreSQL | `gram.y` + `scan.l`（Bison/Flex） | PostgreSQL License | [`postgresql-cst-parser`](https://github.com/future-architect/postgresql-cst-parser) を可能にした前提条件。 |
| GoogleSQL / BigQuery | [`googlesql/parser/googlesql.tm`](https://github.com/google/googlesql)（Textmapper。旧 ZetaSQL） | Apache-2.0 | 公式の実装 grammar があっても、Rust backend のない generator（Textmapper）向けであり得る。 |
| Trino | [`core/trino-grammar/.../SqlBase.g4`](https://github.com/trinodb/trino) | Apache-2.0 | Spark と同じ形。ANTLR4 に Java の runtime hook が付く。 |

## 2. スパイク: generator が受け取らなければならないもの

週次 oracle report に **Generator Hazards** 表を追加した
（[docs/CORPUS.ja.md](../CORPUS.ja.md#外部grammar-oracle)）。data ではなくコードである構成要素、
つまり Rust generator が grammar からそのまま受け取れないものを数える。上記の revision での値:

| grammar | parser rule | lexer rule | semantic predicate | inline action | named action | lexer mode | tree 外の trivia | keyword fallback rule（選択肢数） |
| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- |
| grammars-v4 Snowflake | 699 | 980 | 0 | 0 | — | 0 | 4 | `non_reserved_words` 142、`keyword` 52 |
| Apache Spark SQL | 313 | 527 | 28 | 8 | `@header`、`@members` | 1 | 3 | `nonReserved` 445、`ansiNonReserved` 379、`strictNonReserved` 16 |
| Snowflake-Labs lezer-snowsql | rule 361 | — | — | — | — | — | `@skip` block | JS tokenizer に依存する external specializer 2 |

数値の意味:

- **どの候補も設計上 trivia を捨てる。** 2 つの ANTLR grammar は空白とコメントを
  `channel(HIDDEN)` に送り、Lezer は `@skip` で読み飛ばす。要件 1 の正反対で、生成 parser には
  独自の trivia 層が必要になる。それはまさに `event.rs` が既に実装している部分である。
- **Spark の実装 grammar は Java と組み合わせて初めて完結する。** 28 個の predicate は
  `@members` で宣言された runtime の SQL 設定を参照する: `SQL_standard_keyword_behavior`（ANSI
  mode）、`legacy_setops_precedence_enabled`、`legacy_exponent_literal_as_decimal_enabled`、
  `double_quoted_identifiers`、`parameter_substitution_enabled`、`legacy_identifier_clause_only`。
  さらに `isValidDecimal()`・`isHint()`・`isShiftRightOperator()`・`isOperatorPipeStart()` などの
  helper を呼ぶ。これらはすべて手で移植し、同期し続ける必要がある。
- **keyword の扱いは位置ではなく静的な一覧。** Spark は ANSI mode に応じて 3 つの fallback
  一覧（選択肢 445 / 379 / 16）を切り替える。grammars-v4 は選択肢 142 の `non_reserved_words`
  rule を 1 つ持つ。どちらも「`AT` は table 参照の後でだけ keyword」を表現できないが、
  `contextual.rs` は位置単位でこれを扱っている。
- **grammars-v4 Snowflake は機械的には取り込める**（action なし）が、コミュニティが docs から
  書いたもので、Snowflake のリリースへの追随は保証されない。トリガーは満たさない。

`Corpus` workflow と同じ sparse checkout で再現できる:

```sh
scripts/grammar-oracle-report.py \
  --grammars-v4 /path/to/grammars-v4 \
  --spark /path/to/spark \
  --sqlfluff /path/to/sqlfluff
# target/grammar-oracle-report.md の "Generator Hazards" 節を参照
```

## 3. Rust 側の選択肢（crates.io、2026-09-24 時点）

| ツール | 最新 | grammar 入力 | trivia の lossless 保持 | エラー回復 | contextual keyword | 適合性 |
| --- | --- | --- | --- | --- | --- | --- |
| [tree-sitter](https://crates.io/crates/tree-sitter) | 0.27.0（2026-08-30） | 独自の JS DSL、生成物は C | コメントは `extras` として浮く | ヒューリスティックで、バージョン間で変わり得る | external scanner（C） | ハイライト用にのみ保持し、現在は保留中（ROADMAP）。C runtime は Pure Rust / Wasm 方針に反する。 |
| [antlr4rust](https://crates.io/crates/antlr4rust) | 0.5.2（2025-10-25）。旧 `antlr-rust` は 0.2.2（2022）で停止 | ANTLR4 `.g4` | hidden channel で木に載らない | ANTLR 標準の sync/recover | predicate を Rust で書き直す必要 | ANTLR 公式 target ではなく、生成に JVM ツールが要る。2025-10 に保守再開したが実績は薄い。 |
| [lrpar / cfgrammar](https://crates.io/crates/lrpar)（grmtools） | 0.15.0（2026-07-22） | Yacc/Bison 形式の `.y`（"mostly unchanged" で使える） | lexer 次第。独自層が必要 | CPCT+ による自動 repair sequence | lexer / grammar 側の工夫 | **ベンダーが Bison/Yacc の実装 grammar を公開した場合（`gram.y` シナリオ）に最も汎用的に適合する。** repair は token の挿入 / 削除列なので、全 byte を保持する diagnostic へ変換する層は別途必要。 |
| [lalrpop](https://crates.io/crates/lalrpop) | 0.23.1（2026-03-11） | 独自の LR DSL | なし | 限定的 | なし | Ruff が v0.4.0（2024）で速度と回復のために離脱した。 |
| [pest](https://crates.io/crates/pest) / [chumsky](https://crates.io/crates/chumsky) | 2.9.2 / 0.13.0 | 手書きの PEG / combinator | なし / 手書き | なし / あり | 手書き | grammar を手で書き直すので、公式 grammar の利点がない。 |
| [rowan](https://crates.io/crates/rowan) / [cstree](https://crates.io/crates/cstree) | 0.17.0 / 0.14.0 | 木 library | あり | 対象外 | 対象外 | 直交する。どの生成 parser もこのどちらかを組み立てることになる。本プロジェクトは `rowan`、`postgresql-cst-parser` は `cstree`。 |
| 手書き event/rowan（現行） | — | Rust コード | あり（要件 1） | total（要件 2） | 位置依存（要件 3） | rust-analyzer・Biome・Ruff と同じ設計。 |

## 4. 前例: `postgresql-cst-parser`

[`future-architect/postgresql-cst-parser`](https://github.com/future-architect/postgresql-cst-parser)
（MIT、crates.io で 0.2.0、最終 push 2026-06-23）は、公式 SQL grammar から生成された唯一の
Pure Rust CST parser である。必要だったもの:

- PostgreSQL 自身の `gram.y` と `scan.l` に、[libpg_query](https://github.com/pganalyze/libpg_query)
  の patch を当てたもの;
- Rust 向けに手で書き直した `scan.l`;
- repository 内で自作した 3 つの crate（`lexer-generator`、`parser-generator`、`automata`）。
  これらが parse table と、`gram.y` の rule 名に沿った node を持つ `cstree` の木を作る。

公開 API の `parse()` は `Result` を返し、壊れた入力に対して木を返すことは約束しない。正しい SQL
を整形する PostgreSQL formatter にはそれで十分だが、LSP や editor 拡張に必要な要件 2 には届かない。

## 5. 結論が変わる条件

| 事象 | 対応 |
| --- | --- |
| Snowflake が自社 parser の生成元 grammar を公開する（形式は問わない） | 1 つの文 family で試作する: parser を生成し（Yacc/Bison なら `lrpar`）、既存の event 層で CST を組み立て、fixture での lossless round-trip、malformed suite との回復 diagnostic の一致、現行 parser と比べた Wasm のサイズと速度を測る。4 要件をすべて満たす場合のみ移行する。 |
| Databricks が Runtime の grammar を公開する | 同じ試作を行う。上記の Spark 型 predicate の移植と、ANSI mode による keyword 一覧の分岐の工数を見込む。 |
| 新しいコミュニティ grammar、docs 由来の ANTLR grammar、実装ではない公式 grammar | トリガーではない。conformance oracle として追加する。 |

## 6. この文書を最新に保つ方法

- 週次の `External grammar oracles` job が upstream `HEAD` で hazard を数え直し、corpus 結果と
  一緒に artifact として保存する。predicate / action の急増や新しいソースの出現が、この文書を
  見直す合図になる。
- Snowflake の release notes が parser や grammar tooling に触れたら、上記の Snowflake 側ソースを
  再確認する。コミュニティのソースが保守されているかは
  `gh api repos/<owner>/<repo> --jq .pushed_at` で確認できる。
- トリガーそのものは [ROADMAP.md](../../ROADMAP.md)（研究開発 5）にある。
