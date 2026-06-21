-- Case 019: Materialized view plus search optimization and clustering-heavy query.
CREATE OR REPLACE MATERIALIZED VIEW MART.MV_ORDER_DAILY_TOP_SKU
CLUSTER BY (order_date, tenant_id, region)
AS
WITH sku_daily AS (
    SELECT
        order_date,
        tenant_id,
        region,
        sku,
        SUM(quantity) AS quantity,
        SUM(net_amount) AS net_amount,
        COUNT(DISTINCT order_id) AS order_count
    FROM CORE.FACT_ORDER_ITEM
    WHERE order_date >= DATE '2024-01-01'
    GROUP BY order_date, tenant_id, region, sku
), ranked AS (
    SELECT
        *,
        RANK() OVER (
            PARTITION BY order_date, tenant_id, region
            ORDER BY net_amount DESC, quantity DESC, sku
        ) AS amount_rank
    FROM sku_daily
)
SELECT
    order_date,
    tenant_id,
    region,
    sku,
    quantity,
    net_amount,
    order_count,
    amount_rank
FROM ranked
WHERE amount_rank <= 100;

ALTER TABLE CORE.FACT_ORDER_ITEM ADD SEARCH OPTIMIZATION ON EQUALITY(order_id, customer_id, sku), SUBSTRING(item_name);
