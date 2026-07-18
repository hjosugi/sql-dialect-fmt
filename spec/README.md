<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# spec/ — Snowflake spec tracker (local, **not part of the build**)

This directory records the Snowflake SQL surface sql-dialect-fmt targets and how it changes over time.
It lives **outside the Cargo workspace** (it is not a `crates/*` member), so it never affects
`cargo build`. The local SQLite DB is git-ignored; the seed JSON, changelog, and script are kept.

## Files
- `seed/features.json` — the curated, diffable feature inventory (**edit this**; it's the source of truth).
- `snowflake_spec.py` — stdlib-only CLI: `init` / `import` / `coverage` / `changes` / `snapshot`.
- `CHANGELOG.md` — human notes on notable periodic Snowflake changes.
- `snowflake_spec.db` — local SQLite store (git-ignored, regenerate with `init` + `import`).

## Periodic workflow (manual — responding to changes is **not** automated)
1. Refresh `seed/features.json` from <https://docs.snowflake.com/en/sql-reference>: add new
   statements/clauses/functions, update each `status` (GA/Preview/Deprecated), and set `coverage`
   (`parse` / `partial` / `todo`) to reflect what the parser handles.
2. Record + diff against the DB:
   ```sh
   python3 spec/snowflake_spec.py import spec/seed/features.json --note "2026-08 refresh"
   ```
   Every changed field is logged under a new snapshot.
3. Review what moved: `python3 spec/snowflake_spec.py changes` — note anything important in `CHANGELOG.md`.
4. Pick the next work: `python3 spec/snowflake_spec.py coverage` (parsed/total per category).
5. Update the parser + `ROADMAP.md` by hand for the changes that matter.

## Quick start
```sh
python3 spec/snowflake_spec.py init
python3 spec/snowflake_spec.py import spec/seed/features.json --note "initial seed"
python3 spec/snowflake_spec.py coverage
```
