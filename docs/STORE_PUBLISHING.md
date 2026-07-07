# Store Publishing Runbook

Last checked against official store docs: 2026-06-28.

This is the no-decision path for the remaining one-time store setup. After this setup, future
`v*.*.*` tag pushes can publish the VS Code Marketplace package and Chrome Web Store package
automatically through `.github/workflows/release.yml`. `.github/workflows/extensions.yml`
remains available for manual package and publish runs.

## Link Deck

Open these from top to bottom. The workflow/helper commands later in this document fill in the
repo-side settings, so the links below are only for the store and cloud-console tasks that must be
done in a browser.

| Task | Link |
| --- | --- |
| GitHub repo | [hjosugi/sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt) |
| GitHub CLI | [Install gh](https://cli.github.com/) |
| Release workflow | [Release workflow](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/release.yml) |
| Manual extension package workflow | [Extension Packages workflow](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/extensions.yml) |
| GitHub Actions variables | [Repository variables](https://github.com/hjosugi/sql-dialect-fmt/settings/variables/actions) |
| GitHub Actions secrets | [Repository secrets](https://github.com/hjosugi/sql-dialect-fmt/settings/secrets/actions) |
| Privacy policy URL to paste into stores | [docs/PRIVACY.md](https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md) |
| VS Code Marketplace publisher console | [Visual Studio Marketplace manage](https://marketplace.visualstudio.com/manage) |
| VS Code publisher after creation | [sql-dialect-fmt publisher page](https://marketplace.visualstudio.com/manage/publishers/sql-dialect-fmt) |
| Azure DevOps PATs | [Personal Access Tokens](https://dev.azure.com/_usersSettings/tokens) |
| VS Code publishing docs | [Publishing Extensions](https://code.visualstudio.com/api/working-with-extensions/publishing-extension) |
| Microsoft Entra app registrations | [App registrations](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade) |
| Google account 2-step verification | [2-Step Verification](https://myaccount.google.com/signinoptions/two-step-verification) |
| Chrome Web Store dashboard | [Developer Dashboard](https://chrome.google.com/webstore/developer/dashboard) |
| Chrome developer account setup | [Set up your developer account](https://developer.chrome.com/docs/webstore/set-up-account) |
| Chrome listing fields | [Complete your listing information](https://developer.chrome.com/docs/webstore/cws-dashboard-listing) |
| Chrome privacy fields | [Fill out the privacy fields](https://developer.chrome.com/docs/webstore/cws-dashboard-privacy) |
| Chrome distribution fields | [Set your distribution options](https://developer.chrome.com/docs/webstore/cws-dashboard-distribution) |
| Google Cloud project creation | [Create project](https://console.cloud.google.com/projectcreate) |
| Chrome Web Store API enablement | [Chrome Web Store API](https://console.cloud.google.com/apis/library/chromewebstore.googleapis.com) |
| OAuth consent screen | [OAuth consent](https://console.cloud.google.com/apis/credentials/consent) |
| OAuth credentials | [Credentials](https://console.cloud.google.com/apis/credentials) |
| OAuth Playground | [OAuth 2.0 Playground](https://developers.google.com/oauthplayground) |
| Chrome Web Store API docs | [Use the Chrome Web Store API](https://developer.chrome.com/docs/webstore/using-api) |

Optional: open the main browser pages at once from the workspace root:

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

## Repo Values

Use these exact values unless the store says the ID is unavailable:

| Field | Value |
| --- | --- |
| GitHub repo | `hjosugi/sql-dialect-fmt` |
| VS Code publisher ID | `sql-dialect-fmt` |
| VS Code extension name | `snowflake-sql-sql-dialect-fmt` |
| VS Code display name | `Snowflake SQL (sql-dialect-fmt)` |
| Chrome extension name | `sql-dialect-fmt for SQL editors` |
| Privacy policy URL | `https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md` |

If `sql-dialect-fmt` is not available as the VS Code publisher ID, stop and update
`editors/package.json` before publishing. Marketplace extension identity includes the publisher ID,
so it is better to change it before the first publish than after.

## Fast Path

Use this path for the first public store release. It uses a VS Code PAT because it is the shortest
first-release path. Microsoft says global Azure DevOps PATs retire on 2026-12-01, so schedule the
Entra ID path below before that date.

```sh
gh --version
gh auth status || gh auth login
./scripts/package-extensions.sh
```

Keep this shell open. You will export credentials into it, then the helper will write GitHub
repository variables and secrets.

## VS Code Marketplace

1. Open [Visual Studio Marketplace manage](https://marketplace.visualstudio.com/manage).
2. Sign in with the Microsoft account that should own the extension.
3. Create a publisher:
   - ID: `sql-dialect-fmt`
   - Name: `sql-dialect-fmt`
4. Create an [Azure DevOps Personal Access Token](https://dev.azure.com/_usersSettings/tokens):
   - Organizations: `All accessible organizations`
   - Scope: `Marketplace (Manage)`
   - Expiration: choose the shortest value that is practical for this release account
5. Export it locally:

```sh
export VSCE_PAT='paste-token-here'
```

Do not commit or paste the token into docs/issues/chat. The helper stores it as a GitHub secret.

## Chrome Web Store Item

1. Make sure the Google account has [2-step verification](https://myaccount.google.com/signinoptions/two-step-verification) enabled.
2. Open the [Chrome Web Store Developer Dashboard](https://chrome.google.com/webstore/developer/dashboard).
3. Create the developer account if it does not exist yet.
4. Create a new item by uploading:

```text
target/dist/sql-dialect-fmt-v1.0.0-chrome.zip
```

5. Copy the extension item ID from the item URL or dashboard.
6. Open `Publisher > Settings` and copy the publisher ID.
7. Export both:

```sh
export CHROME_EXTENSION_ID='paste-extension-item-id-here'
export CHROME_PUBLISHER_ID='paste-publisher-id-here'
```

## Chrome Listing Copy

Use this text in the Chrome dashboard.

Short description:

```text
Format Snowflake and Databricks SQL directly in browser editors with sql-dialect-fmt.
```

Detailed description:

```text
sql-dialect-fmt formats SQL in the active Snowflake Snowsight worksheet or Databricks SQL editor.

Run it from the floating editor button, the extension action button, or Alt+Shift+F. If a SQL range is selected, only that range is formatted. Otherwise the extension formats the whole active SQL editor. The options page lets users choose Snowflake or Databricks mode and set line width, indent width, and keyword casing.

Formatting runs locally in the browser with the bundled WebAssembly build of sql-dialect-fmt. The extension does not send worksheet contents to an external service.
```

Single purpose:

```text
Format SQL in the active Snowflake Snowsight worksheet or Databricks SQL editor using the bundled sql-dialect-fmt WebAssembly formatter.
```

Category: `Developer Tools`

Language: choose the language you want the listing to be reviewed in. Use `English` if unsure.

Privacy policy URL:

```text
https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md
```

Permission justifications:

```text
activeTab: Used so the extension action and keyboard shortcut can run only after the user invokes the formatter in the active tab.

Host permissions for Snowflake/Snowsight and Databricks domains: Required to detect the active SQL editor and replace the selected SQL, or the whole editor contents, with formatted SQL.
```

Privacy answers, assuming the extension code is unchanged:

```text
Data collection: No user data is collected.
Remote code: No remote code is used. The formatter WebAssembly file is bundled with the extension.
Data sharing or sale: No data is shared or sold.
```

If the dashboard asks for screenshots or icons and the repo does not yet contain final store art,
create simple product screenshots from Snowsight with the formatter button visible. Do not use
customer SQL; use a tiny demo query such as `select 1 as id`.

## Chrome Web Store API Credentials

1. Open [Google Cloud project creation](https://console.cloud.google.com/projectcreate).
2. Create or select a Google Cloud project for release automation.
3. Open [Chrome Web Store API](https://console.cloud.google.com/apis/library/chromewebstore.googleapis.com) and enable it for the release automation project.
4. Open [OAuth consent screen](https://console.cloud.google.com/apis/credentials/consent).
5. Select `External`, then create the consent screen.
6. Fill only the required app fields:
   - App name: `sql-dialect-fmt release automation`
   - User support email: your release email
   - Developer contact information: your release email
7. Skip scopes.
8. Add your own Google account as a test user.
9. Open [Credentials](https://console.cloud.google.com/apis/credentials).
10. Create `OAuth client ID`.
11. Application type: `Web application`.
12. Name: `sql-dialect-fmt Chrome Web Store publisher`.
13. Authorized redirect URI:

```text
https://developers.google.com/oauthplayground
```

14. Copy the client ID and client secret.
15. Open [OAuth 2.0 Playground](https://developers.google.com/oauthplayground).
16. Open the settings panel and enable `Use your own OAuth credentials`.
17. Paste the client ID and client secret.
18. In `Input your own scopes`, enter:

```text
https://www.googleapis.com/auth/chromewebstore
```

19. Click `Authorize APIs`, sign in as the Google account that owns the Chrome Web Store item, and
    exchange the authorization code for tokens.
20. Copy the refresh token.
21. Export the values:

```sh
export CHROME_CLIENT_ID='paste-client-id-here'
export CHROME_CLIENT_SECRET='paste-client-secret-here'
export CHROME_REFRESH_TOKEN='paste-refresh-token-here'
```

## Write GitHub Secrets And Variables

Manual pages, if you ever want to inspect what the helper wrote:

- [GitHub Actions variables](https://github.com/hjosugi/sql-dialect-fmt/settings/variables/actions)
- [GitHub Actions secrets](https://github.com/hjosugi/sql-dialect-fmt/settings/secrets/actions)

Run the helper once:

```sh
scripts/configure-extension-publishing.sh --repo hjosugi/sql-dialect-fmt --target all --vscode-auth pat
```

Expected output includes these writes:

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

Use `--dry-run` first if you want to verify the variable names without writing anything:

```sh
scripts/configure-extension-publishing.sh --repo hjosugi/sql-dialect-fmt --target all --vscode-auth pat --dry-run
```

## First Publish

Because `v1.0.0` may already exist and the Chrome dashboard item was created by uploading the same
zip, make the first workflow publish submit the existing Chrome draft instead of uploading the same
version again:

```sh
gh variable set CHROME_SKIP_UPLOAD --repo hjosugi/sql-dialect-fmt --body true
```

Then dispatch the first store publish:

```sh
gh workflow run "Extension Packages" \
  --repo hjosugi/sql-dialect-fmt \
  -f version=1.0.0 \
  -f publish=true \
  -f publish_target=all
```

The same run is visible at the [Extension Packages workflow page](https://github.com/hjosugi/sql-dialect-fmt/actions/workflows/extensions.yml).

Watch it:

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

Remove the one-time skip immediately after that run finishes:

```sh
gh variable delete CHROME_SKIP_UPLOAD --repo hjosugi/sql-dialect-fmt
```

For future releases, the normal release tag push is enough once `EXTENSIONS_AUTO_PUBLISH=true` is
set:

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

## Long-Term VS Code Auth: Entra ID

Use this before 2026-12-01 or immediately if the release account cannot use PATs.

1. Create an [Entra application registration](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade) or managed identity for GitHub Actions publishing.
2. Add a federated credential for this GitHub repo and the release workflow.
3. Add that identity to the [Visual Studio Marketplace publisher](https://marketplace.visualstudio.com/manage/publishers/sql-dialect-fmt) with Contributor access.
4. Export:

```sh
export AZURE_CLIENT_ID='paste-client-id-here'
export AZURE_TENANT_ID='paste-tenant-id-here'
export AZURE_SUBSCRIPTION_ID='paste-subscription-id-here' # optional
```

5. Store the repo variables:

```sh
scripts/configure-extension-publishing.sh --repo hjosugi/sql-dialect-fmt --target vscode --vscode-auth azure
```

After this, VS Code publishing uses `vsce publish --azure-credential` in GitHub Actions and no
`VSCE_PAT` is needed.

## If Something Fails

- VS Code says publisher mismatch: make the Marketplace publisher ID match
  `editors/package.json` `publisher`, or change `editors/package.json` before first publish.
- Chrome upload says version already exists: bump `extensions/chrome/manifest.json` version through
  the normal workspace release process and rebuild the zip.
- Chrome API says unauthorized: regenerate the refresh token while signed in as the Google account
  that owns the Chrome Web Store item.
- Chrome API says visibility changed: publish once manually from the dashboard with the current
  visibility settings, then use API publishing again.
- GitHub workflow packages but does not publish: check `EXTENSIONS_AUTO_PUBLISH=true`, or the
  per-store variables `VSCODE_MARKETPLACE_AUTO_PUBLISH=true` and
  `CHROME_WEBSTORE_AUTO_PUBLISH=true`.

## Official References

- [VS Code publishing](https://code.visualstudio.com/api/working-with-extensions/publishing-extension)
- [VS Code Marketplace publisher management](https://marketplace.visualstudio.com/manage)
- [Chrome Web Store API setup](https://developer.chrome.com/docs/webstore/using-api)
- [Chrome Web Store dashboard](https://chrome.google.com/webstore/developer/dashboard)
