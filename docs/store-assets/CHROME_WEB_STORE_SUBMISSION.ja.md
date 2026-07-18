<!-- i18n: language-switcher -->
[English](CHROME_WEB_STORE_SUBMISSION.md) | [日本語](CHROME_WEB_STORE_SUBMISSION.ja.md)

# Chrome Web Store 提出シート

公式の Chrome Web Store リストおよび画像ドキュメントに対して最後に確認した日: 2026-07-11。

このシートは、初回リストおよび後の更新のためのコピー/ペーストの真実のソースです。

## ストアリスト

**製品名**

```text
sql-dialect-fmt for SQL editors
```

**概要**

```text
sql-dialect-fmt を使用して、ブラウザエディタで直接 Snowflake および Databricks SQL をフォーマットします。
```

**詳細な説明**

```text
sql-dialect-fmt は、アクティブな Snowflake Snowsight ワークシートまたは Databricks SQL エディタ内の SQL をフォーマットします。

浮動エディタボタン、拡張機能アクションボタン、または Alt+Shift+F から実行します。SQL 範囲が選択されている場合、その範囲のみがフォーマットされます。そうでない場合、拡張機能はアクティブな SQL エディタ全体をフォーマットします。

オプションページで Snowflake または Databricks モードを選択し、行幅、インデント幅、およびキーワードの大文字小文字を設定します。これらのフォーマッタの設定は、サインインした Chrome プロファイル間で同期できます。

フォーマットは、バンドルされた WebAssembly ビルドの sql-dialect-fmt を使用して、ブラウザ内でローカルに実行されます。SQL テキストは保存されず、外部サービスに送信されることはありません。
```

**カテゴリ:** `開発者ツール`

**言語:** `英語`

## グラフィック資産

| フィールド | アップロード |
| --- | --- |
| 128×128 ストアイコン | `chrome/store-icon-128.png` |
| 1280×800 スクリーンショット 1 | `chrome/screenshot-formatter-1280x800.png` |
| 1280×800 スクリーンショット 2 | `chrome/screenshot-options-1280x800.png` |
| 440×280 小型プロモタイル | `chrome/small-promo-440x280.png` |
| 1400×560 マーキータイル | `chrome/marquee-promo-1400x560.png` |

**ローカライズされたプロモ動画:** `chrome/demo-video-1280x720.mp4` をリリースアカウントの YouTube チャンネルに `非公開` としてアップロードし、実際の YouTube 視聴 URL を貼り付けます。この URL はリポジトリの資格情報から作成できず、偽の URL や非 YouTube URL に置き換えてはいけません。

オプションの YouTube サムネイルとして `chrome/youtube-thumbnail-1280x720.png` を使用します。

**YouTube タイトル**

```text
sql-dialect-fmt for SQL editors — Chrome 拡張機能デモ
```

**YouTube 説明**

```text
sql-dialect-fmt を使用して、アクティブなブラウザエディタ内で直接 Snowflake Snowsight および Databricks SQL をフォーマットします。

エディタボタン、Chrome 拡張機能アクション、または Alt+Shift+F からフォーマッタを実行します。フォーマットはバンドルされた WebAssembly でローカルに実行されます。SQL テキストは保存されず、外部サービスに送信されることはありません。

ソースおよびドキュメント: https://github.com/hjosugi/sql-dialect-fmt
プライバシーポリシー: https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md
```

- 可視性: `非公開`
- 対象: `いいえ、子供向けではありません`
- 有料プロモーション: `いいえ`
- 変更された/合成コンテンツの開示: 現在の YouTube フォームに従って回答します。動画には製品 UI フィクスチャと生成されたブランドアートワークが含まれていますが、現実の人々やイベントは含まれていません。

## プライバシー慣行

**単一目的**

```text
バンドルされた sql-dialect-fmt WebAssembly フォーマッタを使用して、アクティブな Snowflake Snowsight ワークシートまたは Databricks SQL エディタ内の SQL をフォーマットします。
```

**権限の正当化**

```text
activeTab: ユーザーがアクティブタブでフォーマッタを呼び出した後にのみ、拡張機能のアクションとキーボードショートカットが実行できるように使用されます。

storage: SQL ダイアレクト、行幅、インデント幅、およびキーワードの大文字小文字などのフォーマッタの設定のみを chrome.storage.sync を使用して保存します。SQL テキストは保存されません。

Snowflake/Snowsight および Databricks ドメインのホスト権限: アクティブな SQL エディタを検出し、選択された SQL またはエディタ全体の内容をフォーマットされた SQL に置き換えるために必要です。
```

**リモートコード:** `いいえ`

```text
リモートコードは使用されていません。Rust フォーマッタは WebAssembly にコンパイルされ、拡張機能パッケージにバンドルされています。拡張機能によって実行されるすべての JavaScript、CSS、および WebAssembly はパッケージ内に含まれています。
```

**データ収集:** データカテゴリは選択しない。

```text
拡張機能はユーザーデータを収集、送信、販売、または共有しません。SQL テキストは要求に応じてローカルで処理され、保存されません。フォーマッタの設定のみが chrome.storage.sync に保存されます。
```

**プライバシーポリシー URL**

```text
https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md
```

データの使用が Chrome Web Store ポリシーに準拠していること、ユーザーデータが販売されず、開示された単一目的以外で使用されないことを確認するために必要な認証をチェックしてください。

## 配布

- 可視性: `公開`
- 地域: `すべての地域`（リリースオーナーが配布を制限する法的理由がない限り）
- 価格: `無料`

## レビュアーの指示

```text
1. 拡張機能をインストールし、サポートされている Snowflake Snowsight または Databricks SQL エディタを開きます。
2. デモ SQL を入力します: select customer_id,sum(amount) from orders group by customer_id
3. SQL エディタにフォーカスを合わせます。
4. 浮動 sql-dialect-fmt ボタン、拡張機能アクション、または Alt+Shift+F を実行します。
5. SQL がその場でフォーマットされていることを確認します。
6. 拡張機能のオプションを開き、Snowflake と Databricks の間で切り替えるか、行幅、インデント幅、およびキーワードの大文字小文字を変更します。

アカウントの資格情報は拡張機能にバンドルされていません。レビュアーはサポートされているエディタページにアクセスする必要があります。フォーマットはローカルで実行され、開発者が運営するサーバーを呼び出すことはありません。
```

## 最終ダッシュボードチェックリスト

- [ ] `v1.13.0` Chrome zip をアップロードします。
- [ ] すべてのリストコピーを正確に貼り付けます。
- [ ] アイコン、2つのスクリーンショット、小型プロモ、オプションのマーキー ファイルをアップロードします。
- [ ] チェックインされた MP4 を YouTube にアップロードし、実際の URL を貼り付けます。
- [ ] プライバシーの回答と権限の正当化を完了します。
- [ ] 公開 / すべての地域 / 無料に設定します。
- [ ] 下書きを保存し、レビュアーの指示を一度実行します。
- [ ] レビューのために提出します。