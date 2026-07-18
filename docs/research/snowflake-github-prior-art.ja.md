<!-- i18n: language-switcher -->
[English](snowflake-github-prior-art.md) | [日本語](snowflake-github-prior-art.ja.md)

# Snowflake GitHub 先行技術ノート

最終確認日: 2026-06-21。

## 公式Snowflakeノート

- Snowflakeの現在のフロー/パイプ演算子は `->>` であり、`|>` ではありません。ドキュメントでは、これが唯一のサポートされているフロー演算子として説明されており、ステートメントチェーンは `<sql_statement_1> ->> <sql_statement_2>` のように示されています。出典: https://docs.snowflake.com/en/sql-reference/operators-flow
- Snowflakeのリリース9.13は2025年5月にパイプ演算子を導入しました。出典: https://docs.snowflake.com/en/release-notes/2025/9_13
- 2026年のリリースノートでは、ユーザー定義型、インターバルデータ型、プロシージャスコープの一時テーブル構文、仮想列、セマンティックビューの変更、Cortex AI関数の変更など、パーサー関連のSQL表面領域が引き続き追加されています。出典: https://docs.snowflake.com/en/release-notes/new-features-2026

## チェックしたGitHubプロジェクト

- `sqlfluff/sqlfluff` — Snowflakeサポートとdbt/Jinjaの認識を持つ、方言柔軟なSQLリンター/フォーマッター。広範な方言カバレッジとルールの整理に役立つ参考資料ですが、Pythonであり、高度に構成可能です。出典: https://github.com/sqlfluff/sqlfluff
- `tobymao/sqlglot` — Snowflakeを含む31の方言を持つ、依存関係のないPythonパーサー/トランスパイラー/オプティマイザー。方言機能フラグ、パーサーオーバーライド、広範なコーパステストの参考資料として役立ちます。出典: https://github.com/tobymao/sqlglot
- `DerekStride/tree-sitter-sql` — 一般的/許容的なTree-sitter SQL文法。エディタのハイライトのトレードオフに関する参考資料として役立ちます: 許容的なパース、生成されたパーサー配布、既知のSQLハイライトのエッジケース。出典: https://github.com/DerekStride/tree-sitter-sql
- `sql-formatter-org/sql-formatter` — Snowflakeサポートを持つJavaScriptプリティプリンタですが、ストアドプロシージャを明示的にサポートしていません。フォーマッターUXの参考資料として役立ち、Snowflakeスクリプティング/プロシージャがsql-dialect-fmtのコアの差別化要因であることを警告します。出典: https://github.com/sql-formatter-org/sql-formatter
- `tobilg/polyglot` — 32以上の方言に対応したRust/Wasm SQLパーサー/トランスパイラー/フォーマッターで、sqlglotに触発されています。Rust側のAST/ビジター/スタックセーフティパターンを観察するのに役立ちます。出典: https://github.com/tobilg/polyglot

## チェックした研究

- SQLFlex (SIGMOD/PACMMOD 2026) は、方言特有の構文が文法ベースのSQLツールの主要な失敗モードであると主張し、未知の方言フラグメントを検証されたセグメンテーションの背後に隔離することを提案しています。sql-dialect-fmtはコアのレキサー/パーサーを決定論的に保つべきですが、同じ教訓が運用上も適用されます: Snowflake特有のボディの周りでロスレスなトークン/範囲を保持し、方言の変動を構成とテストで明示的にすること。出典: https://arxiv.org/abs/2603.16155

## sql-dialect-fmtの設計への影響

- レキサーをロスレスかつ許容的に保つ。SQLFluff/tree-sitter-sqlは、方言のエッジを生き残る価値を示しています; sql-dialect-fmtは、文法サポートが遅れている場合でもトークン/範囲を返し続けるべきです。
- 可能な限りデータ駆動型の方言変更を維持する。新しい `FLOW_PIPE` トークンと動的なイージーテストフィクスチャの発見はその例です: Snowflakeが構文を追加すると、テストはハードコーディングされたケースリストなしで成長できます。
- Snowflake特有の利点を保持する: SQLスクリプティングと埋め込まれたJavaScript/Pythonボディ。既存のフォーマッターはプロシージャを十分にサポートしていないことが多い; 埋め込まれたイージーフィクスチャクレートは常にオンの回帰ゲートであり、外部生成コーパスはオプションのままであるべきです。
- ハイライトはレキシカルからセマンティックに開始するべきです。新しい `sql-dialect-fmt-highlight` クレートは現在安定した範囲/スコープを提供します; 後のLSPセマンティックトークンは、置き換えることなくパーサーコンテキストを上に重ねることができます。