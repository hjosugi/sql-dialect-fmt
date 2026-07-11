<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# Snowflake SQL

Visual Studio Code用のSnowflake SQL構文ハイライト。これは、[sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt)で使用されるのと同じキーワードと型定義に基づいています。

![Snowflake SQL構文ハイライト](images/syntax-highlighting.png)

## 機能

- Snowflake SQLキーワードと組み込み型
- Snowflakeスクリプティングと`$$ ... $$`ルーチン本体
- 行コメント（`--`, `//`）とブロックコメント（`/* ... */`）
- 文字列、引用識別子、数値リテラル、および演算子
- 位置変数`$1`、セッション変数`$name`、バインド変数`:name`、および`?`
- `@stage`、`@~`、`@%table`、および名前空間付きステージ参照
- `.sql`、`.snowsql`、および`.sfsql`ファイルの関連付け

- **言語ID:** `snowflake-sql`
- **スコープ名:** `source.snowflake-sql`
- **ファイルタイプ:** `.sql`、`.snowsql`、`.sfsql`

キーワードと型の単語リストは、`sql-dialect-fmt-highlight`のテスト（`tests/textmate.rs`）によってフォーマッタのレクサー/ハイライターと連動して維持されています。文法がキーワードまたは型としてスコープするすべての単語は、`sql_dialect_fmt_highlight::classify`によって同じように分類されなければならないため、文法はツールチェーンの他の部分から逸脱することはできません。

## 使用方法

1. 拡張機能をインストールします。
2. `.sql`、`.snowsql`、または`.sfsql`ファイルを開きます。
3. 必要に応じて、**言語モードの変更**を選択し、**Snowflake SQL**を選択します。

この拡張機能は構文ハイライトと言語メタデータを提供します。SQLを実行したり、Snowflakeに接続したり、ブラウザフォーマッタを含んだりすることはありません。CLIフォーマッティングやその他の統合については、[メインプロジェクトのREADME](https://github.com/hjosugi/sql-dialect-fmt#readme)を参照してください。

## プライバシー

この拡張機能には、ランタイムコード、テレメトリー、分析、ネットワークリクエスト、またはリモートフォーマッティングは含まれていません。静的な言語設定とTextMate文法ファイルのみを提供します。プライバシーポリシーについては、[プライバシーポリシー](https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md)を参照してください。

## サポートとソース

- [問題を報告する](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [ソースコード](https://github.com/hjosugi/sql-dialect-fmt)
- ライセンス: [0BSD](LICENSE.md)