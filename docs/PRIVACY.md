# sql-dialect-fmt Extensions Privacy Policy

Last updated: 2026-06-28

This policy applies to:

- `sql-dialect-fmt for Snowsight`, the Chrome extension in `extensions/chrome`
- `Snowflake SQL (sql-dialect-fmt)`, the VS Code extension in `editors`

## Data Collection

These extensions do not collect, sell, transmit, or share user data.

The Chrome extension reads SQL text from the active Snowflake Snowsight worksheet only when the
user runs the formatter from the extension button, the browser action, or the `Alt+Shift+F`
shortcut. The SQL is formatted locally by the WebAssembly module bundled with the extension and is
written back to the active editor. The extension does not send worksheet contents to any external
server.

The VS Code extension contributes Snowflake SQL language metadata and TextMate grammar files. It
does not include telemetry, analytics, network upload, or remote formatting.

## Permissions

The Chrome extension requests access to Snowflake and Snowsight hostnames so it can find the active
worksheet editor and replace the selected SQL, or the whole editor contents, with formatted SQL.
It requests `activeTab` so the browser action can run only after a user gesture in the active tab.

## Storage

These extensions do not store SQL text, credentials, account identifiers, browsing history, or
analytics events.

## Contact

For questions or security reports, open an issue at:

https://github.com/hjosugi/sql-dialect-fmt/issues
