-- 04_run_pipeline.sql
CALL OPS.SP_APPLY_CUSTOMER_EVENTS('BATCH-001', FALSE);
CALL OPS.SP_APPLY_ORDER_EVENTS('BATCH-001', FALSE);
CALL OPS.SP_JS_CAPTURE_SCHEMA_INVENTORY('%');
CALL OPS.SP_PY_PROFILE_MULTILINGUAL(1000);

CREATE OR REPLACE VIEW MART.V_CUSTOMER_360 AS
WITH order_rollup AS (
    SELECT
        customer_id,
        COUNT(*) AS order_count,
        COUNT_IF(order_status = 'DELIVERED') AS delivered_order_count,
        SUM(net_amount) AS lifetime_net_amount,
        MAX(order_time) AS last_order_time,
        ARRAY_AGG(
            OBJECT_CONSTRUCT(
                'order_id', order_id,
                'status', order_status,
                'net_amount', net_amount,
                'currency', currency,
                'order_time', order_time
            )
        ) WITHIN GROUP (ORDER BY order_time DESC) AS orders
    FROM CORE.ORDERS
    GROUP BY customer_id
),
item_rollup AS (
    SELECT
        orders.customer_id,
        COUNT(DISTINCT items.product_id) AS distinct_products,
        SUM(items.quantity) AS total_item_quantity,
        ARRAY_AGG(DISTINCT items.product_id) AS product_ids
    FROM CORE.ORDERS AS orders
    INNER JOIN CORE.ORDER_ITEMS AS items
        ON orders.order_id = items.order_id
    GROUP BY orders.customer_id
)
SELECT
    customers.customer_id,
    customers.display_name,
    customers.email,
    customers.locale,
    customers.region,
    customers.marketing_opt_in,
    customers.attributes,
    COALESCE(order_rollup.order_count, 0) AS order_count,
    COALESCE(order_rollup.delivered_order_count, 0) AS delivered_order_count,
    COALESCE(order_rollup.lifetime_net_amount, 0) AS lifetime_net_amount,
    order_rollup.last_order_time,
    COALESCE(item_rollup.distinct_products, 0) AS distinct_products,
    COALESCE(item_rollup.total_item_quantity, 0) AS total_item_quantity,
    item_rollup.product_ids,
    order_rollup.orders,
    CASE
        WHEN COALESCE(order_rollup.lifetime_net_amount, 0) >= 10000 THEN 'HIGH'
        WHEN COALESCE(order_rollup.lifetime_net_amount, 0) >= 1000 THEN 'MEDIUM'
        ELSE 'LOW'
    END AS value_segment,
    customers.updated_at
FROM CORE.CUSTOMERS AS customers
LEFT JOIN order_rollup
    ON customers.customer_id = order_rollup.customer_id
LEFT JOIN item_rollup
    ON customers.customer_id = item_rollup.customer_id;

CREATE OR REPLACE VIEW MART.V_ORDER_FACTS AS
SELECT
    orders.order_id,
    orders.customer_id,
    customers.display_name,
    customers.locale,
    customers.region,
    orders.order_status,
    orders.order_time,
    orders.order_time::DATE AS order_date,
    orders.currency,
    orders.gross_amount,
    orders.discount_amount,
    orders.net_amount,
    orders.shipping_address:country::STRING AS shipping_country,
    orders.shipping_address:city::STRING AS shipping_city,
    COUNT(items.line_number) AS line_count,
    SUM(items.quantity) AS total_quantity,
    ARRAY_AGG(
        OBJECT_CONSTRUCT(
            'line', items.line_number,
            'product_id', items.product_id,
            'quantity', items.quantity,
            'line_amount', items.line_amount
        )
    ) WITHIN GROUP (ORDER BY items.line_number) AS items
FROM CORE.ORDERS AS orders
INNER JOIN CORE.CUSTOMERS AS customers
    ON orders.customer_id = customers.customer_id
LEFT JOIN CORE.ORDER_ITEMS AS items
    ON orders.order_id = items.order_id
GROUP BY
    orders.order_id,
    orders.customer_id,
    customers.display_name,
    customers.locale,
    customers.region,
    orders.order_status,
    orders.order_time,
    orders.currency,
    orders.gross_amount,
    orders.discount_amount,
    orders.net_amount,
    orders.shipping_address;

-- Second delta batch: update Japanese customer, delete Hebrew customer, add Thai customer.
INSERT INTO RAW.CUSTOMER_EVENTS (
    event_id,
    batch_id,
    event_time,
    event_type,
    source_system,
    payload
)
SELECT
    column1,
    column2,
    column3::TIMESTAMP_TZ,
    column4,
    column5,
    PARSE_JSON(column6)
FROM VALUES
    ('CE101', 'BATCH-002', '2026-06-10 09:00:00 +09:00', 'UPSERT', 'web-ja', '{"customer_id":"C001","display_name":"杉野尾 広貴（更新）","email":"hiroki@example.jp","locale":"ja-JP","region":"JP","marketing_opt_in":false,"attributes":{"interests":["cloud","分散システム","Elixir"],"tier":"platinum"}}'),
    ('CE102', 'BATCH-002', '2026-06-10 10:00:00 +03:00', 'DELETE', 'privacy-he', '{"customer_id":"C006"}'),
    ('CE103', 'BATCH-002', '2026-06-10 11:00:00 +07:00', 'UPSERT', 'mobile-th', '{"customer_id":"C007","display_name":"พิมพ์ชนก ใจดี","email":"pim@example.th","locale":"th-TH","region":"TH","marketing_opt_in":true,"attributes":{"interests":["คลาวด์","ข้อมูล"],"tier":"silver"}}');

INSERT INTO RAW.ORDER_EVENTS (
    event_id,
    batch_id,
    event_time,
    event_type,
    source_system,
    payload
)
SELECT
    column1,
    column2,
    column3::TIMESTAMP_TZ,
    column4,
    column5,
    PARSE_JSON(column6)
FROM VALUES
    ('OE101', 'BATCH-002', '2026-06-10 10:30:00 +02:00', 'UPSERT', 'fulfillment-fr', '{"order_id":"O1002","customer_id":"C002","status":"SHIPPED","order_time":"2026-06-02T10:00:00+02:00","currency":"EUR","discount_amount":0,"shipping_address":{"country":"FR","city":"Lyon"},"items":[{"line":1,"product_id":"P003","quantity":3,"unit_price":18.5}]}'),
    ('OE102', 'BATCH-002', '2026-06-10 12:00:00 +03:00', 'DELETE', 'privacy-ar', '{"order_id":"O1004"}'),
    ('OE103', 'BATCH-002', '2026-06-10 15:00:00 +07:00', 'UPSERT', 'checkout-th', '{"order_id":"O1006","customer_id":"C007","status":"PAID","order_time":"2026-06-10T14:45:00+07:00","currency":"JPY","discount_amount":0,"shipping_address":{"country":"TH","city":"กรุงเทพมหานคร"},"items":[{"line":1,"product_id":"P002","quantity":1,"unit_price":1800}]}');

CALL OPS.SP_APPLY_CUSTOMER_EVENTS('BATCH-002', FALSE);
CALL OPS.SP_APPLY_ORDER_EVENTS('BATCH-002', FALSE);
