<!-- i18n: language-switcher -->
[English](delimiter-strategy.md) | [日本語](delimiter-strategy.ja.md)

# デリミタ戦略

Snowflakeのプロシージャ/関数の本体は、セミコロン、コメント、文字列、埋め込み言語を含むため、扱いが難しいです。フォーマッタは、外部SQLをレキシングする際に、それらの本体を分割したり再解釈したりしてはいけません。

## 現在のSnowflakeの動作

Snowflakeはプロシージャの本体を文字列のようなプロシージャ定義として文書化しています。SnowSQL、Snowsight、Snowflake CLI、およびPythonコネクタのコンテキストにおいて、Snowflakeはプロシージャ定義の周りにある文字列リテラルデリミタ`'`および`$$`を明示的に示しています。

ソース:

- CREATE PROCEDURE: https://docs.snowflake.com/en/sql-reference/sql/create-procedure
- Snowflakeスクリプティングクライアントデリミタ:
  https://docs.snowflake.com/en/developer-guide/snowflake-scripting/running-examples

したがって、sql-dialect-fmtは`$$...$$`を1つのロスレス本体トークンとして扱います。シングルクォートのプロシージャ本体は、レキサーレイヤーでは通常のSQL文字列として残り、パーサーは後で`AS`の後の文字列がプロシージャ本体であると判断できます。

## 先行技術の教訓

- sqlglotは、引用符、識別子、コメント、生の文字列、およびヒアドキュメント文字列をダイアレクト設定されたトークナイザーテーブルとして保持します。これはデリミタの変更に対して適切な形です：データが先、状態遷移機械が後です。
  ソース: https://github.com/tobymao/sqlglot/blob/main/sqlglot/tokens.py
- sqlfluffはSnowflakeの文法をダイアレクトレイヤーに保持し、ロスレスセグメントモデルを使用しており、ダイアレクト特有の本体構造がフォーマッタルールに散在しないように強化しています。
  ソース:
  https://github.com/sqlfluff/sqlfluff/blob/main/src/sqlfluff/dialects/dialect_snowflake.py
- tree-sitterの文法は静的ですが、grammar.jsは本体デリミタルールを1つのリストに保持できるため、追加が局所化されます。
- sqlparser-rsは有用な構文参照ですが、そのASTは埋め込み本体のフォーマッタ所有権には十分なロスレス性を持っていません。
- 最近のダイアレクト非依存のSQLパース研究（SQLFlex、2026）も保守的な分割を強化しています：既知の文法/トークンアンカーを決定論的に保ち、ダイアレクト特有またはまだサポートされていないフラグメントを隔離し、新しい構文をコアパーサーに推測するのではなく、検証フックを保持します。
  ソース: https://arxiv.org/abs/2603.16155

## sql-dialect-fmtルール

レキサーは本体デリミタの認識を担当します。以下を公開します：

- `BodyDelimiter`: オープナー/クローザー/名前
- `LexOptions`: 本体デリミタのテーブル
- `DEFAULT_BODY_DELIMITERS`: 現在は`$$...$$`

デフォルトの動作は現在のSnowflakeと一致する必要があります。将来のデリミタ候補は、Snowflakeが文書化するまでオプトインでなければなりません。なぜなら、`$name`は有効なSnowflake変数形式であり、推測的な`$tag$`サポートはトークン化を変更する可能性があるからです。

## 将来のデリミタを追加する

1. Snowflakeが文書化した後にのみ、`DEFAULT_BODY_DELIMITERS`に`BodyDelimiter`を追加します。
2. 新しい本体がロスレスであり、既存の変数/演算子を飲み込まないことを示す集中したレキサーテストを追加します。
3. `BODY_DELIMITER_RULES`内の`tree-sitter-snowflake/grammar.js`を更新します。
4. Tree-sitterパーサーファイルを再生成します。
5. デリミタがトークン化を超えてユーザーに見える動作を変更する場合にのみ、ハイライト/ホバー/パーサーテストを追加します。

これにより、デリミタの変動がパーサーの回復、フォーマッタルール、ホバーテキスト、およびエディタアダプタから遠ざけられます。