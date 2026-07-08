# Formatting Style

The formatter is intentionally small on options. The expected shape is stable output that can run in
CI without team-local style debates.

## Statements

- SQL keywords are upper-cased by default.
- Statements end with semicolons.
- Major clauses such as `SELECT`, `FROM`, `WHERE`, `GROUP BY`, `QUALIFY`, and `ORDER BY` start on
  their own lines when the query is multi-line.
- Comma-separated lists use one item per line when they do not fit within the configured width.

```sql
SELECT
    customer_id,
    COUNT(*) AS orders,
    SUM(total_amount) AS revenue
FROM analytics.orders
WHERE order_status = 'paid'
GROUP BY customer_id;
```

## Losslessness

Formatting must preserve the meaningful token stream. Inputs with lexer or parser errors are
returned unchanged rather than reformatted through an uncertain tree.

Comments are preserved as leading, trailing, or dangling comments according to their CST position.
Routine bodies and embedded language blocks are kept inside their original SQL delimiters.

## Width And Indentation

`line_width` controls when groups break. `indent_width` controls the spaces added for nested
structures. Both options are available through the CLI, config files, the browser extension, and the
WASM playground.
