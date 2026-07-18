<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# tree-sitter-snowflake

`sql-dialect-fmt` のための Snowflake SQL の Tree-sitter 文法。

この文法は意図的にトークン中心です。Snowflake は急速に変化しており、ロスレス Rust CST パーサーがフォーマッターの真実のソースとして残ります。Tree-sitter 文法は、エディタに対してハイライト、選択、ホバーの配管のための堅牢なベースラインを提供し、新しい Snowflake 構文を拒否することなく機能します。

その平坦なトークンストリームの上に、文法はエディタ指向の構造的な二つのレイヤーを追加します：

- 各トップレベルステートメント（`；` までのトークンの連続）は、折りたたみとナビゲーションのために `statement` ノードにグループ化されます；
- バランスの取れた括弧と即時関数呼び出し構文は、軽量の `expression` ノード（`call_expression`、`parenthesized_expression`）の下にグループ化されます。

クエリセットは、Snowflake の `LANGUAGE <name> ... AS $$...$$` 本体や `EXECUTE IMMEDIATE $$...$$` のためのコンテキスト認識インジェクションも提供します。文法は依然として完全なフォーマッタ文法を目指していません；未知の Snowflake 構文はトークン化され、解析可能なままであるべきです。インデントは今後の作業です。

## 開発

```sh
npm install
npm run generate
npm test
```

生成された C パーサーは Rust ラッパークレート `crates/sql-dialect-fmt-tree-sitter` によって消費されます。

## 公開形状

このディレクトリは、Tree-sitter 文法を直接消費するエディタやツールのための `tree-sitter-snowflake` 文法パッケージとして公開できます。

Rust ワークスペースは、Rust/LSP 消費者のために公開するクレートである `sql-dialect-fmt-tree-sitter` を通じて同じ生成されたパーサーを公開します。エディタプラグインは、この文法の上に薄いアダプターであるべきです：

- Neovim/Helix/Zed: 文法 + `queries/*.scm`
- VS Code: 文法をバンドルし、スコープをテーマにマッピングする拡張パッケージ
- LSP ホバー/セマンティックトークン: ロスレス CST パーサーを真実のソースとして使用し、Tree-sitter ノード範囲を迅速なエディタのベースラインとして使用

これは Snowflake Marketplace/Native App パッケージではなく、Snowflake SQL のためのエディタ/ツールプラグインのパスです。