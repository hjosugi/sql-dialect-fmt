-- Case 01: relational + semi-structured + analytical syntax.
WITH RECURSIVE
source_rows AS (
    SELECT
        column1::NUMBER AS event_id,
        column2::TIMESTAMP_TZ AS event_time,
        PARSE_JSON(column3) AS payload
    FROM VALUES
        (
            1,
            '2026-06-01 09:00:00 +09:00',
            '{"customer":{"id":"C001","name":"杉野尾 広貴","locale":"ja-JP"},"items":[{"sku":"本-001","qty":2,"price":1200.50},{"sku":"茶-002","qty":1,"price":850}],"flags":{"gift":true,"priority":"高"}}'
        ),
        (
            2,
            '2026-06-01 12:30:00 +02:00',
            '{"customer":{"id":"C002","name":"Élodie d''Arcy","locale":"fr-FR"},"items":[{"sku":"CAFÉ-01","qty":3,"price":4.75}],"flags":{"gift":false,"priority":"normal"}}'
        )
),
flattened_items AS (
    SELECT
        s.event_id,
        s.event_time,
        s.payload:customer.id::STRING AS customer_id,
        s.payload:customer.name::STRING AS customer_name,
        s.payload:customer.locale::STRING AS locale,
        item.index AS item_index,
        item.value:sku::STRING AS sku,
        item.value:qty::NUMBER AS quantity,
        item.value:price::NUMBER(18, 2) AS unit_price,
        quantity * unit_price AS line_amount,
        COALESCE(s.payload:flags.gift::BOOLEAN, FALSE) AS is_gift,
        s.payload:flags.priority::STRING AS priority
    FROM source_rows AS s,
        LATERAL FLATTEN(
            INPUT => s.payload:items,
            OUTER => TRUE,
            RECURSIVE => FALSE,
            MODE => 'ARRAY'
        ) AS item
),
ranked_items AS (
    SELECT
        *,
        ROW_NUMBER() OVER (
            PARTITION BY customer_id
            ORDER BY line_amount DESC, event_time DESC, item_index
        ) AS value_rank,
        SUM(line_amount) OVER (
            PARTITION BY customer_id
            ORDER BY event_time, item_index
            ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
        ) AS running_amount,
        RATIO_TO_REPORT(line_amount) OVER (PARTITION BY customer_id) AS amount_ratio
    FROM flattened_items
    QUALIFY value_rank <= 10
),
org_tree(employee_id, manager_id, depth, path) AS (
    SELECT
        employee_id,
        manager_id,
        0 AS depth,
        ARRAY_CONSTRUCT(employee_id) AS path
    FROM CORE.EMPLOYEES
    WHERE manager_id IS NULL

    UNION ALL

    SELECT
        child.employee_id,
        child.manager_id,
        parent.depth + 1,
        ARRAY_APPEND(parent.path, child.employee_id)
    FROM CORE.EMPLOYEES AS child
    INNER JOIN org_tree AS parent
        ON child.manager_id = parent.employee_id
    WHERE NOT ARRAY_CONTAINS(child.employee_id::VARIANT, parent.path)
)
SELECT
    r.customer_id,
    MAX(r.customer_name) AS customer_name,
    MAX(r.locale) AS locale,
    COUNT_IF(r.quantity > 0) AS valid_line_count,
    SUM(r.quantity) AS total_quantity,
    ROUND(SUM(r.line_amount), 2) AS gross_amount,
    ARRAY_AGG(
        OBJECT_CONSTRUCT_KEEP_NULL(
            'sku', r.sku,
            'quantity', r.quantity,
            'amount', r.line_amount,
            'rank', r.value_rank,
            'ratio', r.amount_ratio
        )
    ) WITHIN GROUP (ORDER BY r.value_rank) AS ranked_lines,
    OBJECT_AGG(r.sku, r.line_amount) AS amount_by_sku,
    LISTAGG(DISTINCT r.priority, ' | ') WITHIN GROUP (ORDER BY r.priority) AS priorities
FROM ranked_items AS r
WHERE
    r.customer_name ILIKE ANY ('%杉%', '%élodie%', '%홍%')
    OR r.locale IN ('ja-JP', 'fr-FR', 'ko-KR', 'ar-SA')
GROUP BY GROUPING SETS (
    (r.customer_id),
    ()
)
HAVING SUM(r.line_amount) >= 0
ORDER BY
    GROUPING(r.customer_id),
    gross_amount DESC NULLS LAST;

SELECT
    order_id,
    match_number,
    started_at,
    completed_at,
    DATEDIFF('minute', started_at, completed_at) AS elapsed_minutes
FROM CORE.ORDER_STATUS_HISTORY
MATCH_RECOGNIZE (
    PARTITION BY order_id
    ORDER BY status_at
    MEASURES
        MATCH_NUMBER() AS match_number,
        FIRST(created.status_at) AS started_at,
        LAST(delivered.status_at) AS completed_at
    ONE ROW PER MATCH
    AFTER MATCH SKIP PAST LAST ROW
    PATTERN (created+ paid+ shipped* delivered)
    DEFINE
        created AS created.status = 'CREATED',
        paid AS paid.status = 'PAID',
        shipped AS shipped.status = 'SHIPPED',
        delivered AS delivered.status = 'DELIVERED'
);

SELECT
    *
FROM (
    SELECT
        region,
        locale,
        net_amount
    FROM MART.ORDER_FACTS
    WHERE order_date >= DATEADD('day', -30, CURRENT_DATE())
)
PIVOT (
    SUM(net_amount)
    FOR locale IN (
        'ja-JP' AS JA,
        'en-US' AS EN,
        'fr-FR' AS FR,
        'ar-SA' AS AR
    )
)
ORDER BY region;

SELECT
    customer_id,
    metric_name,
    metric_value
FROM MART.CUSTOMER_METRICS
UNPIVOT INCLUDE NULLS (
    metric_value
    FOR metric_name IN (
        lifetime_value,
        order_count,
        support_ticket_count,
        loyalty_points
    )
)
QUALIFY ROW_NUMBER() OVER (
    PARTITION BY customer_id, metric_name
    ORDER BY metric_value DESC NULLS LAST
) = 1;
