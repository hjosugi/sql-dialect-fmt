-- Case 001: Deep semi-structured SELECT with nested LATERAL FLATTEN and QUALIFY.
WITH staged AS (
    SELECT
        e.event_id,
        e.event_time,
        e.loaded_at,
        e.payload,
        COALESCE(NULLIF(TRIM(e.payload:tenant::STRING), ''), 'UNKNOWN') AS tenant_id,
        TRY_TO_TIMESTAMP_TZ(e.payload:occurred_at::STRING) AS occurred_at,
        OBJECT_CONSTRUCT_KEEP_NULL(
            'file', e.source_filename,
            'row', e.source_row_number,
            'trace', e.payload:meta:trace_id::STRING
        ) AS source_context
    FROM RAW.EVENT_LANDING AS e TABLESAMPLE BERNOULLI (25) REPEATABLE (20260621)
    WHERE
        e.loaded_at >= DATEADD('hour', -36, CURRENT_TIMESTAMP())
        AND e.payload:event_type::STRING IN ('order.created', 'order.updated', 'order.cancelled')
), orders AS (
    SELECT
        s.event_id,
        s.event_time,
        s.loaded_at,
        s.tenant_id,
        s.source_context,
        o.index AS order_index,
        o.value AS order_doc,
        o.value:id::STRING AS order_id,
        TRY_TO_DECIMAL(o.value:total:amount::STRING, 18, 4) AS order_total,
        COALESCE(o.value:total:currency::STRING, 'JPY') AS currency_code
    FROM staged AS s,
        LATERAL FLATTEN(INPUT => s.payload:orders, OUTER => TRUE) AS o
), items AS (
    SELECT
        o.*,
        i.index AS item_index,
        i.value:sku::STRING AS sku,
        COALESCE(TRY_TO_NUMBER(i.value:quantity::STRING), 0) AS quantity,
        TRY_TO_DECIMAL(i.value:unit_price::STRING, 18, 4) AS unit_price,
        d.value:code::STRING AS discount_code,
        TRY_TO_DECIMAL(d.value:amount::STRING, 18, 4) AS discount_amount,
        CASE
            WHEN i.value:attributes:gift::BOOLEAN THEN 'gift'
            WHEN ARRAY_SIZE(i.value:attributes:bundles) > 0 THEN 'bundle'
            WHEN REGEXP_LIKE(i.value:sku::STRING, '^[A-Z]{2}-[0-9]{4}-[A-Z0-9]{3}$') THEN 'catalog'
            ELSE 'other'
        END AS item_kind
    FROM orders AS o,
        LATERAL FLATTEN(INPUT => o.order_doc:items, OUTER => TRUE) AS i,
        LATERAL FLATTEN(INPUT => i.value:discounts, OUTER => TRUE) AS d
), scored AS (
    SELECT
        tenant_id,
        order_id,
        sku,
        item_kind,
        currency_code,
        quantity,
        unit_price,
        COALESCE(discount_amount, 0) AS discount_amount,
        quantity * unit_price - COALESCE(discount_amount, 0) AS net_amount,
        source_context,
        loaded_at,
        ROW_NUMBER() OVER (
            PARTITION BY tenant_id, order_id, sku, COALESCE(discount_code, '<NONE>')
            ORDER BY loaded_at DESC, event_time DESC, event_id DESC
        ) AS latest_rank,
        COUNT(*) OVER (
            PARTITION BY tenant_id, order_id
        ) AS rows_per_order
    FROM items
    WHERE
        order_id IS NOT NULL
        AND sku IS NOT NULL
)
SELECT
    tenant_id,
    order_id,
    ARRAY_AGG(
        OBJECT_CONSTRUCT_KEEP_NULL(
            'sku', sku,
            'kind', item_kind,
            'quantity', quantity,
            'unit_price', unit_price,
            'discount', discount_amount,
            'net', net_amount
        )
    ) WITHIN GROUP (ORDER BY sku, item_kind) AS normalized_items,
    SUM(net_amount) AS normalized_total,
    ANY_VALUE(currency_code) AS currency_code,
    MAX(loaded_at) AS last_loaded_at,
    MAX_BY(source_context, loaded_at) AS newest_source_context
FROM scored
WHERE latest_rank = 1
GROUP BY tenant_id, order_id
HAVING
    normalized_total IS NOT NULL
    AND normalized_total <> 0
ORDER BY last_loaded_at DESC, tenant_id, order_id;
