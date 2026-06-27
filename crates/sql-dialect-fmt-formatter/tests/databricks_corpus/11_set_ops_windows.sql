-- Set operations and window frames.
SELECT
    id,
    sum(
        amount
    ) OVER (PARTITION BY region ORDER BY ts ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS running
FROM main.default.sales
UNION ALL
SELECT id, 0
FROM main.default.refunds;
