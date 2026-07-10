# sql-dialect-fmt HANDOFF（アーカイブ）— 2026-07-10

> 旧 `HANDOFF.md` は 2026-06-27 時点の作業再開メモで、v0.1.0/v1.0.0
> 前提の記述や解消済み技術負債を多く含んでいた。現在の進行管理は
> GitHub Issues とリリース履歴へ移したため、このファイルは短い再開メモと
> アーカイブ案内として維持する。

関連: [README.md](README.md) / [ROADMAP.md](ROADMAP.md) /
[CHANGELOG.md](CHANGELOG.md) / [RELEASING.md](RELEASING.md) /
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) /
[docs/research/](docs/research/)

## 現在の状態

- 最新の GitHub release は `v1.8.0`。
- CLI / formatter / parser / LSP / tree-sitter / VS Code extension /
  Chrome extension / release asset workflow は実用配布レーンに乗っている。
- crates.io publish は release workflow 上で opt-in のため、GitHub release と
  crates.io の公開版は一致しないことがある。公開判断は
  [RELEASING.md](RELEASING.md) と `scripts/publish-crates.sh` を確認する。
- Store 配布（VS Code Marketplace / Chrome Web Store）は workflow と helper
  を持つが、初回 listing・審査・publisher 権限は各 store 側の管理画面作業を含む。
- 細かい未完了項目はこのファイルでは追跡しない。GitHub Issues の
  `enhancement` / `formatter` / `parser` / `lsp` / `docs` / `release`
  などのラベルを真実の源にする。

## 再開時の確認

通常のコード変更では次を最初に確認する。

```sh
python3 scripts/check-version-consistency.py 1.8.0
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

配布物や release に触れる場合は追加で確認する。

```sh
./scripts/package-extensions.sh
cargo bench -p sql-dialect-fmt-formatter --bench format -- --test
scripts/run-external-corpus.sh --sample
scripts/conformance-report.py \
  --path crates/sql-dialect-fmt-formatter/tests/corpus_sample \
  --out target/conformance-report.md
```

## リリース手順

1. 変更を実装し、関連テストを通す。
2. `python3 scripts/update-version.py <version> --changelog --date <YYYY-MM-DD>`
   で workspace / lockfile / editor package / docs / changelog link を更新する。
3. `CHANGELOG.md` の新 section に実装内容を追記する。
4. `python3 scripts/check-version-consistency.py <version>` と release gate を再実行する。
5. `release: v<version>` で commit し、`v<version>` tag を作る。
6. `main` と tag を push し、GitHub Actions の Release / CI / Docs / Corpus を確認する。
7. Release assets と必要な publish job の状態を確認し、該当 issue に release と commit を
   コメントして close する。

crates.io publish は依存順の公開と index 反映待ちが必要なため、GitHub release
とは別の明示判断として扱う。

## 現在の参照先

- 構成・設計: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- 機能カバレッジ: [ROADMAP.md](ROADMAP.md)
- 変更履歴: [CHANGELOG.md](CHANGELOG.md)
- release checklist: [RELEASING.md](RELEASING.md)
- store publish: [docs/STORE_PUBLISHING.md](docs/STORE_PUBLISHING.md)
- Snowflake 仕様調査: [spec/](spec/) と [docs/research/](docs/research/)

## 代表的な残作業

この一覧は作業キューではなく、再開時の方向感を示すだけの索引。
優先順位と完了判定は GitHub Issues 側を確認する。

- Formatter polish: コメント配置、長い論理式、空行保存、埋め込み SQL body。
- Parser / formatter coverage: balanced paren 構文、Dialect 差分、未構造化 DDL。
- LSP / editor: rich hover、設定 reload、VS Code integration、store listing。
- Refactor: formatter module split、shared helpers、feature flags、generated tables。
- Release operations: GitHub release asset 検証、store publish、crates.io publish 判断。

旧 2026-06-27 の詳細メモが必要な場合は、このファイルの git history を参照する。
