# Canonical formatting contract

The `expected.sql` files define the answer for this suite. This is a project convention,
not an official Snowflake style guide.

1. SQL keywords are uppercase.
2. Indentation is four spaces; continuation expressions use one additional level.
3. Every top-level statement ends with `;`.
4. Major clauses begin on their own line.
5. `SELECT`, `INSERT`, `UPDATE`, `MERGE`, function arguments, and object properties use
   one item per line when multiline.
6. Commas are trailing, not leading.
7. `JOIN ... ON`, window specifications, `LATERAL FLATTEN`, `PIVOT`, `UNPIVOT`, and
   `MATCH_RECOGNIZE` are structurally indented.
8. Snowflake Scripting bodies use `AS $$ ... $$`; nested blocks are indented.
9. Embedded JavaScript and Python are formatted in their native style in `expected.sql`.
   `expected_sql_only.sql` is also supplied for those two cases when the formatter is
   intentionally SQL-only and must preserve body bytes.
10. String values, quoted identifiers, comments, Unicode code points, JSON paths, regexes,
    bind markers, and dollar-quoted body contents must remain semantically unchanged.
11. Unquoted identifiers are shown in uppercase in canonical output.
12. Files are UTF-8 with LF line endings and a final newline.
