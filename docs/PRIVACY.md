<!-- i18n: language-switcher -->
[English](PRIVACY.md) | [日本語](PRIVACY.ja.md)

# Privacy

This policy covers the `Snowflake SQL (sql-dialect-fmt)` VS Code extension.

Formatting runs locally in the VS Code extension host through the bundled WebAssembly formatter.
The extension does not send SQL, settings, file names, credentials, account identifiers, or usage
analytics to the project maintainers or to a formatting service.

When the optional `sqlDialectFmt.lsp.enabled` setting is enabled, the extension starts the locally
installed `sql-dialect-fmt-lsp` executable over stdio. The language server also processes documents
locally and makes no network requests on behalf of the extension.

The extension stores no SQL text. VS Code itself persists user/workspace settings according to the
editor's normal settings behavior.

Report privacy or security concerns through the repository's security policy.
