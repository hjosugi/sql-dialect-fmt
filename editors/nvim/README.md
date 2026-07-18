<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt for Neovim

A small Neovim plugin for [sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt):
it registers the `snowflake-sql` filetype and wires up the `sql-dialect-fmt-lsp` language
server (formatting, range formatting, on-type formatting, diagnostics, hover, semantic
tokens, completion).

By default it associates `*.snowsql` and `*.sfsql` only. Plain `*.sql` buffers are **not**
touched, so sqls/sqlls or any other SQL language server keeps working; claiming `*.sql` is
an explicit opt-in (`claim_sql = true`).

## Requirements

- Neovim 0.10+ (0.11+ uses the native `vim.lsp.config` / `vim.lsp.enable` path).
- The `sql-dialect-fmt-lsp` binary on your `PATH`:

  ```sh
  # From crates.io
  cargo install sql-dialect-fmt-lsp --locked

  # Directly from this repository
  cargo install --git https://github.com/hjosugi/sql-dialect-fmt sql-dialect-fmt-lsp

  # From a local checkout
  cargo install --path crates/sql-dialect-fmt-lsp
  ```

  The Homebrew tap (`brew tap hjosugi/sql-dialect-fmt https://github.com/hjosugi/sql-dialect-fmt && brew install sql-dialect-fmt`)
  and the GitHub Release tarballs currently ship the `sql-dialect-fmt` **CLI** binary — useful
  for the [CLI formatting](#formatting-with-the-cli-instead-of-the-lsp) route below — while the
  LSP server is installed with `cargo install sql-dialect-fmt-lsp`.

## Install

The plugin lives in the `editors/nvim` subdirectory of the main repository.

With [lazy.nvim](https://github.com/folke/lazy.nvim):

```lua
{
  "hjosugi/sql-dialect-fmt",
  name = "sql-dialect-fmt.nvim",
  config = function(plugin)
    -- The Neovim plugin lives in a subdirectory of the monorepo.
    vim.opt.rtp:append(plugin.dir .. "/editors/nvim")
    require("sql-dialect-fmt").setup({
      -- claim_sql = true,          -- also map plain *.sql to snowflake-sql
      -- filetypes = { "snowflake-sql", "sql" },  -- or: attach to sql buffers
      -- settings = { lineWidth = 100, indentWidth = 4, dialect = "snowflake" },
    })
  end,
}
```

With [packer.nvim](https://github.com/wbthomason/packer.nvim):

```lua
use({
  "hjosugi/sql-dialect-fmt",
  rtp = "editors/nvim",
  config = function()
    require("sql-dialect-fmt").setup()
  end,
})
```

## Configuration

`setup()` accepts:

| Key | Default | Description |
| --- | --- | --- |
| `filetype` | `true` | Register the `snowflake-sql` filetype for `*.snowsql` / `*.sfsql`. |
| `claim_sql` | `false` | Also map plain `*.sql` buffers to `snowflake-sql`. Off so other SQL LSPs keep `*.sql`. |
| `server` | `true` | Define and enable the language server. |
| `cmd` | `{ "sql-dialect-fmt-lsp" }` | Command used to launch the server (stdio). |
| `filetypes` | `{ "snowflake-sql" }` | Filetypes the server attaches to. Add `"sql"` to attach without changing the filetype. |
| `root_markers` | `{ "sql-dialect-fmt.toml", ".git" }` | Project root markers, matching the server's config discovery. |
| `settings` | `{}` | Settings sent under the `sqlDialectFmt` section: `lineWidth`, `indentWidth`, `dialect` (`snowflake`/`databricks`), `uppercaseKeywords`, `keywordCase`, `lineEnding`, `lint.*`. |

The server resolves options as **defaults → nearest `sql-dialect-fmt.toml` → editor
settings**, so a project config file keeps CI and the editor consistent.

Format with the usual `vim.lsp.buf.format()`, e.g. on save:

```lua
vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = { "*.snowsql", "*.sfsql" },
  callback = function()
    vim.lsp.buf.format()
  end,
})
```

### Defining the server yourself (nvim-lspconfig style)

If you prefer to skip `setup()`'s server management (`server = false`), the definition is
plain data — with Neovim 0.11+ or nvim-lspconfig's `vim.lsp.config`:

```lua
vim.lsp.config("sql_dialect_fmt_lsp", {
  cmd = { "sql-dialect-fmt-lsp" },
  filetypes = { "snowflake-sql" },
  root_markers = { "sql-dialect-fmt.toml", ".git" },
  settings = { sqlDialectFmt = { lineWidth = 100 } },
})
vim.lsp.enable("sql_dialect_fmt_lsp")
```

## Formatting with the CLI instead of the LSP

If you only want formatting, the `sql-dialect-fmt` CLI (stdin → stdout) plugs into
formatter runners.

[conform.nvim](https://github.com/stevearc/conform.nvim):

```lua
require("conform").setup({
  formatters_by_ft = {
    ["snowflake-sql"] = { "sql_dialect_fmt" },
    -- sql = { "sql_dialect_fmt" },
  },
  formatters = {
    sql_dialect_fmt = {
      command = "sql-dialect-fmt",
      args = { "--stdin-filepath", "$FILENAME" },
      stdin = true,
    },
  },
})
```

[none-ls / null-ls](https://github.com/nvimtools/none-ls.nvim):

```lua
local null_ls = require("null-ls")
null_ls.register({
  name = "sql-dialect-fmt",
  method = null_ls.methods.FORMATTING,
  filetypes = { "snowflake-sql" },
  generator = require("null-ls.helpers").formatter_factory({
    command = "sql-dialect-fmt",
    args = { "--stdin-filepath", "$FILENAME" },
    to_stdin = true,
  }),
})
```

`--stdin-filepath` lets the CLI discover the nearest `sql-dialect-fmt.toml` for the file
being formatted.

## Tree-sitter highlighting (optional)

The plugin ships `queries/snowflake/` (highlights, locals, injections, folds — copies of
[`tree-sitter-snowflake/queries/`](../../tree-sitter-snowflake/queries)), so once the
`snowflake` parser is installed the `snowflake-sql` filetype gets tree-sitter highlighting.
Register the bundled grammar with nvim-treesitter (master branch):

```lua
require("nvim-treesitter.parsers").get_parser_configs().snowflake = {
  install_info = {
    url = "https://github.com/hjosugi/sql-dialect-fmt",
    location = "tree-sitter-snowflake",
    files = { "src/parser.c" },
    branch = "main",
  },
  filetype = "snowflake-sql",
}
```

Then run `:TSInstall snowflake`. Without the parser, the plugin still works — the LSP
provides semantic tokens for highlighting.

## Support and source

- [Report an issue](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [Source code](https://github.com/hjosugi/sql-dialect-fmt)
- License: [0BSD](../../LICENSE)
