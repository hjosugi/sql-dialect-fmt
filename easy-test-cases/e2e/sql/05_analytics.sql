-- 05_analytics.sql
-- Complex analytical queries intended for formatter and scenario validation.
WITH ranked_customers AS (
    SELECT
        *,
        DENSE_RANK() OVER (
            PARTITION BY region
            ORDER BY lifetime_net_amount DESC, customer_id
        ) AS region_value_rank,
        PERCENT_RANK() OVER (
            ORDER BY lifetime_net_amount
        ) AS global_value_percentile
    FROM MART.V_CUSTOMER_360
)
SELECT
    region,
    customer_id,
    display_name,
    locale,
    order_count,
    lifetime_net_amount,
    region_value_rank,
    global_value_percentile,
    value_segment,
    ARRAY_SIZE(product_ids) AS product_count
FROM ranked_customers
QUALIFY region_value_rank <= 3
ORDER BY region, region_value_rank, customer_id;

SELECT
    *
FROM (
    SELECT
        region,
        locale,
        net_amount
    FROM MART.V_ORDER_FACTS
)
PIVOT (
    SUM(net_amount)
    FOR locale IN (
        'ja-JP' AS JA,
        'fr-FR' AS FR,
        'ko-KR' AS KO,
        'hi-IN' AS HI,
        'th-TH' AS TH
    )
)
ORDER BY region;

WITH metrics AS (
    SELECT
        customer_id,
        order_count,
        delivered_order_count,
        lifetime_net_amount,
        total_item_quantity
    FROM MART.V_CUSTOMER_360
)
SELECT
    customer_id,
    metric_name,
    metric_value
FROM metrics
UNPIVOT INCLUDE NULLS (
    metric_value
    FOR metric_name IN (
        order_count,
        delivered_order_count,
        lifetime_net_amount,
        total_item_quantity
    )
)
ORDER BY customer_id, metric_name;

SELECT
    order_id,
    match_number,
    first_status,
    last_status,
    first_status_at,
    last_status_at
FROM CORE.ORDER_STATUS_HISTORY
MATCH_RECOGNIZE (
    PARTITION BY order_id
    ORDER BY status_at
    MEASURES
        MATCH_NUMBER() AS match_number,
        FIRST(any_status.status) AS first_status,
        LAST(any_status.status) AS last_status,
        FIRST(any_status.status_at) AS first_status_at,
        LAST(any_status.status_at) AS last_status_at
    ONE ROW PER MATCH
    AFTER MATCH SKIP PAST LAST ROW
    PATTERN (any_status+)
    DEFINE
        any_status AS any_status.status IS NOT NULL
)
ORDER BY order_id, match_number;

SELECT
    text_id,
    language_tag,
    text_value,
    LENGTH(text_value) AS character_count,
    OCTET_LENGTH(text_value) AS utf8_byte_count,
    REGEXP_COUNT(text_value, '[[:alpha:]]') AS alphabetic_count,
    metadata:script::STRING AS declared_script,
    metadata:direction::STRING AS direction
FROM CORE.MULTILINGUAL_TEXTS
WHERE
    text_value ILIKE ANY ('%データ%', '%데이터%', '%بيانات%', '%נתונים%', '%डेटा%')
    OR language_tag IN ('ja-JP', 'ko-KR', 'ar-SA', 'he-IL', 'hi-IN')
ORDER BY language_tag, text_id;
