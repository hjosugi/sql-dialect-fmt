<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# spec/ — Snowflake仕様トラッカー（ローカル、**ビルドの一部ではありません**）

このディレクトリは、Snowflake SQLの表面sql-dialect-fmtターゲットとその変化を記録します。
これは**Cargoワークスペースの外**に存在するため（`crates/*`メンバーではありません）、`cargo build`には影響しません。ローカルのSQLite DBはgitで無視され、シードJSON、変更履歴、およびスクリプトが保持されます。

## ファイル
- `seed/features.json` — 精選された、差分可能な機能インベントリ（**これを編集してください**；真実の源です）。
- `snowflake_spec.py` — 標準ライブラリのみのCLI：`init` / `import` / `coverage` / `changes` / `snapshot`。
- `CHANGELOG.md` — 重要な定期的Snowflake変更に関する人間のメモ。
- `snowflake_spec.db` — ローカルSQLiteストア（gitで無視され、`init` + `import`で再生成）。

## 定期的なワークフロー（手動 — 変更への対応は**自動化されていません**）
1. <https://docs.snowflake.com/en/sql-reference>から`seed/features.json`を更新します：新しい
   ステートメント/句/関数を追加し、各`status`（GA/Preview/Deprecated）を更新し、`coverage`
   （`parse` / `partial` / `todo`）を設定して、パーサーが処理する内容を反映させます。
2. DBに対して記録 + 差分を取ります：
   ```sh
   python3 spec/snowflake_spec.py import spec/seed/features.json --note "2026-08 refresh"
   ```
   変更された各フィールドは新しいスナップショットの下に記録されます。
3. 何が移動したかを確認します：`python3 spec/snowflake_spec.py changes` — `CHANGELOG.md`に重要なことをメモします。
4. 次の作業を選択します：`python3 spec/snowflake_spec.py coverage`（カテゴリごとの解析済み/合計）。
5. 重要な変更については、手動でパーサーと`ROADMAP.md`を更新します。

## クイックスタート
```sh
python3 spec/snowflake_spec.py init
python3 spec/snowflake_spec.py import spec/seed/features.json --note "初期シード"
python3 spec/snowflake_spec.py coverage
```