//! Zed extension for `sql-dialect-fmt`.
//!
//! Launches the `sql-dialect-fmt-lsp` binary from `PATH` (or from the
//! `lsp.sql-dialect-fmt-lsp.binary` settings override) for `Snowflake SQL`
//! buffers, and forwards Zed's per-server `initialization_options` / `settings`
//! blocks to it. The server itself resolves options as defaults → nearest
//! `sql-dialect-fmt.toml` → editor settings.

use zed_extension_api::settings::LspSettings;
use zed_extension_api::{self as zed, serde_json, LanguageServerId, Result};

struct SqlDialectFmtExtension;

impl zed::Extension for SqlDialectFmtExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree).ok();
        let binary = settings.as_ref().and_then(|settings| settings.binary.as_ref());

        let command = binary
            .and_then(|binary| binary.path.clone())
            .or_else(|| worktree.which("sql-dialect-fmt-lsp"))
            .ok_or_else(|| {
                "sql-dialect-fmt-lsp not found on PATH. Install it with \
                 `cargo install sql-dialect-fmt-lsp --locked`."
                    .to_string()
            })?;
        let args = binary
            .and_then(|binary| binary.arguments.clone())
            .unwrap_or_default();

        Ok(zed::Command {
            command,
            args,
            env: Vec::new(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|settings| settings.initialization_options))
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|settings| settings.settings))
    }
}

zed::register_extension!(SqlDialectFmtExtension);
