<!-- i18n: language-switcher -->
[English](TESTING.md) | [日本語](TESTING.ja.md)

# テスト

テストスイートは意図的に層状になっています。変更が失敗した場合、失敗したクレートは後退した層を指し示すべきです。

## 標準ゲート

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Tree-sitter文法チェック:

```sh
cd tree-sitter-snowflake
npm exec --package tree-sitter-cli@0.26.9 -- tree-sitter generate
npm exec --package tree-sitter-cli@0.26.9 -- tree-sitter test
```

## ファジング

カバレッジガイドのファジングは除外された `fuzz/` クレートに存在し、通常のワークスペースチェックは迅速かつ自己完結しています。スケジュールされた `Fuzz` ワークフローは、同じターゲットを毎週実行し、`fuzz/artifacts/` と失敗したターゲットの生成されたコーパスをアップロードします。

```sh
cargo install cargo-fuzz --locked
cargo +nightly fuzz run lexer_roundtrip
cargo +nightly fuzz run parser_lossless
cargo +nightly fuzz run formatter_idempotent
```

制限付きのローカルスモーク実行のためには:

```sh
cargo +nightly fuzz run parser_lossless -- -max_total_time=60
```

## どこで何をテストするか

共有ヘルパー:

- 機械的な不変条件を `sql-dialect-fmt-test-support` に置く
- 個々のテストファイルは名前付きケーステーブルと期待される動作に集中させる
- 失敗がフィクスチャの名前を示すように、コンテキストを持つヘルパーを好む

エンコーディング:

- UTF-8、UTF-8 BOM、および UTF-16 LE/BE BOM の往復バイト単位
- 無効またはサポートされていないバイトは不透明のままであり、書き換えられない
- CLIテストはファイル境界でのエンコーディング保持をカバーするべき

レキサー:

- すべてのバイトは正確に1つのトークンでカバーされる
- トークンテキストは元の入力に戻る
- 終端のない文字列/コメントは診断を生成し、パニックを引き起こさない
- 区切り文字の変更はテーブル駆動であり、変数/演算子を飲み込まない
- LF、CRLF、古いMac CR、および混合行末

パーサー:

- サポートされている文法のCST形状
- 不完全なSQLに対する回復力のあるリカバリー
- 壊れた入力と有効な入力のためのロスレス往復
- 二次的な動作を露呈する可能性のある長い入力

ハイライト:

- エディタアダプター用の安定したキャプチャカテゴリ
- Unicodeおよび混合改行にわたるバイト範囲
- Snowflake特有の演算子と型

ホバー:

- ホバーされたトークンまたは宣言名の範囲選択
- 手続き、タスク、型、およびプロパティの簡潔な要約
- 編集中の壊れたSQLはパニックを引き起こさないべき

Tree-sitter:

- 公開文法の動作に対するコーパス例
- クエリコンパイル
- 重要なハイライトスコープのための実際のキャプチャ実行
- 文法変更と共にコミットされた生成された `src/parser.c` と `src/node-types.json`
- コーパスと生成されたパーサーファイルに反映されたボディ区切りルールの変更

## フィクスチャポリシー

`cargo test --workspace` は自己完結でなければなりません。安定したキュレーションされた例を `crates/sql-dialect-fmt-test-fixtures` に保持してください。

キュレーションされたSQLフィクスチャは `sql-dialect-fmt-test-fixtures` に保存され、`EASY_CASES` を通じて公開されます。このコーパスは常にオンの最小ゲートであり、全体の品質基準ではありません。現在の最小カウントは `MINIMUM_EMBEDDED_EASY_CASES` に保持されているため、新しいフィクスチャはすべての消費者テストを更新する必要はありません:

- CLIテストはゴールデンフィクスチャの発見とプロファイルマッピングを検証します。
- レキサー/ハイライト/Tree-sitterテストは、すべての埋め込まれたフィクスチャがクリーンでロスレスであることを要求します。
- パーサーフィクスチャテストは、すべての埋め込まれたフィクスチャがロスレスに回復することを要求します。文法サポートが追加されると、焦点を当てた `clean` パーサーテストを追加します。
- バグがフィクスチャよりも特定のときは、その動作を所有するクレートの隣に狭いテーブル駆動テストを追加します。

より広範な生成コーパスはリポジトリの外に留まるべきです。生成されたフィクスチャディレクトリをコミットするのではなく、1回限りのローカルチェックにはCLIの `--fixtures` フラグを使用してください。

フォーマッターコーパスチェックには、常にオンの追加レイヤーがあります:
`crates/sql-dialect-fmt-formatter/tests/corpus_sample/`。これらのファイルはフォーマッターの標準形でコミットされ、`external_corpus.rs` によって冪等性、重要なトークンの保持、およびクリーンな再解析がチェックされます。より大きなローカルまたはプライベートコーパスは `SQL_DIALECT_FMT_EXTERNAL_CORPUS` を使用するべきです。詳細は `docs/CORPUS.md` を参照してください。