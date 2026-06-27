-- Representative SELECT: CTE, joins, window functions, and semi-structured access.
WITH recent_orders AS (
    SELECT o.order_id, o.customer_id, o.payload:channel::STRING AS channel, o.net_amount
    FROM SALES.ORDERS AS o
    WHERE o.order_date >= DATEADD('day', -30, CURRENT_DATE())
)
SELECT
    r.customer_id,
    COUNT(*) AS order_count,
    SUM(r.net_amount) AS total_amount,
    ROW_NUMBER() OVER (PARTITION BY r.channel ORDER BY SUM(r.net_amount) DESC) AS channel_rank
FROM recent_orders AS r
INNER JOIN SALES.CUSTOMERS AS c ON r.customer_id = c.customer_id
WHERE c.status = 'ACTIVE'
GROUP BY r.customer_id, r.channel
HAVING SUM(r.net_amount) > 0
ORDER BY total_amount DESC NULLS LAST;
