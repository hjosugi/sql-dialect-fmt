-- sql-dialect-fmt Neovim integration.
--
-- Registers the `snowflake-sql` filetype and starts the `sql-dialect-fmt-lsp`
-- language server (formatting, range formatting, diagnostics, hover, semantic
-- tokens, completion). Nothing runs until `setup()` is called, and plain
-- `*.sql` buffers are left alone unless you opt in with `claim_sql = true`,
-- so other SQL language servers keep working out of the box.
--
-- The server itself layers configuration as: formatter defaults -> nearest
-- `sql-dialect-fmt.toml` -> the `settings` table passed here.

local M = {}

M.defaults = {
  -- Register the `snowflake-sql` filetype for `*.snowsql` / `*.sfsql`.
  filetype = true,
  -- Also map plain `*.sql` buffers to `snowflake-sql`. Off by default so this
  -- plugin does not steal `*.sql` from other SQL tooling.
  claim_sql = false,
  -- Define and enable the language server. Set to false if you manage the
  -- server yourself (e.g. through nvim-lspconfig or conform.nvim).
  server = true,
  -- Command used to launch the language server (stdio).
  cmd = { "sql-dialect-fmt-lsp" },
  -- Filetypes the server attaches to. Add "sql" here to attach to plain SQL
  -- buffers without changing their filetype.
  filetypes = { "snowflake-sql" },
  -- Project root markers, matching the server's own config discovery.
  root_markers = { "sql-dialect-fmt.toml", ".git" },
  -- Settings sent to the server, e.g. { lineWidth = 100, indentWidth = 4,
  -- dialect = "snowflake", uppercaseKeywords = true }. Wrapped under the
  -- `sqlDialectFmt` section automatically unless already wrapped.
  settings = {},
}

--- Wrap flat settings under the `sqlDialectFmt` section the server reads.
local function server_settings(settings)
  if settings.sqlDialectFmt or settings["sql-dialect-fmt"] or settings.sql_dialect_fmt then
    return settings
  end
  return { sqlDialectFmt = settings }
end

local function register_filetype(opts)
  local extension = {
    snowsql = "snowflake-sql",
    sfsql = "snowflake-sql",
  }
  if opts.claim_sql then
    extension.sql = "snowflake-sql"
  end
  vim.filetype.add({ extension = extension })
  -- Reuse the bundled tree-sitter grammar (named "snowflake") for this
  -- filetype once the parser is installed; harmless when it is not.
  pcall(vim.treesitter.language.register, "snowflake", "snowflake-sql")
end

--- Locate the project root for a buffer (Neovim 0.10+, with a fallback).
local function find_root(bufnr, markers)
  if vim.fs.root then
    return vim.fs.root(bufnr, markers)
  end
  local name = vim.api.nvim_buf_get_name(bufnr)
  local dir = name ~= "" and vim.fs.dirname(name) or (vim.uv or vim.loop).cwd()
  local found = vim.fs.find(markers, { path = dir, upward = true })[1]
  return found and vim.fs.dirname(found) or nil
end

local function enable_server(opts)
  local settings = server_settings(opts.settings)
  if vim.lsp.config and vim.lsp.enable then
    -- Neovim 0.11+: native lspconfig-style definition.
    vim.lsp.config("sql_dialect_fmt_lsp", {
      cmd = opts.cmd,
      filetypes = opts.filetypes,
      root_markers = opts.root_markers,
      settings = settings,
    })
    vim.lsp.enable("sql_dialect_fmt_lsp")
    return
  end
  -- Neovim 0.10: start the server per buffer.
  vim.api.nvim_create_autocmd("FileType", {
    pattern = opts.filetypes,
    group = vim.api.nvim_create_augroup("sql_dialect_fmt_lsp", { clear = true }),
    callback = function(event)
      vim.lsp.start({
        name = "sql_dialect_fmt_lsp",
        cmd = opts.cmd,
        root_dir = find_root(event.buf, opts.root_markers),
        settings = settings,
      }, { bufnr = event.buf })
    end,
  })
end

function M.setup(opts)
  opts = vim.tbl_deep_extend("force", vim.deepcopy(M.defaults), opts or {})
  if opts.filetype then
    register_filetype(opts)
  end
  if opts.server then
    enable_server(opts)
  end
  return opts
end

return M
