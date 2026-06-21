# Snowflake spec change log

Human-readable notes on Snowflake SQL surface changes that matter to snow-fmt. The machine log
lives in the SQLite DB (`python3 spec/snowflake_spec.py changes`); this file is the curated summary.

## 2026-06-21 — initial seed
- Seeded `spec/seed/features.json` from snow-fmt's ROADMAP + prior-art research (curated, not a
  live crawl — refresh from docs.snowflake.com on the next pass).
- Parser coverage at seed time: **Phase 1–2** — SELECT + all single-select clauses, JOINs,
  subqueries/derived tables, set operations, CTEs, compound predicates, and window functions.
- Notable gaps flagged `todo` to drive upcoming work: **CASE expressions**, `VALUES`, the `|>`
  pipe operator, semi-structured `:` path access, `PIVOT`/`UNPIVOT`, `MATCH_RECOGNIZE`, all DML,
  all DDL, Snowflake Scripting, embedded JS/Python bodies, and Cortex/AISQL functions.
- TODO next refresh: replace the curated seed with a docs-sourced inventory and set accurate
  GA/Preview/Deprecated status per feature.
