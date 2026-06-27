-- Representative Databricks SELECT: CTE, joins, window functions, QUALIFY.
WITH recent AS (
    SELECT e.user_id, e.amount, e.channel
    FROM main.default.events AS e
    WHERE e.ts > '2024-01-01'
)
SELECT r.user_id, count(*) AS n, sum(r.amount) AS total
FROM recent AS r
INNER JOIN main.default.users AS u ON r.user_id = u.id
WHERE u.active IS TRUE
GROUP BY r.user_id, r.channel
HAVING sum(r.amount) > 100
QUALIFY row_number() OVER (PARTITION BY r.user_id ORDER BY total DESC) = 1
ORDER BY total DESC NULLS LAST
LIMIT 50;
