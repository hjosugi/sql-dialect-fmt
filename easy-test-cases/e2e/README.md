# E2E scenario: Global multilingual commerce pipeline

This scenario is intentionally over-engineered for parser, formatter, and integration tests.
It models customer and order CDC, multilingual data, SQL/JavaScript/Python procedures,
semi-structured payloads, analytical views, streams, and a suspended task graph.

Run `sql/00_bootstrap.sql` through `sql/07_assertions.sql` in lexical order. The final call to
`OPS.SP_VALIDATE_E2E()` raises a custom Snowflake Scripting exception if any deterministic
assertion fails.

The task graph is defined only to validate DDL and formatter behavior. Newly created tasks are
suspended; this suite never resumes them. `sql/20_optional_copy.sql` is independent and needs
files uploaded to `@RAW.INGEST_STAGE`.

The one-file version is under `final/` as an intentionally ugly input and its canonical answer.
