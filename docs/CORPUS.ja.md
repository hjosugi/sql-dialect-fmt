<!-- i18n: language-switcher -->
[English](CORPUS.md) | [日本語](CORPUS.ja.md)

# コーパス回帰ハーネス

コーパスハーネスは、フォーマッタがこれまで見たことのない大規模な実世界のSQLに対して誠実であることを保証します。任意のSQLに対して特定のフォーマットスタイルを主張するのではなく、すべての入力に対して保持されるべき不変条件を主張します：

1. `parse`は決してパニックを起こさない。
2. フォーマットは重要な非トリビアトークンのケース折りたたまれたストリームを保持する。
3. `format(format(x)) == format(x)`。
4. クリーンな入力をフォーマットすると、クリーンに再解析できる出力が得られる。

チェックは `crates/sql-dialect-fmt-formatter/tests/external_corpus.rs` にあります。同じ `check_file` 関数が常時オンのサンプルコーパスとオプトインの外部コーパスの両方をサポートしているため、2つのパスはずれません。

## 常時オンのサンプルコーパス

`crates/sql-dialect-fmt-formatter/tests/corpus_sample/` には、小規模なキュレーションされた `.sql` ファイルのセットが含まれています：

| ファイル | カバーする内容 |
| --- | --- |
| `01_select.sql` | CTE、結合、ウィンドウ関数、半構造化アクセス |
| `02_dml.sql` | `INSERT` / `UPDATE` / `DELETE` / `MERGE` |
| `03_ddl.sql` | `CREATE TABLE` / `VIEW` / `WAREHOUSE` |
| `04_copy.sql` | `COPY INTO` のロードとアンロード |
| `05_scripting.sql` | SQL本体を持つSnowflakeスクリプティング手続き |
| `06_semantic_view.sql` | `CREATE SEMANTIC VIEW` |

`sample_corpus_is_clean` は `cargo test --workspace` の一部として実行されます。上記の不変条件に加えて、各コミットされたサンプルはすでにフォーマッタの標準形式である必要があります：`format(x) == x`。

意図的なフォーマット変更後にサンプルを再生成するには、次のコマンドを使用します：

```sh
cargo run -p sql-dialect-fmt --bin sql-dialect-fmt -- --write --no-config \
  crates/sql-dialect-fmt-formatter/tests/corpus_sample
```

このセットは小さく、代表的であることを保ってください。広範な生成またはプライベートコーパスは、以下の外部ハーネスの背後に置くべきです。

## 外部コーパス

ハーネスを1つ以上のローカルファイルまたはディレクトリにポイントします：

```sh
SQL_DIALECT_FMT_EXTERNAL_CORPUS=/path/to/sqls \
  cargo test -p sql-dialect-fmt-formatter --test external_corpus -- --ignored
```

`SQL_DIALECT_FMT_EXTERNAL_CORPUS` はファイルとディレクトリのパスリストを受け入れます。ディレクトリはケースを無視して `*.sql` ファイルを再帰的に検索します。相対パスはCargoのテスト作業ディレクトリから解決され、必要に応じてワークスペースのルートからも解決されるため、CIは `crates/...` パスを直接渡すことができます。`SQL_DIALECT_FMT_EXTERNAL_CORPUS_LIMIT` は迅速なスモークラン用にファイルの数を制限し、非UTF-8ファイルはスキップされます。

ラッパースクリプトはローカルパス、コミットされたサンプルコーパス、およびダウンロードされたアーカイブをサポートします：

```sh
scripts/run-external-corpus.sh --sample
scripts/run-external-corpus.sh --path /path/to/sqls --limit 500
scripts/run-external-corpus.sh --url https://example.com/sql-corpus.tar.gz --limit 500
```

## 適合性レポートジェネレーター

`scripts/conformance-report.py` はローカルディレクトリ/ファイル/アーカイブまたはダウンロードされたアーカイブから `.sql` ファイルとSQLのフェンデッドコードブロックを抽出し、一時的なコーパスを作成し、同じ外部コーパスハーネスを実行し、Markdownパーサー/フォーマッターレポートを出力します：

```sh
scripts/conformance-report.py --path crates/sql-dialect-fmt-formatter/tests/corpus_sample \
  --out target/conformance-report.md
scripts/conformance-report.py --url https://example.com/docs-or-examples.tar.gz --limit 500
```

これは公式仕様に基づくカバレッジの1.0レーンです：手書きのCSTパーサーを置き換えるものではありませんが、すべてのドキュメント/例のスイープに対して再現可能なパーサーギャップレポートを提供し、CIと同じロスレス性/冪等性の不変条件を再利用します。

## 継続的運用

`.github/workflows/corpus.yml` はすべてのプルリクエスト、`main`、および週次で実行されます。デフォルトでは、コミットされたサンプルコーパスをチェックします。週次の実行でより広範なプライベートまたは生成されたコーパスをカバーするには、リポジトリ変数を設定します：

| 変数 | 意味 |
| --- | --- |
| `SQL_DIALECT_FMT_EXTERNAL_CORPUS_URL` | `.sql` ファイルを含むオプションの `.tar.gz`、`.tgz`、`.tar`、または `.zip` アーカイブのURL。 |
| `SQL_DIALECT_FMT_EXTERNAL_CORPUS_LIMIT` | 非常に大きなコーパスに対するスモークランのオプションの上限。 |

同じ値は、ワークフローディスパッチ入力 `corpus_url` と `corpus_limit` を通じて一回限りの実行のために提供できます。

リポジトリは現在、ピン留めされた公開のdbtコーパスシードに対して外部ワークフローを実行するように設定されています：

```text
https://github.com/dbt-labs/jaffle-shop/archive/08ef1d578de5b55f226aae34f30d7077df8e9f35.tar.gz
```

そのシードは意図的にこのリポジトリにベンダーされていません；より広範な公開またはプライベートコーパスに回転する際には、リポジトリ変数を更新してください。

外部コーパスは事前にフォーマットされている必要はありません；不変条件のみがチェックされます。実行は、失敗する前にすべての問題のあるファイルを収集するため、一度のパスで完全なリストを提供します。

## 失敗のトリアージ

まず、ファイルを孤立して再現します：

```sh
cargo run -p sql-dialect-fmt --bin sql-dialect-fmt -- --no-config path/to/offender.sql > /tmp/once.sql
cargo run -p sql-dialect-fmt --bin sql-dialect-fmt -- --no-config /tmp/once.sql > /tmp/twice.sql
diff /tmp/once.sql /tmp/twice.sql
```

`not idempotent` は通常、2回目のパスで異なる構造を生成する降下ルールを指します。`significant tokens changed across formatting` はロスレス性のバグです。`formatted output does not reparse cleanly` は通常、フォーマッタの出力に対するバグまたはフォーマッタが現在出力するためのパーサーギャップを意味します。