<!-- i18n: language-switcher -->
[English](STORE_PUBLISHING.md) | [日本語](STORE_PUBLISHING.ja.md)

# ストア公開ランブック

公式ストアドキュメントに基づいて最終確認済み: 2026-07-11

これは、残りの一度きりのストア設定を行うための決定不要な手順書です。この設定が完了すれば、将来の`v*.*.*`タグプッシュにより、VS Code MarketplaceパッケージとChrome Web Storeパッケージが`.github/workflows/release.yml`を通じて自動的に公開されます。`.github/workflows/extensions.yml`は、手動でのパッケージ作成および公開実行のために引き続き利用可能です。

## リンクデッキ

以下のリンクを上から順に開いてください。このドキュメントの後半に記載されているワークフロー/ヘルパーコマンドはリポジトリ側の設定を補完します。そのため、以下のリンクはブラウザで行う必要があるストアやクラウドコンソールのタスク用です。

| タスク | リンク |
| --- | --- |
| GitHubリポジトリ | [hjosugi/sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt) |
| GitHub CLI | [ghをインストール](https://cli.github.com/) |
| リリースワークフロー | [リリースワークフロー](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/release.yml) |
| 手動拡張パッケージワークフロー | [拡張パッケージワークフロー](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/extensions.yml) |
| GitHub Actions変数 | [リポジトリ変数](https://github.com/hjosugi/sql-dialect-fmt/settings/variables/actions) |
| GitHub Actionsシークレット | [リポジトリシークレット](https://github.com/hjosugi/sql-dialect-fmt/settings/secrets/actions) |
| ストアに貼り付けるプライバシーポリシーURL | [docs/PRIVACY.md](https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md) |
| VS Code Marketplaceパブリッシャーコンソール | [Visual Studio Marketplace管理](https://marketplace.visualstudio.com/manage) |
| VS Codeパブリッシャー作成後 | [sql-dialect-fmtパブリッシャーページ](https://marketplace.visualstudio.com/manage/publishers/sql-dialect-fmt) |
| Azure DevOps PATs | [個人用アクセス トークン](https://dev.azure.com/_usersSettings/tokens) |
| VS Code公開ドキュメント | [拡張機能の公開](https://code.visualstudio.com/api/working-with-extensions/publishing-extension) |
| Microsoft Entraアプリ登録 | [アプリ登録](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade) |
| Googleアカウント2段階認証 | [2段階認証](https://myaccount.google.com/signinoptions/two-step-verification) |
| Chrome Web Storeダッシュボード | [開発者ダッシュボード](https://chrome.google.com/webstore/developer/dashboard) |
| Chrome開発者アカウント設定 | [開発者アカウントの設定](https://developer.chrome.com/docs/webstore/set-up-account) |
| Chromeリスティングフィールド | [リスティング情報の入力](https://developer.chrome.com/docs/webstore/cws-dashboard-listing) |
| Chrome画像要件 | [画像の提供](https://developer.chrome.com/docs/webstore/images) |
| Chromeプライバシーフィールド | [プライバシーフィールドの入力](https://developer.chrome.com/docs/webstore/cws-dashboard-privacy) |
| Chrome配布フィールド | [配布オプションの設定](https://developer.chrome.com/docs/webstore/cws-dashboard-distribution) |
| Google Cloudプロジェクト作成 | [プロジェクトの作成](https://console.cloud.google.com/projectcreate) |
| Chrome Web Store API有効化 | [Chrome Web Store API](https://console.cloud.google.com/apis/library/chromewebstore.googleapis.com) |
| OAuth同意画面 | [OAuth同意画面](https://console.cloud.google.com/apis/credentials/consent) |
| OAuth認証情報 | [認証情報](https://console.cloud.google.com/apis/credentials) |
| OAuth Playground | [OAuth 2.0 Playground](https://developers.google.com/oauthplayground) |
| Chrome Web Store APIドキュメント | [Chrome Web Store APIの使用](https://developer.chrome.com/docs/webstore/using-api) |

オプション: ワークスペースのルートから主要なブラウザページを一度に開く:

```sh
python3 - <<'PY'
import webbrowser
for url in [
    "https://marketplace.visualstudio.com/manage",
    "https://dev.azure.com/_usersSettings/tokens",
    "https://chrome.google.com/webstore/developer/dashboard",
    "https://console.cloud.google.com/projectcreate",
    "https://console.cloud.google.com/apis/library/chromewebstore.googleapis.com",
    "https://console.cloud.google.com/apis/credentials/consent",
    "https://console.cloud.google.com/apis/credentials",
    "https://developers.google.com/oauthplayground",
    "https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/extensions.yml",
]:
    webbrowser.open(url)
PY
```

## リポジトリ値

ストアでIDが利用できないと表示されない限り、以下の値を正確に使用してください:

| フィールド | 値 |
| --- | --- |
| GitHubリポジトリ | `hjosugi/sql-dialect-fmt` |
| VS CodeパブリッシャーID | `sql-dialect-fmt` |
| VS Code拡張名 | `snowflake-sql-sql-dialect-fmt` |
| VS Code表示名 | `Snowflake SQL (sql-dialect-fmt)` |
| Chrome拡張名 | `sql-dialect-fmt for SQL editors` |
| プライバシーポリシーURL | `https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md` |

もし`sql-dialect-fmt`がVS CodeパブリッシャーIDとして利用できない場合、公開前に`editors/package.json`を更新してください。Marketplaceの拡張機能の識別にはパブリッシャーIDが含まれるため、最初の公開前に変更する方が後から変更するよりも良いです。

## 簡易手順

初回のストア公開には以下の手順を使用してください。これはVS CodeのPATを使用する最短の初回公開手順です。Microsoftによると、グローバルAzure DevOps PATは2026-12-01に廃止されるため、それまでに以下のEntra ID手順をスケジュールしてください。

```sh
gh --version
gh auth status || gh auth login
./scripts/package-extensions.sh
```

このシェルを開いたままにしてください。この後、資格情報をエクスポートし、ヘルパーがGitHubリポジトリの変数とシークレットを書き込みます。

## VS Code Marketplace

1. [Visual Studio Marketplace管理](https://marketplace.visualstudio.com/manage)を開きます。
2. 拡張機能を所有するMicrosoftアカウントでサインインします。
3. パブリッシャーを作成します:
   - ID: `sql-dialect-fmt`
   - 名前: `sql-dialect-fmt`
4. [Azure DevOps個人用アクセス トークン](https://dev.azure.com/_usersSettings/tokens)を作成します:
   - 組織: `すべてのアクセス可能な組織`
   - スコープ: `Marketplace (管理)`
   - 有効期限: このリリースアカウントに実用的な最短の値を選択
5. ローカルにエクスポートします:

```sh
export VSCE_PAT='ここにトークンを貼り付けてください'
```

トークンをドキュメント、問題、チャットにコミットまたは貼り付けないでください。ヘルパーがこれをGitHubシークレットとして保存します。

## Chrome Web Storeアイテム

1. Googleアカウントに[2段階認証](https://myaccount.google.com/signinoptions/two-step-verification)が有効になっていることを確認します。
2. [Chrome Web Store開発者ダッシュボード](https://chrome.google.com/webstore/developer/dashboard)を開きます。
3. 開発者アカウントがまだ存在しない場合は作成します。
4. 以下をアップロードして新しいアイテムを作成します:

```text
target/dist/sql-dialect-fmt-v1.13.0-chrome.zip
```

5. アイテムURLまたはダッシュボードから拡張アイテムIDをコピーします。
6. `Publisher > Settings`を開き、パブリッシャーIDをコピーします。
7. 両方をエクスポートします:

```sh
export CHROME_EXTENSION_ID='ここに拡張アイテムIDを貼り付けてください'
export CHROME_PUBLISHER_ID='ここにパブリッシャーIDを貼り付けてください'
```

## Chromeリスティング用コピー

Chromeダッシュボードで以下のテキストを使用してください。

短い説明:

```text
sql-dialect-fmtを使用して、ブラウザエディタ内で直接SnowflakeおよびDatabricks SQLをフォーマットします。
```

詳細な説明:

```text
sql-dialect-fmtは、アクティブなSnowflake SnowsightワークシートまたはDatabricks SQLエディタ内のSQLをフォーマットします。

フローティングエディタボタン、拡張アクションボタン、またはAlt+Shift+Fから実行できます。SQLの範囲が選択されている場合、その範囲のみがフォーマットされます。それ以外の場合、拡張機能はアクティブなSQLエディタ全体をフォーマットします。オプションページでは、SnowflakeまたはDatabricksモードを選択し、行幅、インデント幅、キーワードの大文字小文字を設定できます。

フォーマットは、sql-dialect-fmtのWebAssemblyビルドを使用してブラウザ内でローカルに実行されます。この拡張機能は、ワークシートの内容を外部サービスに送信しません。
```

単一目的:

```text
バンドルされたsql-dialect-fmt WebAssemblyフォーマッタを使用して、アクティブなSnowflake SnowsightワークシートまたはDatabricks SQLエディタ内のSQLをフォーマットします。
```

カテゴリ: `Developer Tools`

言語: リスティングをレビューしてもらいたい言語を選択してください。不明な場合は`English`を使用してください。

プライバシーポリシーURL:

```text
https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md
```

権限の正当化:

```text
activeTab: 拡張アクションやキーボードショートカットが、ユーザーがアクティブなタブでフォーマッタを起動した後にのみ実行されるようにするために使用されます。

storage: chrome.storage.syncを使用して、SQL方言、行幅、インデント幅、キーワードの大文字小文字などのフォーマッタ設定のみを保存します。SQLテキストは保存されません。

Snowflake/SnowsightおよびDatabricksドメインのホスト権限: アクティブなSQLエディタを検出し、選択されたSQLまたはエディタ全体の内容をフォーマットされたSQLに置き換えるために必要です。
```

プライバシーに関する回答（拡張コードが変更されていない場合）:

```text
データ収集: ユーザーデータは収集されません。
リモートコード: リモートコードは使用されません。フォーマッタのWebAssemblyファイルは拡張機能にバンドルされています。
データ共有または販売: データは共有または販売されません。
```

## Chromeグラフィックアセットとビデオ

以下のアセットを正確にアップロードしてください:

| ダッシュボードフィールド | ファイル |
| --- | --- |
| ストアアイコン, 128×128 | `docs/store-assets/chrome/store-icon-128.png` |
| スクリーンショット1, 1280×800 | `docs/store-assets/chrome/screenshot-formatter-1280x800.png` |
| スクリーンショット2, 1280×800 | `docs/store-assets/chrome/screenshot-options-1280x800.png` |
| 小型プロモタイル, 440×280 | `docs/store-assets/chrome/small-promo-440x280.png` |
| マーキープロモタイル, 1400×560, オプション | `docs/store-assets/chrome/marquee-promo-1400x560.png` |
| YouTubeデモソース, 1280×720 | `docs/store-assets/chrome/demo-video-1280x720.mp4` |

デモMP4をリリースアカウントのYouTubeチャンネルに「限定公開」としてアップロードし、その実際の`https://www.youtube.com/watch?v=...`URLをローカライズされたプロモビデオフィールドに貼り付けてください。このフィールドにはリポジトリやGitHubリリースURLを代わりに使用することはできません。ダッシュボードにプレースホルダーURLを作成しないでください。

完全なコピーペースト用の提出シートとレビュアー向けの指示は[`docs/store-assets/CHROME_WEB_STORE_SUBMISSION.md`](store-assets/CHROME_WEB_STORE_SUBMISSION.md)に記載されています。

## Chrome Web Store API認証情報

1. [Google Cloudプロジェクト作成](https://console.cloud.google.com/projectcreate)を開きます。
2. リリース自動化用のGoogle Cloudプロジェクトを作成または選択します。
3. [Chrome Web Store API](https://console.cloud.google.com/apis/library/chromewebstore.googleapis.com)を開き、リリース自動化プロジェクト用に有効化します。
4. [OAuth同意画面](https://console.cloud.google.com/apis/credentials/consent)を開きます。
5. `外部`を選択し、同意画面を作成します。
6. 必要なアプリフィールドのみを入力します:
   - アプリ名: `sql-dialect-fmt release automation`
   - ユーザーサポートメール: リリース用メールアドレス
   - 開発者連絡先情報: リリース用メールアドレス
7. スコープはスキップします。
8. 自分のGoogleアカウントをテストユーザーとして追加します。
9. [認証情報](https://console.cloud.google.com/apis/credentials)を開きます。
10. `OAuthクライアントID`を作成します。
11. アプリケーションタイプ: `ウェブアプリケーション`
12. 名前: `sql-dialect-fmt Chrome Web Store publisher`
13. 承認済みリダイレクトURI:

```text
https://developers.google.com/oauthplayground
```

14. クライアントIDとクライアントシークレットをコピーします。
15. [OAuth 2.0 Playground](https://developers.google.com/oauthplayground)を開きます。
16. 設定パネルを開き、`Use your own OAuth credentials`を有効にします。
17. クライアントIDとクライアントシークレットを貼り付けます。
18. `Input your own scopes`に以下を入力します:

```text
https://www.googleapis.com/auth/chromewebstore
```

19. `Authorize APIs`をクリックし、Chrome Web Storeアイテムを所有するGoogleアカウントでサインインし、認証コードをトークンに交換します。
20. リフレッシュトークンをコピーします。
21. 値をエクスポートします:

```sh
export CHROME_CLIENT_ID='ここにクライアントIDを貼り付けてください'
export CHROME_CLIENT_SECRET='ここにクライアントシークレットを貼り付けてください'
export CHROME_REFRESH_TOKEN='ここにリフレッシュトークンを貼り付けてください'
```

## GitHubシークレットと変数の書き込み

ヘルパーが書き込んだ内容を確認したい場合の手動ページ:

- [GitHub Actions変数](https://github.com/hjosugi/sql-dialect-fmt/settings/variables/actions)
- [GitHub Actionsシークレット](https://github.com/hjosugi/sql-dialect-fmt/settings/secrets/actions)

ヘルパーを一度実行します:

```sh
scripts/configure-extension-publishing.sh --repo hjosugi/sql-dialect-fmt --target all --vscode-auth pat
```

期待される出力には以下の書き込みが含まれます:

```text
VSCE_AUTH_MODE=pat
VSCE_PAT secret
CHROME_PUBLISHER_ID
CHROME_EXTENSION_ID
CHROME_CLIENT_ID secret
CHROME_CLIENT_SECRET secret
CHROME_REFRESH_TOKEN secret
EXTENSIONS_AUTO_PUBLISH=true
```

変数名を確認したい場合は、最初に`--dry-run`を使用してください:

```sh
scripts/configure-extension-publishing.sh --repo hjosugi/sql-dialect-fmt --target all --vscode-auth pat --dry-run
```

## 初回公開

Chromeダッシュボードアイテムは`v1.13.0`のzipをアップロードすることで作成されるため、最初のワークフロー公開では既存のドラフトを送信し、同じバージョンを再度アップロードしないようにします:

```sh
gh variable set CHROME_SKIP_UPLOAD --repo hjosugi/sql-dialect-fmt --body true
```

その後、最初のストア公開を実行します:

```sh
gh workflow run "Extension Packages" \
  --repo hjosugi/sql-dialect-fmt \
  -f version=1.13.0 \
  -f publish=true \
  -f publish_target=all
```

同じ実行は[拡張パッケージワークフローページ](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/extensions.yml)で確認できます。

監視:

```sh
run_id="$(
  gh run list \
    --repo hjosugi/sql-dialect-fmt \
    --workflow "Extension Packages" \
    --limit 1 \
    --json databaseId \
    --jq '.[0].databaseId'
)"
gh run watch "$run_id" --repo hjosugi/sql-dialect-fmt --exit-status
```

その実行が終了したら、すぐに一度限りのスキップを削除します:

```sh
gh variable delete CHROME_SKIP_UPLOAD --repo hjosugi/sql-dialect-fmt
```

## 更新の公開

将来のリリースでは、`EXTENSIONS_AUTO_PUBLISH=true`が設定されていれば、通常のリリースタグプッシュだけで十分です。VSIXパブリッシャーは既存のパブリッシャー/拡張IDを使用し、Chromeパブリッシャーは新しいパッケージを設定済みの`CHROME_EXTENSION_ID`にアップロードするため、両方の操作で既存のリスティングが更新されます:

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

部分的な失敗後に1つのストアを再試行するには、同じリリースバージョンとそのターゲットのみで手動ワークフローをディスパッチします。ストアバージョンは不変であるため、パッケージが受け入れられなかった場合にのみ同じバージョンを使用してください。それ以外の場合は、新しいワークスペースバージョンを公開してください。

```sh
gh workflow run "Extension Packages" \
  --repo hjosugi/sql-dialect-fmt \
  -f version=X.Y.Z \
  -f publish=true \
  -f publish_target=vscode # またはchrome
```

両方のワークフローは、要求された公開の前に`scripts/check-publishing-credentials.sh`を実行します。そのため、シークレットまたは変数が不足している場合は、シークレット値を非表示にしたまま設定名で失敗します。

## 長期的なVS Code認証: Entra ID

2026-12-01以前、またはリリースアカウントがPATを使用できない場合はすぐにこれを使用してください。

1. [Entraアプリケーション登録](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade)またはGitHub Actions公開用のマネージドIDを作成します。
2. このGitHubリポジトリとリリースワークフロー用にフェデレーション資格情報を追加します。
3. [Visual Studio Marketplaceパブリッシャー](https://marketplace.visualstudio.com/manage/publishers/sql-dialect-fmt)にこのIDをContributorアクセスで追加します。
4. エクスポート:

```sh
export AZURE_CLIENT_ID='ここにクライアントIDを貼り付けてください'
export AZURE_TENANT_ID='ここにテナントIDを貼り付けてください'
export AZURE_SUBSCRIPTION_ID='ここにサブスクリプションIDを貼り付けてください' # オプション
```

5. リポジトリ変数を保存します:

```sh
scripts/configure-extension-publishing.sh --repo hjosugi/sql-dialect-fmt --target vscode --vscode-auth azure
```

これ以降、VS Codeの公開はGitHub Actions内で`vsce publish --azure-credential`を使用し、`VSCE_PAT`は不要になります。

## 問題が発生した場合

- VS Codeがパブリッシャーの不一致を示す場合: MarketplaceのパブリッシャーIDを`editors/package.json`の`publisher`と一致させるか、最初の公開前に`editors/package.json`を変更してください。
- Chromeのアップロードでバージョンが既に存在すると表示される場合: 通常のワークスペースリリースプロセスを通じて`extensions/chrome/manifest.json`のバージョンを更新し、zipを再構築してください。
- Chrome APIが認証されていないと表示する場合: Chrome Web Storeアイテムを所有するGoogleアカウントでサインインし、リフレッシュトークンを再生成してください。
- Chrome APIが可視性が変更されたと表示する場合: 現在の可視性設定でダッシュボードから手動で一度公開し、その後再びAPI公開を使用してください。
- GitHubワークフローがパッケージ化するが公開しない場合: `EXTENSIONS_AUTO_PUBLISH=true`、またはストアごとの変数`VSCODE_MARKETPLACE_AUTO_PUBLISH=true`および`CHROME_WEBSTORE_AUTO_PUBLISH=true`を確認してください。
- 再試行でバージョンが既に存在すると表示される場合: ストアがそのパッケージを受け入れたことを意味します。ワークスペースバージョンを更新し、代わりに更新を公開してください。

## 公式リファレンス

- [VS Code公開](https://code.visualstudio.com/api/working-with-extensions/publishing-extension)
- [VS Code Marketplaceパブリッシャー管理](https://marketplace.visualstudio.com/manage)
- [Chrome Web Store API設定](https://developer.chrome.com/docs/webstore/using-api)
- [Chrome Web Storeダッシュボード](https://chrome.google.com/webstore/developer/dashboard)