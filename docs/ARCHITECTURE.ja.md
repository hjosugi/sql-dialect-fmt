<!-- i18n: language-switcher -->
[English](ARCHITECTURE.md) | [日本語](ARCHITECTURE.ja.md)

# アーキテクチャ

sql-dialect-fmtは、小さなレイヤーに分割されており、貢献者が一度に1つの懸念に取り組むことができます。フォーマッターはまだ実装されていませんが、現在のベースは言語フロントエンドとエディタ向けのメタデータです。

## 設計目標

- ロスレス: コメント、空白、および壊れた入力が保持されます。
- レジリエント: パーサーとエディタ機能は、ユーザーが入力している間も動作し続けます。
- デフォルトで高速: ホットパスは不要な割り当てを避けます。
- スノーフレークファースト: `->>`、半構造化パス、ステージ、手続き、タスク、および埋め込みボディなどの機能はファーストクラスです。
- 公開が簡単: Tree-sitter、ホバー、ハイライト、CLI、および将来のLSPの部品は、1つの絡まったクレートではなく、別々のパッケージです。

## レイヤーマップ

```text
source SQL
  |
  v
sql-dialect-fmt-encoding     bytes -> UTF-8 text, or opaque bytes when unsafe
  |
  v
sql-dialect-fmt-lexer        lossless tokens + lexical diagnostics
  |
  +--> sql-dialect-fmt-highlight    lexical token classification
  |
  +--> sql-dialect-fmt-hover        editor hover summaries
  |
  v
sql-dialect-fmt-parser       resilient rowan CST
  |
  v
future formatter/LSP  Doc IR, semantic tokens, diagnostics

tree-sitter-snowflakeは、Tree-sitterを直接消費するツールのための並行エディタ文法です。意図的に許容的でトークン中心です。
```

## 構文とレキサー

`sql-dialect-fmt-encoding`はCLI/ファイル境界を所有します。UTF-8、BOM付きのUTF-8、およびBOM付きのUTF-16 LE/BEを検出し、編集されたテキストを元のエンコーディングに戻すことができます。無効またはサポートされていないバイトストリームは不透明なままで、バイトとして往復します。フォーマッターレイヤーはエンコーディングを推測して書き換えてはいけません。

`sql-dialect-fmt-syntax`は共有語彙を所有します：

- `SyntaxKind`内のトークンとノードの種類
- 大文字と小文字を区別しないキーワード検索
- rowan言語の接着剤

`sql-dialect-fmt-lexer`は手書きです。退屈で予測可能なままであるべきです：

- バイトに対して1回のパス
- ホットパスでの正規表現なし
- トークンごとの割り当てなし
- UTF-8の境界の外でスライスしない
- 停止せずに構文エラーを報告する

最も強い不変条件: すべてのトークンテキストを連結することは、入力を正確に再現しなければなりません。

埋め込み手続き/関数ボディは、小さな区切りテーブル（`BodyDelimiter` + `LexOptions`）を通じて認識されます。現在のスノーフレークはロスレスボディトークンに`$$...$$`を使用しています。将来の区切りはデータとして追加されるべきであり、レキサーの状態をクローンすることによって追加されるべきではありません。詳細は[docs/research/delimiter-strategy.md](research/delimiter-strategy.md)を参照してください。

## パーサー

`sql-dialect-fmt-parser`はイベントを通じてrowan CSTを構築します。パーサーは悪いSQLをパニックに変えてはいけません。未知または不完全な入力は、ソースバイトを保持しながらツリー内でエラーになります。

パーサーテストを使用して：

- 優先順位と構造
- 不完全な構文の周りの回復
- 行末の保持
- 遅いパスを露呈する可能性のある長い入力

## エディタ機能

`sql-dialect-fmt-highlight`はレキサーから始まり、完全なパーサーがすべてのスノーフレーク構造を知る前に機能することができます。

`sql-dialect-fmt-hover`はLSPに依存しません。バイト範囲、タイトル、本文、種類、およびオプションのドキュメントURLを持つ小さな`Hover`モデルを返します。将来のLSPサーバーは、このモデルを複製するのではなく適応させるべきです。

`tree-sitter-snowflake`はエディタとコードホストのためのものです。新しいスノーフレーク構文の下で堅牢であるべきです。折りたたみ、注入、選択などの実際のエディタ機能を改善する場合にのみ、構造を徐々に追加してください。

## スノーフレーク構文の追加

1. `sql-dialect-fmt-lexer`でトークンサポートを追加または確認します。
2. `sql-dialect-fmt-syntax`または`sql-dialect-fmt-highlight`でキーワード/タイプ分類を追加します。
3. 構造が重要な場合にのみパーサーサポートを追加します。
4. エディタフィードバックを改善する場合はホバー/クエリサポートを追加します。
5. 集中テストを追加し、PRにスノーフレークドキュメントのリンクを含めます。