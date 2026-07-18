# CLI Reference

```sh
sql-dialect-fmt [OPTIONS] [PATH ...]
```

With no paths, or with `-`, the CLI reads stdin and writes formatted SQL to stdout. Directory
arguments are searched recursively for `*.sql` files while common generated directories such as
`.git`, `node_modules`, and `target` are skipped.

## Modes

| Mode | Command | Exit behavior |
| --- | --- | --- |
| Print | `sql-dialect-fmt query.sql` | formatted SQL on stdout |
| Write | `sql-dialect-fmt --write query.sql` | rewrites files in place |
| Check | `sql-dialect-fmt --check sql/` | exits `1` if any file would change |
| Diff | `sql-dialect-fmt --check --diff sql/` | prints unified diffs for unformatted files |

## Options

| Option | Meaning |
| --- | --- |
| `--dialect snowflake\|databricks` | Select SQL dialect. |
| `--line-width N` | Target print width. |
| `--indent-width N` | Spaces per indent level. |
| `--no-uppercase` | Preserve keyword casing instead of upper-casing keywords. |
| `--stdin-filepath PATH` | Use a path for stdin config discovery and diagnostics. |
| `--no-config` | Ignore `sql-dialect-fmt.toml`. |
| `--write` | Rewrite path arguments in place. |
| `--check` | Verify formatting without writing. |
| `--diff` | Show diffs with `--check`. |

## Homebrew

```sh
brew tap hjosugi/sql-dialect-fmt https://github.com/hjosugi/sql-dialect-fmt
brew install sql-dialect-fmt
```

## CI

```yaml
- uses: hjosugi/sql-dialect-fmt@v1
  with:
    args: "sql/**/*.sql"
```

```sh
docker run --rm -v "$PWD:/work" -w /work ghcr.io/hjosugi/sql-dialect-fmt:1.16.1 --check .
```

## pre-commit

```yaml
repos:
  - repo: https://github.com/hjosugi/sql-dialect-fmt
    rev: v1.16.1
    hooks:
      - id: sql-dialect-fmt
```

Use `sql-dialect-fmt-check` for verification-only hooks.
