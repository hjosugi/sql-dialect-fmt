<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# sql-dialect-fmt for Neovim

[sql-dialect-fmt](https://github.com/hjosugi/sql-dialect-fmt) の小さな Neovim プラグインです。
`snowflake-sql` ファイルタイプを登録し、`sql-dialect-fmt-lsp` 言語サーバー（フォーマット、範囲
フォーマット、入力時フォーマット、診断、ホバー、セマンティックトークン、補完）をセットアップします。

デフォルトでは `*.snowsql` と `*.sfsql` のみを関連付けます。素の `*.sql` バッファには**触れない**
ため、sqls / sqlls などの他の SQL 言語サーバーはそのまま動作します。`*.sql` の関連付けは
`claim_sql = true` による明示的なオプトインです。

## 必要条件

- Neovim 0.10 以降（0.11 以降ではネイティブの `vim.lsp.config` / `vim.lsp.enable` を使用）。
- `PATH` 上の `sql-dialect-fmt-lsp` バイナリ：

  ```sh
  # crates.io から
  cargo install sql-dialect-fmt-lsp --locked

  # このリポジトリから直接
  cargo install --git https://github.com/hjosugi/sql-dialect-fmt sql-dialect-fmt-lsp

  # ローカルチェックアウトから
  cargo install --path crates/sql-dialect-fmt-lsp
  ```

  Homebrew タップ（`brew tap hjosugi/sql-dialect-fmt https://github.com/hjosugi/sql-dialect-fmt && brew install sql-dialect-fmt`）
  と GitHub Release の tarball には現在 `sql-dialect-fmt` **CLI** バイナリが含まれます —
  後述の [CLI でのフォーマット](#lsp-の代わりに-cli-でフォーマットする) に便利です。LSP サーバーは
  `cargo install sql-dialect-fmt-lsp` でインストールします。

## インストール

プラグインはメインリポジトリの `editors/nvim` サブディレクトリにあります。

[lazy.nvim](https://github.com/folke/lazy.nvim) の場合：

```lua
{
  "hjosugi/sql-dialect-fmt",
  name = "sql-dialect-fmt.nvim",
  config = function(plugin)
    -- Neovim プラグインはモノレポのサブディレクトリにあります。
    vim.opt.rtp:append(plugin.dir .. "/editors/nvim")
    require("sql-dialect-fmt").setup({
      -- claim_sql = true,          -- 素の *.sql も snowflake-sql に関連付ける
      -- filetypes = { "snowflake-sql", "sql" },  -- または: sql バッファにもアタッチ
      -- settings = { lineWidth = 100, indentWidth = 4, dialect = "snowflake" },
    })
  end,
}
```

[packer.nvim](https://github.com/wbthomason/packer.nvim) の場合：

```lua
use({
  "hjosugi/sql-dialect-fmt",
  rtp = "editors/nvim",
  config = function()
    require("sql-dialect-fmt").setup()
  end,
})
```

## 設定

`setup()` は以下を受け付けます：

| キー | デフォルト | 説明 |
| --- | --- | --- |
| `filetype` | `true` | `*.snowsql` / `*.sfsql` に `snowflake-sql` ファイルタイプを登録する。 |
| `claim_sql` | `false` | 素の `*.sql` バッファも `snowflake-sql` に関連付ける。他の SQL LSP が `*.sql` を使い続けられるようオフ。 |
| `server` | `true` | 言語サーバーを定義して有効化する。 |
| `cmd` | `{ "sql-dialect-fmt-lsp" }` | サーバーの起動コマンド（stdio）。 |
| `filetypes` | `{ "snowflake-sql" }` | サーバーがアタッチするファイルタイプ。ファイルタイプを変えずにアタッチするには `"sql"` を追加。 |
| `root_markers` | `{ "sql-dialect-fmt.toml", ".git" }` | プロジェクトルートのマーカー。サーバーの設定探索と一致。 |
| `settings` | `{}` | `sqlDialectFmt` セクションとして送られる設定：`lineWidth`、`indentWidth`、`dialect`（`snowflake`/`databricks`）、`uppercaseKeywords`、`keywordCase`、`lineEnding`、`lint.*`。 |

サーバーはオプションを**デフォルト → 最寄りの `sql-dialect-fmt.toml` → エディタ設定**の順で
解決するため、プロジェクトの設定ファイルによって CI とエディタが一貫します。

フォーマットは通常どおり `vim.lsp.buf.format()` で実行できます。保存時に実行する例：

```lua
vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = { "*.snowsql", "*.sfsql" },
  callback = function()
    vim.lsp.buf.format()
  end,
})
```

### サーバーを自分で定義する（nvim-lspconfig スタイル）

`setup()` によるサーバー管理を使わない場合（`server = false`）、定義は単なるデータです —
Neovim 0.11 以降または nvim-lspconfig の `vim.lsp.config` で：

```lua
vim.lsp.config("sql_dialect_fmt_lsp", {
  cmd = { "sql-dialect-fmt-lsp" },
  filetypes = { "snowflake-sql" },
  root_markers = { "sql-dialect-fmt.toml", ".git" },
  settings = { sqlDialectFmt = { lineWidth = 100 } },
})
vim.lsp.enable("sql_dialect_fmt_lsp")
```

## LSP の代わりに CLI でフォーマットする

フォーマットだけが必要な場合は、`sql-dialect-fmt` CLI（stdin → stdout）をフォーマッタ
ランナーに組み込めます。

[conform.nvim](https://github.com/stevearc/conform.nvim)：

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

[none-ls / null-ls](https://github.com/nvimtools/none-ls.nvim)：

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

`--stdin-filepath` により、CLI はフォーマット対象ファイルの最寄りの
`sql-dialect-fmt.toml` を発見できます。

## Tree-sitter ハイライト（オプション）

プラグインは `queries/snowflake/`（highlights、locals、injections、folds —
[`tree-sitter-snowflake/queries/`](../../tree-sitter-snowflake/queries) のコピー）を同梱して
いるため、`snowflake` パーサーをインストールすれば `snowflake-sql` ファイルタイプで
tree-sitter ハイライトが有効になります。バンドルされた文法を nvim-treesitter（master
ブランチ）に登録するには：

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

その後 `:TSInstall snowflake` を実行します。パーサーがなくてもプラグインは動作します —
ハイライトは LSP のセマンティックトークンが提供します。

## サポートとソース

- [問題を報告する](https://github.com/hjosugi/sql-dialect-fmt/issues)
- [ソースコード](https://github.com/hjosugi/sql-dialect-fmt)
- ライセンス: [0BSD](../../LICENSE)
