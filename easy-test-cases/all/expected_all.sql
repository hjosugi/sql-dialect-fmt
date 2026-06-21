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

-- Case 002: Recursive hierarchy rollup with cycle guard and grouped analytics.
WITH RECURSIVE org_tree AS (
    SELECT
        e.employee_id,
        e.manager_id,
        e.department_id,
        e.region,
        e.hire_date,
        e.employee_name,
        0 AS depth,
        ARRAY_CONSTRUCT(e.employee_id) AS path_ids,
        TO_VARCHAR(e.employee_id) AS path_text
    FROM HR.EMPLOYEES AS e
    WHERE e.manager_id IS NULL

    UNION ALL

    SELECT
        child.employee_id,
        child.manager_id,
        child.department_id,
        child.region,
        child.hire_date,
        child.employee_name,
        parent.depth + 1 AS depth,
        ARRAY_APPEND(parent.path_ids, child.employee_id) AS path_ids,
        parent.path_text || ' > ' || TO_VARCHAR(child.employee_id) AS path_text
    FROM HR.EMPLOYEES AS child
        INNER JOIN org_tree AS parent
            ON child.manager_id = parent.employee_id
    WHERE
        parent.depth < 30
        AND NOT ARRAY_CONTAINS(child.employee_id::VARIANT, parent.path_ids)
), enriched AS (
    SELECT
        t.*,
        d.department_name,
        DATEDIFF('day', t.hire_date, CURRENT_DATE()) AS tenure_days,
        COUNT(*) OVER (
            PARTITION BY t.manager_id
        ) AS sibling_count,
        DENSE_RANK() OVER (
            PARTITION BY t.department_id
            ORDER BY t.depth DESC, t.hire_date ASC
        ) AS depth_rank_in_department
    FROM org_tree AS t
        LEFT JOIN HR.DEPARTMENTS AS d
            ON t.department_id = d.department_id
), rollup_rows AS (
    SELECT
        region,
        department_name,
        depth,
        GROUPING(region) AS g_region,
        GROUPING(department_name) AS g_department,
        GROUPING(depth) AS g_depth,
        COUNT(*) AS employee_count,
        APPROX_COUNT_DISTINCT(manager_id) AS manager_count,
        MAX(depth) AS max_depth,
        AVG(tenure_days) AS avg_tenure_days,
        ARRAY_AGG(employee_name) WITHIN GROUP (ORDER BY depth DESC, hire_date ASC) AS sample_names
    FROM enriched
    GROUP BY ROLLUP(region, department_name, depth)
)
SELECT
    CASE WHEN g_region = 1 THEN '<ALL_REGIONS>' ELSE region END AS region_label,
    CASE WHEN g_department = 1 THEN '<ALL_DEPARTMENTS>' ELSE department_name END AS department_label,
    CASE WHEN g_depth = 1 THEN -1 ELSE depth END AS depth_bucket,
    employee_count,
    manager_count,
    max_depth,
    ROUND(avg_tenure_days, 2) AS avg_tenure_days,
    sample_names[0]::STRING AS first_sample_employee,
    RATIO_TO_REPORT(employee_count) OVER (
        PARTITION BY g_region, region
    ) AS share_in_region
FROM rollup_rows
WHERE employee_count >= 1
ORDER BY g_region, region_label, g_department, department_label, depth_bucket;

-- Case 003: MATCH_RECOGNIZE funnel pattern with measures and post-filtering.
WITH filtered_events AS (
    SELECT
        user_id,
        session_id,
        event_time,
        LOWER(event_name) AS event_name,
        page_url,
        TRY_TO_DECIMAL(properties:amount::STRING, 18, 2) AS amount,
        properties:currency::STRING AS currency_code,
        properties:error_code::STRING AS error_code
    FROM WEB.CLICKSTREAM_EVENTS
    WHERE
        event_time >= DATEADD('day', -14, CURRENT_TIMESTAMP())
        AND user_id IS NOT NULL
), matches AS (
    SELECT *
    FROM filtered_events
    MATCH_RECOGNIZE (
        PARTITION BY user_id, session_id
        ORDER BY event_time
        MEASURES
            MATCH_NUMBER() AS funnel_match_number,
            FIRST(login.event_time) AS login_at,
            LAST(success.event_time) AS success_at,
            COUNT(browse.*) AS browse_count,
            MAX(payment.amount) AS max_payment_amount,
            ARRAY_AGG(CLASSIFIER()) WITHIN GROUP (ORDER BY MATCH_SEQUENCE_NUMBER()) AS classifier_path
        ONE ROW PER MATCH
        AFTER MATCH SKIP TO NEXT ROW
        PATTERN (login browse{1,5} checkout? payment failure* success)
        DEFINE
            login AS event_name IN ('login', 'sign_in'),
            browse AS event_name IN ('view_item', 'search', 'category_view'),
            checkout AS event_name = 'checkout_started',
            payment AS event_name IN ('payment_submitted', 'payment_retry') AND amount IS NOT NULL,
            failure AS event_name = 'payment_failed' AND error_code IS NOT NULL,
            success AS event_name IN ('order_completed', 'subscription_started')
    )
)
SELECT
    user_id,
    session_id,
    funnel_match_number,
    login_at,
    success_at,
    DATEDIFF('second', login_at, success_at) AS seconds_to_success,
    browse_count,
    max_payment_amount,
    classifier_path,
    ROW_NUMBER() OVER (
        PARTITION BY user_id
        ORDER BY success_at DESC, funnel_match_number DESC
    ) AS latest_success_rank
FROM matches
QUALIFY latest_success_rank <= 3
ORDER BY user_id, latest_success_rank;

-- Case 004: PIVOT, UNPIVOT, and GROUPING SETS in one report.
WITH monthly AS (
    SELECT
        DATE_PART('year', order_date) AS order_year,
        DATE_PART('month', order_date) AS order_month,
        region,
        channel,
        SUM(net_amount) AS net_amount,
        COUNT(DISTINCT customer_id) AS buyers
    FROM MART.FACT_ORDERS
    WHERE order_date >= DATE_FROM_PARTS(YEAR(CURRENT_DATE()) - 1, 1, 1)
    GROUP BY order_year, order_month, region, channel
), pivoted AS (
    SELECT *
    FROM monthly
    PIVOT (
        SUM(net_amount) FOR order_month IN (
            1 AS JAN,
            2 AS FEB,
            3 AS MAR,
            4 AS APR,
            5 AS MAY,
            6 AS JUN,
            7 AS JUL,
            8 AS AUG,
            9 AS SEP,
            10 AS OCT,
            11 AS NOV,
            12 AS DEC
        )
    )
), unpivoted AS (
    SELECT
        order_year,
        region,
        channel,
        month_name,
        net_amount
    FROM pivoted
    UNPIVOT INCLUDE NULLS (
        net_amount FOR month_name IN (
            JAN,
            FEB,
            MAR,
            APR,
            MAY,
            JUN,
            JUL,
            AUG,
            SEP,
            OCT,
            NOV,
            DEC
        )
    )
), grouped AS (
    SELECT
        order_year,
        region,
        channel,
        month_name,
        GROUPING(order_year) AS g_year,
        GROUPING(region) AS g_region,
        GROUPING(channel) AS g_channel,
        GROUPING(month_name) AS g_month,
        SUM(net_amount) AS total_net_amount,
        AVG(NULLIF(net_amount, 0)) AS avg_active_month_amount
    FROM unpivoted
    GROUP BY GROUPING SETS (
        (order_year, region, channel, month_name),
        (order_year, region, channel),
        (order_year, region),
        (order_year),
        ()
    )
)
SELECT
    IFF(g_year = 1, '<ALL_YEARS>', TO_VARCHAR(order_year)) AS year_label,
    IFF(g_region = 1, '<ALL_REGIONS>', region) AS region_label,
    IFF(g_channel = 1, '<ALL_CHANNELS>', channel) AS channel_label,
    IFF(g_month = 1, '<ALL_MONTHS>', month_name) AS month_label,
    total_net_amount,
    avg_active_month_amount,
    RANK() OVER (
        PARTITION BY g_year, order_year, g_region, region
        ORDER BY total_net_amount DESC NULLS LAST
    ) AS amount_rank
FROM grouped
WHERE COALESCE(total_net_amount, 0) <> 0
ORDER BY year_label, region_label, channel_label, month_label;

-- Case 005: MERGE with nested source CTE, lateral flatten, deletes, updates, and inserts.
MERGE INTO CORE.CUSTOMER_PROFILE AS target
USING (
    WITH ranked_events AS (
        SELECT
            e.payload:customer:id::STRING AS customer_id,
            e.payload:customer:email::STRING AS email,
            e.payload:customer:name::STRING AS display_name,
            e.payload:customer:status::STRING AS customer_status,
            e.payload:customer:locale::STRING AS locale,
            e.payload:customer:marketing_opt_in::BOOLEAN AS marketing_opt_in,
            e.event_time,
            e.event_id,
            e.payload:customer:addresses AS addresses,
            ROW_NUMBER() OVER (
                PARTITION BY e.payload:customer:id::STRING
                ORDER BY e.event_time DESC, e.loaded_at DESC, e.event_id DESC
            ) AS row_num
        FROM RAW.CUSTOMER_EVENTS AS e
        WHERE e.payload:event_type::STRING IN ('customer.upserted', 'customer.deleted')
        QUALIFY row_num = 1
    ), address_summary AS (
        SELECT
            r.customer_id,
            COUNT_IF(a.value:type::STRING = 'shipping') AS shipping_address_count,
            MAX_BY(a.value:country::STRING, a.index) AS last_seen_country,
            ARRAY_AGG(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'type', a.value:type::STRING,
                    'country', a.value:country::STRING,
                    'postal_code', a.value:postal_code::STRING
                )
            ) WITHIN GROUP (ORDER BY a.index) AS normalized_addresses
        FROM ranked_events AS r,
            LATERAL FLATTEN(INPUT => r.addresses, OUTER => TRUE) AS a
        GROUP BY r.customer_id
    )
    SELECT
        r.customer_id,
        r.email,
        r.display_name,
        r.customer_status,
        COALESCE(r.locale, 'und') AS locale,
        COALESCE(r.marketing_opt_in, FALSE) AS marketing_opt_in,
        COALESCE(a.shipping_address_count, 0) AS shipping_address_count,
        a.last_seen_country,
        a.normalized_addresses,
        SHA2_HEX(
            TO_JSON(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'email', r.email,
                    'name', r.display_name,
                    'status', r.customer_status,
                    'locale', COALESCE(r.locale, 'und'),
                    'marketing', COALESCE(r.marketing_opt_in, FALSE),
                    'addresses', a.normalized_addresses
                )
            ),
            256
        ) AS row_hash,
        r.event_time,
        r.event_id
    FROM ranked_events AS r
        LEFT JOIN address_summary AS a
            ON r.customer_id = a.customer_id
) AS source
    ON target.customer_id = source.customer_id
WHEN MATCHED AND source.customer_status = 'deleted' THEN
    DELETE
WHEN MATCHED AND target.row_hash IS DISTINCT FROM source.row_hash THEN
    UPDATE SET
        target.email = source.email,
        target.display_name = source.display_name,
        target.customer_status = source.customer_status,
        target.locale = source.locale,
        target.marketing_opt_in = source.marketing_opt_in,
        target.shipping_address_count = source.shipping_address_count,
        target.last_seen_country = source.last_seen_country,
        target.normalized_addresses = source.normalized_addresses,
        target.row_hash = source.row_hash,
        target.source_event_id = source.event_id,
        target.updated_at = CURRENT_TIMESTAMP()
WHEN NOT MATCHED AND source.customer_status <> 'deleted' THEN
    INSERT (
        customer_id,
        email,
        display_name,
        customer_status,
        locale,
        marketing_opt_in,
        shipping_address_count,
        last_seen_country,
        normalized_addresses,
        row_hash,
        source_event_id,
        created_at,
        updated_at
    )
    VALUES (
        source.customer_id,
        source.email,
        source.display_name,
        source.customer_status,
        source.locale,
        source.marketing_opt_in,
        source.shipping_address_count,
        source.last_seen_country,
        source.normalized_addresses,
        source.row_hash,
        source.event_id,
        CURRENT_TIMESTAMP(),
        CURRENT_TIMESTAMP()
    );

-- Case 006: Multi-table INSERT FIRST with conditional routing and nested SELECT.
INSERT FIRST
    WHEN severity_score >= 90 AND is_replay = FALSE THEN
        INTO OPS.ALERT_CRITICAL (
            alert_id,
            tenant_id,
            event_id,
            severity_score,
            alert_payload,
            created_at
        )
        VALUES (
            UUID_STRING(),
            tenant_id,
            event_id,
            severity_score,
            alert_payload,
            CURRENT_TIMESTAMP()
        )
    WHEN severity_score >= 60 THEN
        INTO OPS.ALERT_WARNING (
            alert_id,
            tenant_id,
            event_id,
            severity_score,
            alert_payload,
            created_at
        )
        VALUES (
            UUID_STRING(),
            tenant_id,
            event_id,
            severity_score,
            alert_payload,
            CURRENT_TIMESTAMP()
        )
    ELSE
        INTO OPS.ALERT_INFO (
            alert_id,
            tenant_id,
            event_id,
            severity_score,
            alert_payload,
            created_at
        )
        VALUES (
            UUID_STRING(),
            tenant_id,
            event_id,
            severity_score,
            alert_payload,
            CURRENT_TIMESTAMP()
        )
SELECT
    tenant_id,
    event_id,
    is_replay,
    severity_score,
    OBJECT_CONSTRUCT_KEEP_NULL(
        'rule_id', rule_id,
        'rule_name', rule_name,
        'matched_terms', matched_terms,
        'source', source_context,
        'message', message
    ) AS alert_payload
FROM (
    SELECT
        e.payload:tenant::STRING AS tenant_id,
        e.event_id,
        COALESCE(e.payload:meta:is_replay::BOOLEAN, FALSE) AS is_replay,
        r.rule_id,
        r.rule_name,
        e.payload:message::STRING AS message,
        e.source_context,
        ARRAY_INTERSECTION(
            SPLIT(LOWER(e.payload:message::STRING), ' '),
            r.match_terms
        ) AS matched_terms,
        LEAST(
            100,
            r.base_score
            + 5 * ARRAY_SIZE(matched_terms)
            + IFF(e.payload:meta:vip::BOOLEAN, 10, 0)
        ) AS severity_score,
        ROW_NUMBER() OVER (
            PARTITION BY e.event_id, r.rule_id
            ORDER BY severity_score DESC, r.updated_at DESC
        ) AS rule_rank
    FROM RAW.EVENT_LANDING AS e
        INNER JOIN OPS.ALERT_RULES AS r
            ON r.enabled
            AND e.payload:event_type::STRING = r.event_type
    WHERE e.loaded_at >= DATEADD('minute', -30, CURRENT_TIMESTAMP())
    QUALIFY rule_rank = 1
);

-- Case 007: COPY INTO table with transformations, metadata columns, and tricky file options.
COPY INTO RAW.ORDER_JSON_LANDING (
    event_id,
    event_time,
    tenant_id,
    order_id,
    payload,
    source_filename,
    source_row_number,
    file_content_key,
    loaded_at
)
FROM (
    SELECT
        $1:event_id::STRING AS event_id,
        TRY_TO_TIMESTAMP_TZ($1:event_time::STRING) AS event_time,
        COALESCE($1:tenant::STRING, 'UNKNOWN') AS tenant_id,
        $1:order:id::STRING AS order_id,
        OBJECT_INSERT(
            $1,
            '_load_context',
            OBJECT_CONSTRUCT(
                'file', METADATA$FILENAME,
                'row', METADATA$FILE_ROW_NUMBER,
                'start_scan_time', METADATA$START_SCAN_TIME
            ),
            TRUE
        ) AS payload,
        METADATA$FILENAME AS source_filename,
        METADATA$FILE_ROW_NUMBER AS source_row_number,
        METADATA$FILE_CONTENT_KEY AS file_content_key,
        METADATA$START_SCAN_TIME AS loaded_at
    FROM @RAW.EXT_EVENT_STAGE/orders/2026/06/ (
        FILE_FORMAT => RAW.FF_JSON_LINES
    )
)
PATTERN = '.*orders_(jp|kr|sa|global)_[0-9]{8}_[0-9]{6}\\.jsonl(\\.gz)?'
ON_ERROR = 'SKIP_FILE_5%'
PURGE = FALSE
FORCE = FALSE
RETURN_FAILED_ONLY = FALSE
SIZE_LIMIT = 5368709120;

-- Case 008: COPY INTO location unload with partitioned paths and complex SELECT.
COPY INTO @MART.EXPORT_STAGE/customer_score_daily/
FROM (
    WITH latest AS (
        SELECT
            customer_id,
            score_date,
            model_version,
            score,
            percentile,
            OBJECT_CONSTRUCT_KEEP_NULL(
                'segment', segment,
                'region', region,
                'locale', locale,
                'explain', explanation
            ) AS detail
        FROM MART.CUSTOMER_SCORE_DAILY
        WHERE score_date BETWEEN DATEADD('day', -7, CURRENT_DATE()) AND CURRENT_DATE()
        QUALIFY ROW_NUMBER() OVER (
            PARTITION BY customer_id, score_date
            ORDER BY updated_at DESC, model_version DESC
        ) = 1
    )
    SELECT
        score_date,
        customer_id,
        model_version,
        score,
        percentile,
        TO_JSON(detail) AS detail_json,
        CURRENT_TIMESTAMP() AS exported_at
    FROM latest
    WHERE score IS NOT NULL
)
PARTITION BY ('score_date=' || TO_VARCHAR(score_date, 'YYYY-MM-DD'))
FILE_FORMAT = (
    TYPE = CSV
    COMPRESSION = GZIP
    FIELD_DELIMITER = '|'
    RECORD_DELIMITER = '\n'
    FIELD_OPTIONALLY_ENCLOSED_BY = '"'
    NULL_IF = ('', 'NULL', '\\N')
)
HEADER = TRUE
OVERWRITE = TRUE
MAX_FILE_SIZE = 268435456
DETAILED_OUTPUT = TRUE;

-- Case 009: Masking policy, row access policy, tags, and protected table DDL.
CREATE OR REPLACE MASKING POLICY GOVERNANCE.MASK_EMAIL_CONDITIONAL AS (
    email_value STRING,
    tenant_id STRING
)
RETURNS STRING ->
    CASE
        WHEN IS_ROLE_IN_SESSION('SECURITY_ADMIN') THEN email_value
        WHEN EXISTS (
            SELECT 1
            FROM GOVERNANCE.TENANT_PRIVILEGE_MAP AS m
            WHERE
                m.tenant_id = tenant_id
                AND IS_DATABASE_ROLE_IN_SESSION(m.database_role_name)
        ) THEN email_value
        WHEN email_value IS NULL THEN NULL
        ELSE REGEXP_REPLACE(email_value, '(^.).*(@.*$)', '\\1***\\2')
    END
COMMENT = 'Conditional email masking / メールマスク / إخفاء البريد'
EXEMPT_OTHER_POLICIES = TRUE;

CREATE OR REPLACE ROW ACCESS POLICY GOVERNANCE.RAP_TENANT_REGION AS (
    tenant_id STRING,
    region STRING
)
RETURNS BOOLEAN ->
    IS_ROLE_IN_SESSION('ACCOUNTADMIN')
    OR EXISTS (
        SELECT 1
        FROM GOVERNANCE.ROLE_TENANT_REGION_MAP AS m
        WHERE
            m.database_role_name = CURRENT_ROLE()
            AND m.tenant_id = tenant_id
            AND (
                m.region = region
                OR m.region = '*'
            )
    )
COMMENT = 'Tenant and region row access policy';

CREATE OR REPLACE TABLE CORE.PROTECTED_CUSTOMER_PROFILE (
    customer_id STRING NOT NULL COMMENT 'surrogate customer id',
    tenant_id STRING NOT NULL,
    region STRING NOT NULL,
    email STRING WITH MASKING POLICY GOVERNANCE.MASK_EMAIL_CONDITIONAL USING (email, tenant_id),
    display_name STRING,
    locale STRING DEFAULT 'und',
    profile VARIANT,
    created_at TIMESTAMP_TZ DEFAULT CURRENT_TIMESTAMP(),
    updated_at TIMESTAMP_TZ,
    CONSTRAINT PK_PROTECTED_CUSTOMER_PROFILE PRIMARY KEY (customer_id) NOT ENFORCED
)
CLUSTER BY (tenant_id, region, DATE_TRUNC('day', created_at))
ROW ACCESS POLICY GOVERNANCE.RAP_TENANT_REGION ON (tenant_id, region)
WITH TAG (
    GOVERNANCE.DATA_CLASSIFICATION = 'confidential',
    GOVERNANCE.OWNER = 'customer-platform'
)
COMMENT = 'Protected customer profile table with masking and row access policy';

-- Case 010: Dynamic table with nested query, target lag, cluster key, and frozen region.
CREATE OR REPLACE DYNAMIC TABLE MART.DT_CUSTOMER_360_DAILY (
    snapshot_date DATE,
    tenant_id STRING,
    customer_id STRING,
    region STRING,
    order_count NUMBER,
    gross_amount NUMBER(18, 4),
    last_order_at TIMESTAMP_TZ,
    support_ticket_count NUMBER,
    risk_flags ARRAY,
    customer_features OBJECT
)
TARGET_LAG = '30 minutes'
WAREHOUSE = FORMATTER_TEST_WH
REFRESH_MODE = AUTO
INITIALIZE = ON_CREATE
CLUSTER BY (snapshot_date, tenant_id, region)
FROZEN WHERE (snapshot_date < CURRENT_DATE() - 30)
WITH TAG (
    GOVERNANCE.PIPELINE = 'customer_360',
    GOVERNANCE.QUALITY = 'gold'
)
AS
WITH orders AS (
    SELECT
        order_date AS snapshot_date,
        tenant_id,
        customer_id,
        region,
        COUNT(*) AS order_count,
        SUM(gross_amount) AS gross_amount,
        MAX(order_at) AS last_order_at
    FROM CORE.FACT_ORDER
    WHERE order_date >= CURRENT_DATE() - 400
    GROUP BY order_date, tenant_id, customer_id, region
), tickets AS (
    SELECT
        DATE_TRUNC('day', created_at)::DATE AS snapshot_date,
        tenant_id,
        customer_id,
        COUNT_IF(status <> 'closed') AS support_ticket_count,
        ARRAY_AGG(DISTINCT severity) WITHIN GROUP (ORDER BY severity) AS ticket_severities
    FROM CORE.SUPPORT_TICKET
    WHERE created_at >= DATEADD('day', -400, CURRENT_TIMESTAMP())
    GROUP BY 1, tenant_id, customer_id
), risk AS (
    SELECT
        tenant_id,
        customer_id,
        ARRAY_AGG(flag) WITHIN GROUP (ORDER BY weight DESC, flag) AS risk_flags
    FROM CORE.CUSTOMER_RISK_FLAG
    WHERE active
    GROUP BY tenant_id, customer_id
)
SELECT
    o.snapshot_date,
    o.tenant_id,
    o.customer_id,
    o.region,
    o.order_count,
    o.gross_amount,
    o.last_order_at,
    COALESCE(t.support_ticket_count, 0) AS support_ticket_count,
    COALESCE(r.risk_flags, ARRAY_CONSTRUCT()) AS risk_flags,
    OBJECT_CONSTRUCT_KEEP_NULL(
        'ticket_severities', t.ticket_severities,
        'avg_order_value', DIV0(o.gross_amount, o.order_count),
        'days_since_last_order', DATEDIFF('day', o.last_order_at, CURRENT_TIMESTAMP())
    ) AS customer_features
FROM orders AS o
    LEFT JOIN tickets AS t
        ON o.snapshot_date = t.snapshot_date
        AND o.tenant_id = t.tenant_id
        AND o.customer_id = t.customer_id
    LEFT JOIN risk AS r
        ON o.tenant_id = r.tenant_id
        AND o.customer_id = r.customer_id;

-- Case 011: Streams and task graph with WHEN conditions and finalizer task.
CREATE OR REPLACE STREAM RAW.ORDER_EVENT_STREAM
    ON TABLE RAW.ORDER_JSON_LANDING
    APPEND_ONLY = FALSE
    SHOW_INITIAL_ROWS = TRUE
    COMMENT = 'Order landing CDC stream / 注文CDC';

CREATE OR REPLACE TASK OPS.TASK_ORDER_PIPELINE_ROOT
    WAREHOUSE = FORMATTER_TEST_WH
    SCHEDULE = 'USING CRON */15 * * * * UTC'
    USER_TASK_TIMEOUT_MS = 1800000
    SUSPEND_TASK_AFTER_NUM_FAILURES = 5
    TASK_AUTO_RETRY_ATTEMPTS = 2
    COMMENT = 'Root task for order pipeline graph'
AS
    INSERT INTO OPS.PIPELINE_RUN_LOG (
        run_id,
        pipeline_name,
        status,
        started_at,
        context
    )
    SELECT
        UUID_STRING(),
        'order_pipeline',
        'STARTED',
        CURRENT_TIMESTAMP(),
        OBJECT_CONSTRUCT('source', 'task_graph', 'stream_has_data', SYSTEM$STREAM_HAS_DATA('RAW.ORDER_EVENT_STREAM'));

CREATE OR REPLACE TASK OPS.TASK_ORDER_PIPELINE_APPLY
    WAREHOUSE = FORMATTER_TEST_WH
    AFTER OPS.TASK_ORDER_PIPELINE_ROOT
    WHEN SYSTEM$STREAM_HAS_DATA('RAW.ORDER_EVENT_STREAM')
AS
    CALL OPS.SP_APPLY_ORDER_STREAM('RAW.ORDER_EVENT_STREAM');

CREATE OR REPLACE TASK OPS.TASK_ORDER_PIPELINE_REFRESH_MART
    WAREHOUSE = FORMATTER_TEST_WH
    AFTER OPS.TASK_ORDER_PIPELINE_APPLY
AS
    CALL OPS.SP_REFRESH_ORDER_MARTS('MART.DT_CUSTOMER_360_DAILY');

CREATE OR REPLACE TASK OPS.TASK_ORDER_PIPELINE_FINALIZER
    WAREHOUSE = FORMATTER_TEST_WH
    FINALIZE = OPS.TASK_ORDER_PIPELINE_ROOT
AS
    INSERT INTO OPS.PIPELINE_RUN_LOG (
        run_id,
        pipeline_name,
        status,
        finished_at,
        context
    )
    SELECT
        UUID_STRING(),
        'order_pipeline',
        'FINISHED',
        CURRENT_TIMESTAMP(),
        OBJECT_CONSTRUCT('finalizer', TRUE, 'query_id', LAST_QUERY_ID());

-- Case 012: Snowflake Scripting procedure with nested loops, transactions, cursors, and exceptions.
CREATE OR REPLACE PROCEDURE OPS.SP_REBUILD_TENANT_DAILY(
    P_TENANT_ID STRING,
    P_FROM_DATE DATE,
    P_TO_DATE DATE,
    P_DRY_RUN BOOLEAN DEFAULT FALSE
)
RETURNS VARIANT
LANGUAGE SQL
STRICT
EXECUTE AS OWNER
AS
$$
DECLARE
    v_run_id STRING DEFAULT UUID_STRING();
    v_current_date DATE;
    v_rows NUMBER DEFAULT 0;
    v_total_rows NUMBER DEFAULT 0;
    v_sql STRING;
    v_stage RESULTSET;
    c_regions CURSOR FOR
        SELECT DISTINCT region
        FROM CORE.TENANT_REGION
        WHERE tenant_id = ?
        ORDER BY region;
    e_bad_range EXCEPTION (-20010, 'Invalid date range');
BEGIN
    IF (P_FROM_DATE IS NULL OR P_TO_DATE IS NULL OR P_FROM_DATE > P_TO_DATE) THEN
        RAISE e_bad_range;
    END IF;

    INSERT INTO OPS.PIPELINE_RUN_LOG (run_id, pipeline_name, tenant_id, status, started_at)
    VALUES (:v_run_id, 'tenant_daily_rebuild', :P_TENANT_ID, 'STARTED', CURRENT_TIMESTAMP());

    BEGIN TRANSACTION;

    FOR region_record IN c_regions USING (P_TENANT_ID) DO
        v_current_date := P_FROM_DATE;

        WHILE (v_current_date <= P_TO_DATE) DO
            BEGIN
                DELETE FROM MART.TENANT_DAILY_METRIC
                WHERE
                    tenant_id = :P_TENANT_ID
                    AND region = :region_record.region
                    AND metric_date = :v_current_date;

                INSERT INTO MART.TENANT_DAILY_METRIC (
                    tenant_id,
                    region,
                    metric_date,
                    metric_name,
                    metric_value,
                    run_id,
                    created_at
                )
                WITH base AS (
                    SELECT
                        tenant_id,
                        region,
                        order_date,
                        COUNT(*) AS order_count,
                        SUM(net_amount) AS net_amount,
                        COUNT(DISTINCT customer_id) AS buyer_count
                    FROM CORE.FACT_ORDER
                    WHERE
                        tenant_id = :P_TENANT_ID
                        AND region = :region_record.region
                        AND order_date = :v_current_date
                    GROUP BY tenant_id, region, order_date
                )
                SELECT tenant_id, region, order_date, 'order_count', order_count, :v_run_id, CURRENT_TIMESTAMP() FROM base
                UNION ALL
                SELECT tenant_id, region, order_date, 'net_amount', net_amount, :v_run_id, CURRENT_TIMESTAMP() FROM base
                UNION ALL
                SELECT tenant_id, region, order_date, 'buyer_count', buyer_count, :v_run_id, CURRENT_TIMESTAMP() FROM base;

                v_rows := SQLROWCOUNT;
                v_total_rows := v_total_rows + v_rows;
            EXCEPTION
                WHEN STATEMENT_ERROR CONTINUE THEN
                    INSERT INTO OPS.PIPELINE_ERROR_LOG (
                        run_id,
                        tenant_id,
                        region,
                        error_code,
                        error_message,
                        error_state,
                        context,
                        created_at
                    )
                    SELECT
                        :v_run_id,
                        :P_TENANT_ID,
                        :region_record.region,
                        :SQLCODE,
                        :SQLERRM,
                        :SQLSTATE,
                        OBJECT_CONSTRUCT('metric_date', :v_current_date),
                        CURRENT_TIMESTAMP();
            END;

            v_current_date := DATEADD('day', 1, v_current_date);
        END WHILE;
    END FOR;

    v_sql := 'SELECT metric_name, SUM(metric_value) AS metric_value '
        || 'FROM MART.TENANT_DAILY_METRIC '
        || 'WHERE tenant_id = ? AND metric_date BETWEEN ? AND ? '
        || 'GROUP BY metric_name ORDER BY metric_name';

    v_stage := (EXECUTE IMMEDIATE :v_sql USING (P_TENANT_ID, P_FROM_DATE, P_TO_DATE));

    IF (P_DRY_RUN) THEN
        ROLLBACK;
    ELSE
        COMMIT;
    END IF;

    UPDATE OPS.PIPELINE_RUN_LOG
    SET
        status = IFF(:P_DRY_RUN, 'ROLLED_BACK', 'FINISHED'),
        finished_at = CURRENT_TIMESTAMP(),
        context = OBJECT_CONSTRUCT('rows_written', :v_total_rows)
    WHERE run_id = :v_run_id;

    RETURN OBJECT_CONSTRUCT(
        'run_id', v_run_id,
        'tenant_id', P_TENANT_ID,
        'from_date', P_FROM_DATE,
        'to_date', P_TO_DATE,
        'dry_run', P_DRY_RUN,
        'rows_written', v_total_rows
    );
END;
$$;

-- Case 013: JavaScript stored procedure with dynamic SQL, template strings, binds, and result handling.
CREATE OR REPLACE PROCEDURE OPS.SP_JS_DEEP_SCHEMA_PROFILE(
    P_DATABASE STRING,
    P_SCHEMA_PATTERN STRING DEFAULT '%',
    P_INCLUDE_COLUMNS BOOLEAN DEFAULT TRUE
)
RETURNS VARIANT
LANGUAGE JAVASCRIPT
STRICT
EXECUTE AS CALLER
AS
$$
function quoteIdent(value) {
    if (value === null || value === undefined || !/^[A-Za-z_][A-Za-z0-9_$]*$/.test(String(value))) {
        throw new Error("Unsafe identifier / 危険な識別子: " + value);
    }
    return '"' + String(value).replace(/"/g, '""') + '"';
}

const db = quoteIdent(P_DATABASE);
const sqlText = `
    WITH tables AS (
        SELECT
            table_catalog,
            table_schema,
            table_name,
            table_type,
            row_count,
            bytes,
            created,
            last_altered,
            comment,
            ROW_NUMBER() OVER (
                PARTITION BY table_schema
                ORDER BY bytes DESC NULLS LAST, table_name
            ) AS size_rank
        FROM ${db}.INFORMATION_SCHEMA.TABLES
        WHERE table_schema ILIKE ?
        QUALIFY size_rank <= 100
    ), columns AS (
        SELECT
            table_catalog,
            table_schema,
            table_name,
            ARRAY_AGG(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'name', column_name,
                    'type', data_type,
                    'nullable', is_nullable,
                    'ordinal', ordinal_position,
                    'comment', comment
                )
            ) WITHIN GROUP (ORDER BY ordinal_position) AS column_docs
        FROM ${db}.INFORMATION_SCHEMA.COLUMNS
        WHERE ? AND table_schema ILIKE ?
        GROUP BY table_catalog, table_schema, table_name
    )
    SELECT
        t.table_schema,
        ARRAY_AGG(
            OBJECT_CONSTRUCT_KEEP_NULL(
                'name', t.table_name,
                'type', t.table_type,
                'rows', t.row_count,
                'bytes', t.bytes,
                'created', t.created,
                'last_altered', t.last_altered,
                'comment', t.comment,
                'columns', c.column_docs
            )
        ) WITHIN GROUP (ORDER BY t.size_rank, t.table_name) AS objects
    FROM tables AS t
        LEFT JOIN columns AS c
            ON t.table_catalog = c.table_catalog
            AND t.table_schema = c.table_schema
            AND t.table_name = c.table_name
    GROUP BY t.table_schema
    ORDER BY t.table_schema
`;

const stmt = snowflake.createStatement({
    sqlText: sqlText,
    binds: [P_SCHEMA_PATTERN, P_INCLUDE_COLUMNS, P_SCHEMA_PATTERN]
});
const rs = stmt.execute();
const schemas = {};
let totalObjects = 0;

while (rs.next()) {
    const schemaName = rs.getColumnValue(1);
    const objects = rs.getColumnValue(2) || [];
    schemas[schemaName] = {
        object_count: objects.length,
        objects: objects,
        note: "schema profile / スキーマプロファイル / ملف المخطط"
    };
    totalObjects += objects.length;
}

snowflake.log("info", `Profile completed: db=${P_DATABASE}, objects=${totalObjects}`);
return {
    status: "OK",
    database: P_DATABASE,
    schema_pattern: P_SCHEMA_PATTERN,
    include_columns: P_INCLUDE_COLUMNS,
    total_objects: totalObjects,
    schemas: schemas,
    query_id: stmt.getQueryId(),
    generated_at: new Date().toISOString()
};
$$;

-- Case 014: Python Snowpark procedure with multilingual text profiling and nested DataFrame SQL.
CREATE OR REPLACE PROCEDURE OPS.SP_PY_PROFILE_TEXT_COLUMNS(
    P_SOURCE_TABLE STRING,
    P_TEXT_COLUMN STRING,
    P_LIMIT NUMBER DEFAULT 5000
)
RETURNS VARIANT
LANGUAGE PYTHON
RUNTIME_VERSION = '3.12'
PACKAGES = ('snowflake-snowpark-python')
HANDLER = 'main'
EXECUTE AS CALLER
AS
$$
import re
import unicodedata
from snowflake.snowpark import Session
from snowflake.snowpark.functions import col, length, lit, regexp_count

SCRIPT_RE = {
    "latin": re.compile(r"[A-Za-z]"),
    "hiragana": re.compile(r"[\u3040-\u309F]"),
    "katakana": re.compile(r"[\u30A0-\u30FF]"),
    "hangul": re.compile(r"[\uAC00-\uD7AF]"),
    "arabic": re.compile(r"[\u0600-\u06FF]"),
}

def normalize_text(value):
    if value is None:
        return ""
    return unicodedata.normalize("NFKC", str(value)).strip()

def detect_scripts(value):
    normalized = normalize_text(value)
    return [name for name, pattern in SCRIPT_RE.items() if pattern.search(normalized)]

def main(session: Session, p_source_table: str, p_text_column: str, p_limit: int):
    quoted_table = '"' + p_source_table.replace('"', '""') + '"' if '.' not in p_source_table else p_source_table
    sql = f'''
        SELECT
            {p_text_column} AS text_value,
            COUNT(*) AS row_count,
            MIN(created_at) AS first_seen_at,
            MAX(created_at) AS last_seen_at
        FROM {quoted_table}
        WHERE {p_text_column} IS NOT NULL
        GROUP BY {p_text_column}
        ORDER BY row_count DESC, text_value
        LIMIT ?
    '''
    rows = session.sql(sql, params=[int(p_limit)]).collect()
    summary = {
        "source_table": p_source_table,
        "text_column": p_text_column,
        "sample_size": len(rows),
        "scripts": {},
        "examples": []
    }

    for row in rows:
        text = normalize_text(row["TEXT_VALUE"])
        scripts = detect_scripts(text)
        for script in scripts or ["unknown"]:
            summary["scripts"].setdefault(script, 0)
            summary["scripts"][script] += int(row["ROW_COUNT"])
        if len(summary["examples"]) < 25:
            summary["examples"].append({
                "text": text,
                "scripts": scripts,
                "row_count": int(row["ROW_COUNT"]),
                "first_seen_at": str(row["FIRST_SEEN_AT"]),
                "last_seen_at": str(row["LAST_SEEN_AT"])
            })

    return summary
$$;

-- Case 015: Anonymous procedure with WITH ... AS PROCEDURE and nested SQL block.
WITH RUN_BACKFILL AS PROCEDURE (
    P_TENANT_ID STRING,
    P_BACKFILL_DATE DATE
)
RETURNS VARIANT
LANGUAGE SQL
AS
$$
DECLARE
    v_inserted NUMBER DEFAULT 0;
    v_deleted NUMBER DEFAULT 0;
BEGIN
    DELETE FROM QA.BACKFILL_PREVIEW
    WHERE
        tenant_id = :P_TENANT_ID
        AND business_date = :P_BACKFILL_DATE;
    v_deleted := SQLROWCOUNT;

    INSERT INTO QA.BACKFILL_PREVIEW (
        tenant_id,
        business_date,
        metric_name,
        metric_value,
        detail,
        created_at
    )
    WITH base AS (
        SELECT
            tenant_id,
            order_date,
            COUNT(*) AS order_count,
            SUM(net_amount) AS net_amount,
            COUNT(DISTINCT customer_id) AS buyer_count
        FROM CORE.FACT_ORDER
        WHERE
            tenant_id = :P_TENANT_ID
            AND order_date = :P_BACKFILL_DATE
        GROUP BY tenant_id, order_date
    )
    SELECT tenant_id, order_date, 'order_count', order_count, OBJECT_CONSTRUCT(), CURRENT_TIMESTAMP() FROM base
    UNION ALL
    SELECT tenant_id, order_date, 'net_amount', net_amount, OBJECT_CONSTRUCT(), CURRENT_TIMESTAMP() FROM base
    UNION ALL
    SELECT tenant_id, order_date, 'buyer_count', buyer_count, OBJECT_CONSTRUCT(), CURRENT_TIMESTAMP() FROM base;
    v_inserted := SQLROWCOUNT;

    RETURN OBJECT_CONSTRUCT(
        'tenant_id', P_TENANT_ID,
        'business_date', P_BACKFILL_DATE,
        'deleted', v_deleted,
        'inserted', v_inserted
    );
END;
$$
CALL RUN_BACKFILL('TENANT-JP-001', DATE '2026-06-21');

-- Case 016: SQL, JavaScript, and Python UDF/UDTF definitions in one file.
CREATE OR REPLACE FUNCTION UTIL.FN_JSON_LABELS(P_PAYLOAD VARIANT)
RETURNS TABLE (
    label_key STRING,
    label_value STRING,
    language_code STRING,
    confidence FLOAT
)
LANGUAGE SQL
AS
$$
    SELECT
        f.key::STRING AS label_key,
        f.value:value::STRING AS label_value,
        COALESCE(f.value:lang::STRING, 'und') AS language_code,
        TRY_TO_DOUBLE(f.value:confidence::STRING) AS confidence
    FROM TABLE(FLATTEN(INPUT => P_PAYLOAD:labels, OUTER => TRUE)) AS f
    WHERE f.key IS NOT NULL
$$;

CREATE OR REPLACE FUNCTION UTIL.FN_NORMALIZE_PHONE(P_VALUE STRING)
RETURNS STRING
LANGUAGE JAVASCRIPT
AS
$$
if (P_VALUE === null) {
    return null;
}
const digits = String(P_VALUE).replace(/[^0-9+]/g, "");
if (digits.startsWith("+")) {
    return digits;
}
if (digits.startsWith("81")) {
    return "+" + digits;
}
if (digits.startsWith("0")) {
    return "+81" + digits.substring(1);
}
return digits;
$$;

CREATE OR REPLACE FUNCTION UTIL.FN_SAFE_SLUG(P_VALUE STRING)
RETURNS STRING
LANGUAGE PYTHON
RUNTIME_VERSION = '3.12'
HANDLER = 'slugify'
AS
$$
import re
import unicodedata

def slugify(value):
    if value is None:
        return None
    text = unicodedata.normalize("NFKC", str(value)).lower()
    text = re.sub(r"[^a-z0-9]+", "-", text)
    text = re.sub(r"^-+|-+$", "", text)
    return text or None
$$;

-- Case 017: Snowpipe CREATE PIPE with auto ingest and complex COPY body.
CREATE OR REPLACE PIPE RAW.PIPE_ORDER_EVENTS_AUTO
    AUTO_INGEST = TRUE
    ERROR_INTEGRATION = OPS.NOTIFICATION_INT
    COMMENT = 'Auto ingest order events from cloud notification'
AS
COPY INTO RAW.ORDER_JSON_LANDING (
    event_id,
    event_time,
    tenant_id,
    order_id,
    payload,
    source_filename,
    source_row_number,
    file_content_key,
    loaded_at
)
FROM (
    SELECT
        $1:event_id::STRING,
        TRY_TO_TIMESTAMP_TZ($1:event_time::STRING),
        COALESCE($1:tenant::STRING, 'UNKNOWN'),
        $1:order:id::STRING,
        $1,
        METADATA$FILENAME,
        METADATA$FILE_ROW_NUMBER,
        METADATA$FILE_CONTENT_KEY,
        METADATA$START_SCAN_TIME
    FROM @RAW.EXT_EVENT_STAGE/orders/ (
        FILE_FORMAT => RAW.FF_JSON_LINES
    )
)
PATTERN = '.*orders/.*/event_[0-9]{14}_[a-z0-9-]+\\.jsonl(\\.gz)?'
ON_ERROR = 'CONTINUE';

-- Case 018: CREATE ALERT with EXISTS condition and notification procedure call.
CREATE OR REPLACE ALERT OPS.ALERT_PIPELINE_SLA_BREACH
    WAREHOUSE = FORMATTER_TEST_WH
    SCHEDULE = 'USING CRON */10 * * * * UTC'
    COMMENT = 'Alert when active pipeline runs exceed SLA'
IF (EXISTS (
    SELECT 1
    FROM OPS.PIPELINE_RUN_LOG AS r
    WHERE
        r.status = 'STARTED'
        AND DATEDIFF('minute', r.started_at, CURRENT_TIMESTAMP()) > COALESCE(r.sla_minutes, 60)
        AND NOT EXISTS (
            SELECT 1
            FROM OPS.PIPELINE_ALERT_SUPPRESSION AS s
            WHERE
                s.pipeline_name = r.pipeline_name
                AND CURRENT_TIMESTAMP() BETWEEN s.suppressed_from AND s.suppressed_to
        )
))
THEN
    CALL OPS.SP_SEND_PIPELINE_ALERT(
        'SLA_BREACH',
        (
            SELECT ARRAY_AGG(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'run_id', run_id,
                    'pipeline', pipeline_name,
                    'tenant', tenant_id,
                    'started_at', started_at,
                    'age_minutes', DATEDIFF('minute', started_at, CURRENT_TIMESTAMP())
                )
            )
            FROM OPS.PIPELINE_RUN_LOG
            WHERE
                status = 'STARTED'
                AND DATEDIFF('minute', started_at, CURRENT_TIMESTAMP()) > COALESCE(sla_minutes, 60)
        )
    );

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

-- Case 020: Time travel, clone, swap, result scan, and rollback-friendly repair SQL.
CREATE OR REPLACE TRANSIENT TABLE QA.FACT_ORDER_REPAIR_CLONE
    CLONE CORE.FACT_ORDER
    AT (TIMESTAMP => TO_TIMESTAMP_TZ('2026-06-21 00:00:00 +0900'));

INSERT INTO QA.FACT_ORDER_REPAIR_AUDIT (
    audit_id,
    order_id,
    old_status,
    new_status,
    repair_reason,
    created_at
)
SELECT
    UUID_STRING(),
    broken.order_id,
    broken.order_status,
    clone.order_status,
    'restore status from time travel clone',
    CURRENT_TIMESTAMP()
FROM CORE.FACT_ORDER AS broken
    INNER JOIN QA.FACT_ORDER_REPAIR_CLONE AS clone
        ON broken.order_id = clone.order_id
WHERE
    broken.order_status = 'UNKNOWN'
    AND clone.order_status <> 'UNKNOWN';

UPDATE CORE.FACT_ORDER AS target
SET
    order_status = source.order_status,
    updated_at = CURRENT_TIMESTAMP(),
    repair_context = OBJECT_CONSTRUCT('source', 'time_travel_clone', 'query_id', LAST_QUERY_ID())
FROM QA.FACT_ORDER_REPAIR_CLONE AS source
WHERE
    target.order_id = source.order_id
    AND target.order_status = 'UNKNOWN'
    AND source.order_status <> 'UNKNOWN';

CREATE OR REPLACE TABLE QA.FACT_ORDER_BEFORE_BAD_DEPLOY AS
SELECT *
FROM CORE.FACT_ORDER BEFORE (STATEMENT => '01b4d67a-0001-0602-0000-000000000123')
WHERE updated_at >= DATEADD('hour', -6, CURRENT_TIMESTAMP());

ALTER TABLE QA.FACT_ORDER_REPAIR_CLONE SWAP WITH QA.FACT_ORDER_BEFORE_BAD_DEPLOY;

-- Case 021: Secure view with nested CTEs, lateral flatten, masking-aware columns, and comments.
CREATE OR REPLACE SECURE VIEW MART.VW_CUSTOMER_SUPPORT_CONTEXT
COPY GRANTS
COMMENT = 'Secure support context view / セキュア問い合わせ文脈ビュー'
AS
WITH latest_profile AS (
    SELECT
        customer_id,
        tenant_id,
        region,
        email,
        display_name,
        profile,
        updated_at
    FROM CORE.PROTECTED_CUSTOMER_PROFILE
    QUALIFY ROW_NUMBER() OVER (
        PARTITION BY customer_id
        ORDER BY updated_at DESC NULLS LAST, created_at DESC
    ) = 1
), open_tickets AS (
    SELECT
        t.ticket_id,
        t.customer_id,
        t.tenant_id,
        t.created_at,
        t.status,
        t.severity,
        t.subject,
        c.value:category::STRING AS category,
        c.value:confidence::FLOAT AS category_confidence
    FROM CORE.SUPPORT_TICKET AS t,
        LATERAL FLATTEN(INPUT => t.classification:categories, OUTER => TRUE) AS c
    WHERE t.status <> 'closed'
), ticket_rollup AS (
    SELECT
        customer_id,
        tenant_id,
        COUNT(*) AS open_ticket_count,
        MAX(created_at) AS latest_ticket_at,
        ARRAY_AGG(
            OBJECT_CONSTRUCT_KEEP_NULL(
                'ticket_id', ticket_id,
                'severity', severity,
                'subject', subject,
                'category', category,
                'confidence', category_confidence
            )
        ) WITHIN GROUP (ORDER BY created_at DESC, ticket_id) AS tickets
    FROM open_tickets
    GROUP BY customer_id, tenant_id
)
SELECT
    p.tenant_id,
    p.region,
    p.customer_id,
    p.email,
    p.display_name,
    COALESCE(r.open_ticket_count, 0) AS open_ticket_count,
    r.latest_ticket_at,
    COALESCE(r.tickets, ARRAY_CONSTRUCT()) AS tickets,
    OBJECT_CONSTRUCT_KEEP_NULL(
        'locale', p.profile:locale::STRING,
        'vip', p.profile:flags:vip::BOOLEAN,
        'last_login_at', p.profile:activity:last_login_at::STRING
    ) AS profile_summary
FROM latest_profile AS p
    LEFT JOIN ticket_rollup AS r
        ON p.customer_id = r.customer_id
        AND p.tenant_id = r.tenant_id;

-- Case 022: ASOF JOIN, RESAMPLE, window interpolation, and device time-series cleanup.
WITH filtered AS (
    SELECT
        device_id,
        reading_at,
        metric_name,
        TRY_TO_DOUBLE(metric_value) AS metric_value,
        metadata
    FROM IOT.SENSOR_READING
    WHERE
        reading_at >= DATEADD('hour', -24, CURRENT_TIMESTAMP())
        AND metric_name IN ('temperature', 'humidity', 'pressure')
), sampled AS (
    SELECT
        device_id,
        reading_at,
        metric_name,
        metric_value,
        INTERPOLATE_FFILL(metric_value) OVER (
            PARTITION BY device_id, metric_name
            ORDER BY reading_at
        ) AS metric_value_filled
    FROM filtered
    RESAMPLE (
        USING reading_at
        INCREMENT BY INTERVAL '5 minutes'
        PARTITION BY device_id, metric_name
    )
), calibrated AS (
    SELECT
        s.device_id,
        s.reading_at,
        s.metric_name,
        s.metric_value_filled,
        c.calibration_id,
        c.offset_value,
        c.scale_value,
        s.metric_value_filled * COALESCE(c.scale_value, 1) + COALESCE(c.offset_value, 0) AS calibrated_value
    FROM sampled AS s
        ASOF JOIN IOT.DEVICE_CALIBRATION AS c
            MATCH_CONDITION (s.reading_at >= c.calibrated_at)
            ON s.device_id = c.device_id
            AND s.metric_name = c.metric_name
)
SELECT
    device_id,
    DATE_TRUNC('hour', reading_at) AS reading_hour,
    metric_name,
    AVG(calibrated_value) AS avg_calibrated_value,
    MIN(calibrated_value) AS min_calibrated_value,
    MAX(calibrated_value) AS max_calibrated_value,
    COUNT(*) AS sample_count,
    ANY_VALUE(calibration_id) AS sample_calibration_id
FROM calibrated
GROUP BY device_id, reading_hour, metric_name
ORDER BY device_id, reading_hour, metric_name;

-- Case 023: CREATE SEMANTIC VIEW with tables, relationships, facts, dimensions, metrics, and verified queries.
CREATE OR REPLACE SEMANTIC VIEW SEMANTIC.SV_CUSTOMER_REVENUE
TABLES (
    orders AS MART.FACT_ORDER PRIMARY KEY (order_id) WITH SYNONYMS ('orders', 'purchases') COMMENT = 'Order fact table',
    customers AS MART.DIM_CUSTOMER PRIMARY KEY (customer_id) WITH SYNONYMS ('customers', 'users'),
    dates AS MART.DIM_DATE PRIMARY KEY (date_key) COMMENT = 'Calendar dimension'
)
RELATIONSHIPS (
    order_customer AS orders (customer_id) REFERENCES customers,
    order_date AS orders (order_date_key) REFERENCES dates
)
FACTS (
    PUBLIC orders.net_amount AS net_amount COMMENT = 'Net order amount after discounts',
    PRIVATE orders.cost_amount AS cost_amount COMMENT = 'Internal cost amount',
    PUBLIC orders.is_first_order LABELS = (FILTER) AS IFF(order_sequence = 1, TRUE, FALSE)
)
DIMENSIONS (
    PUBLIC orders.order_date AS order_date WITH SYNONYMS ('purchase date', '注文日'),
    PUBLIC customers.region AS region WITH SYNONYMS ('area', '地域'),
    PUBLIC customers.segment AS segment LABELS = (FILTER),
    PUBLIC dates.fiscal_month AS fiscal_month WITH SYNONYMS ('fiscal period')
)
METRICS (
    PUBLIC orders.revenue AS SUM(orders.net_amount),
    PRIVATE orders.margin AS SUM(orders.net_amount - orders.cost_amount),
    PUBLIC orders.average_order_value AS DIV0(SUM(orders.net_amount), COUNT(DISTINCT orders.order_id)),
    PUBLIC orders.running_revenue AS SUM(orders.net_amount) OVER (
        PARTITION BY customers.region
        ORDER BY dates.date_actual
        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )
)
COMMENT = 'Semantic view for customer revenue analytics'
AI_SQL_GENERATION 'Use fiscal_month for monthly questions. Use revenue for external users. Do not expose private margin unless role allows it.'
AI_QUESTION_CATEGORIZATION 'Classify questions about sales, customers, regions, and order trends.'
AI_VERIFIED_QUERIES (
    TOP_REGION_REVENUE AS (
        QUESTION 'Which regions had the highest revenue this month?'
        VERIFIED_AT 1767225600
        ONBOARDING_QUESTION TRUE
        VERIFIED_BY '( analyst = revenue-analytics@example.com )'
        SQL 'SELECT customers.region, SUM(orders.net_amount) AS revenue FROM MART.FACT_ORDER AS orders JOIN MART.DIM_CUSTOMER AS customers USING (customer_id) WHERE DATE_TRUNC(''month'', orders.order_date) = DATE_TRUNC(''month'', CURRENT_DATE()) GROUP BY customers.region ORDER BY revenue DESC'
    )
)
WITH TAG (
    GOVERNANCE.DOMAIN = 'revenue',
    GOVERNANCE.OWNER = 'analytics-platform'
)
COPY GRANTS;

-- Case 024: Tags, column comments, masking assignment, and table alterations.
CREATE TAG IF NOT EXISTS GOVERNANCE.DATA_CLASSIFICATION
    ALLOWED_VALUES 'public', 'internal', 'confidential', 'restricted'
    COMMENT = 'Allowed data classification values';

CREATE TAG IF NOT EXISTS GOVERNANCE.RETENTION_CLASS
    ALLOWED_VALUES 'short', 'standard', 'long', 'legal_hold'
    COMMENT = 'Retention policy class';

ALTER TABLE CORE.PROTECTED_CUSTOMER_PROFILE
    MODIFY COLUMN email
        SET MASKING POLICY GOVERNANCE.MASK_EMAIL_CONDITIONAL USING (email, tenant_id),
    MODIFY COLUMN email
        SET TAG GOVERNANCE.DATA_CLASSIFICATION = 'restricted',
    MODIFY COLUMN profile
        SET TAG GOVERNANCE.DATA_CLASSIFICATION = 'confidential',
    SET TAG GOVERNANCE.RETENTION_CLASS = 'standard';

ALTER TABLE CORE.PROTECTED_CUSTOMER_PROFILE
    ADD COLUMN IF NOT EXISTS consent_history ARRAY COMMENT 'Consent snapshots by channel',
    ADD COLUMN IF NOT EXISTS privacy_context OBJECT COMMENT 'Privacy audit context',
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP();

COMMENT ON COLUMN CORE.PROTECTED_CUSTOMER_PROFILE.profile IS 'Raw profile JSON. Contains locale, flags, and activity summary.';
COMMENT ON TABLE CORE.PROTECTED_CUSTOMER_PROFILE IS 'Protected customer profile with governance metadata.';

-- Case 025: Stage directory operations, LIST/REMOVE, and RESULT_SCAN inspection.
CREATE OR REPLACE STAGE RAW.QUALITY_REVIEW_STAGE
    DIRECTORY = (
        ENABLE = TRUE
        AUTO_REFRESH = FALSE
    )
    ENCRYPTION = (
        TYPE = 'SNOWFLAKE_SSE'
    )
    COMMENT = 'Internal stage for formatter quality review files';

LIST @RAW.QUALITY_REVIEW_STAGE/incoming/ PATTERN = '.*\\.(json|jsonl|csv|parquet)(\\.gz)?';

CREATE OR REPLACE TEMPORARY TABLE QA.STAGE_LISTING_SNAPSHOT AS
SELECT
    "name" AS staged_file_name,
    "size" AS size_bytes,
    "md5" AS md5_hex,
    "last_modified" AS last_modified_at,
    CURRENT_TIMESTAMP() AS captured_at
FROM TABLE(RESULT_SCAN(LAST_QUERY_ID()))
WHERE
    "size" > 0
    AND REGEXP_LIKE("name", '.*/(incoming|retry)/.*');

REMOVE @RAW.QUALITY_REVIEW_STAGE/retry/ PATTERN = '.*\\.tmp$';

SELECT
    staged_file_name,
    size_bytes,
    md5_hex,
    ROW_NUMBER() OVER (ORDER BY size_bytes DESC, staged_file_name) AS size_rank
FROM QA.STAGE_LISTING_SNAPSHOT
QUALIFY size_rank <= 100;

-- Case 026: Transaction block, query history, RESULT_SCAN, and session variables.
SET TARGET_TABLE = 'MART.TENANT_DAILY_METRIC';
SET RUN_ID = UUID_STRING();

BEGIN;

CREATE OR REPLACE TEMPORARY TABLE QA.RUN_QUERY_PROFILE AS
SELECT
    query_id,
    user_name,
    role_name,
    warehouse_name,
    start_time,
    end_time,
    execution_status,
    total_elapsed_time,
    rows_produced,
    bytes_scanned,
    query_text
FROM TABLE(INFORMATION_SCHEMA.QUERY_HISTORY(
    END_TIME_RANGE_START => DATEADD('hour', -2, CURRENT_TIMESTAMP()),
    END_TIME_RANGE_END => CURRENT_TIMESTAMP(),
    RESULT_LIMIT => 10000
))
WHERE
    query_text ILIKE '%' || $TARGET_TABLE || '%'
    AND execution_status IN ('SUCCESS', 'FAILED_WITH_ERROR');

INSERT INTO OPS.QUERY_AUDIT_SUMMARY (
    run_id,
    target_table,
    status,
    query_count,
    failed_count,
    total_elapsed_ms,
    created_at
)
SELECT
    $RUN_ID,
    $TARGET_TABLE,
    IFF(COUNT_IF(execution_status <> 'SUCCESS') > 0, 'HAS_FAILURE', 'OK'),
    COUNT(*),
    COUNT_IF(execution_status <> 'SUCCESS'),
    SUM(total_elapsed_time),
    CURRENT_TIMESTAMP()
FROM QA.RUN_QUERY_PROFILE;

SELECT * FROM IDENTIFIER($TARGET_TABLE) WHERE metric_date = CURRENT_DATE() LIMIT 10;

CREATE OR REPLACE TEMPORARY TABLE QA.TARGET_TABLE_SAMPLE AS
SELECT
    *,
    CURRENT_TIMESTAMP() AS sampled_at,
    LAST_QUERY_ID() AS source_query_id
FROM TABLE(RESULT_SCAN(LAST_QUERY_ID()));

COMMIT;

-- Case 027: Stream consumption MERGE using METADATA$ACTION and METADATA$ISUPDATE.
MERGE INTO CORE.ORDER_CURRENT AS target
USING (
    WITH changes AS (
        SELECT
            event_id,
            tenant_id,
            order_id,
            payload,
            METADATA$ACTION AS stream_action,
            METADATA$ISUPDATE AS stream_is_update,
            METADATA$ROW_ID AS stream_row_id,
            loaded_at,
            ROW_NUMBER() OVER (
                PARTITION BY tenant_id, order_id
                ORDER BY loaded_at DESC, stream_row_id DESC
            ) AS change_rank
        FROM RAW.ORDER_EVENT_STREAM
        WHERE order_id IS NOT NULL
        QUALIFY change_rank = 1
    )
    SELECT
        tenant_id,
        order_id,
        payload:order:status::STRING AS order_status,
        TRY_TO_DECIMAL(payload:order:total:amount::STRING, 18, 4) AS order_amount,
        payload:order:total:currency::STRING AS currency_code,
        payload:customer:id::STRING AS customer_id,
        stream_action,
        stream_is_update,
        stream_row_id,
        loaded_at,
        SHA2_HEX(TO_JSON(payload), 256) AS payload_hash
    FROM changes
) AS source
    ON target.tenant_id = source.tenant_id
    AND target.order_id = source.order_id
WHEN MATCHED AND source.stream_action = 'DELETE' THEN
    DELETE
WHEN MATCHED AND source.payload_hash IS DISTINCT FROM target.payload_hash THEN
    UPDATE SET
        target.order_status = source.order_status,
        target.order_amount = source.order_amount,
        target.currency_code = source.currency_code,
        target.customer_id = source.customer_id,
        target.payload_hash = source.payload_hash,
        target.last_stream_row_id = source.stream_row_id,
        target.updated_at = CURRENT_TIMESTAMP()
WHEN NOT MATCHED AND source.stream_action = 'INSERT' THEN
    INSERT (
        tenant_id,
        order_id,
        order_status,
        order_amount,
        currency_code,
        customer_id,
        payload_hash,
        last_stream_row_id,
        created_at,
        updated_at
    )
    VALUES (
        source.tenant_id,
        source.order_id,
        source.order_status,
        source.order_amount,
        source.currency_code,
        source.customer_id,
        source.payload_hash,
        source.stream_row_id,
        CURRENT_TIMESTAMP(),
        CURRENT_TIMESTAMP()
    );

-- Case 028: Set operations with nested windows, MINUS, INTERSECT, and ordered final output.
WITH candidates AS (
    SELECT customer_id, tenant_id, 'high_value' AS reason
    FROM MART.CUSTOMER_360
    WHERE lifetime_value >= 100000

    UNION ALL

    SELECT customer_id, tenant_id, 'recent_growth' AS reason
    FROM MART.CUSTOMER_360
    WHERE revenue_30d > revenue_30d_previous * 1.5

    UNION ALL

    SELECT customer_id, tenant_id, 'support_risk' AS reason
    FROM MART.CUSTOMER_360
    WHERE open_ticket_count >= 3
), eligible AS (
    SELECT customer_id, tenant_id
    FROM candidates
    INTERSECT
    SELECT customer_id, tenant_id
    FROM CORE.CUSTOMER_CONSENT
    WHERE marketing_allowed
), suppressed AS (
    SELECT customer_id, tenant_id
    FROM OPS.CAMPAIGN_SUPPRESSION
    WHERE CURRENT_DATE() BETWEEN suppressed_from AND suppressed_to
), final_candidates AS (
    SELECT customer_id, tenant_id
    FROM eligible
    MINUS
    SELECT customer_id, tenant_id
    FROM suppressed
), reason_rollup AS (
    SELECT
        c.customer_id,
        c.tenant_id,
        ARRAY_AGG(DISTINCT c.reason) WITHIN GROUP (ORDER BY c.reason) AS reasons,
        COUNT(DISTINCT c.reason) AS reason_count
    FROM candidates AS c
        INNER JOIN final_candidates AS f
            ON c.customer_id = f.customer_id
            AND c.tenant_id = f.tenant_id
    GROUP BY c.customer_id, c.tenant_id
)
SELECT
    r.tenant_id,
    r.customer_id,
    r.reasons,
    r.reason_count,
    ROW_NUMBER() OVER (
        PARTITION BY r.tenant_id
        ORDER BY r.reason_count DESC, r.customer_id
    ) AS tenant_rank
FROM reason_rollup AS r
QUALIFY tenant_rank <= 1000
ORDER BY tenant_id, tenant_rank;

-- Case 029: Nested Snowflake Scripting block with cursor, exception handlers, and dynamic table names.
EXECUTE IMMEDIATE
$$
DECLARE
    v_schema STRING DEFAULT 'MART';
    v_table STRING;
    v_sql STRING;
    v_ok_count NUMBER DEFAULT 0;
    v_error_count NUMBER DEFAULT 0;
    c_tables CURSOR FOR
        SELECT table_name
        FROM INFORMATION_SCHEMA.TABLES
        WHERE
            table_schema = ?
            AND table_type = 'BASE TABLE'
            AND table_name ILIKE 'FACT_%'
        ORDER BY table_name;
BEGIN
    FOR t IN c_tables USING (v_schema) DO
        v_table := v_schema || '.' || t.table_name;
        BEGIN
            v_sql := 'INSERT INTO OPS.TABLE_HEALTH_CHECK(table_name, row_count, null_key_count, checked_at) '
                || 'SELECT ?, COUNT(*), COUNT_IF(id IS NULL), CURRENT_TIMESTAMP() FROM ' || v_table;
            EXECUTE IMMEDIATE :v_sql USING (v_table);
            v_ok_count := v_ok_count + 1;
        EXCEPTION
            WHEN STATEMENT_ERROR CONTINUE THEN
                v_error_count := v_error_count + 1;
                INSERT INTO OPS.TABLE_HEALTH_CHECK_ERROR (
                    table_name,
                    error_code,
                    error_message,
                    error_state,
                    checked_at
                )
                SELECT
                    :v_table,
                    :SQLCODE,
                    :SQLERRM,
                    :SQLSTATE,
                    CURRENT_TIMESTAMP();
        END;
    END FOR;

    RETURN OBJECT_CONSTRUCT('ok_count', v_ok_count, 'error_count', v_error_count);
END;
$$;

-- Case 030: Mega scenario combining DDL, CTE, MERGE, task call, comments, Unicode, and deep nesting.
CREATE OR REPLACE TEMPORARY TABLE QA.MEGA_FORMATTER_INPUT (
    tenant_id STRING,
    document_id STRING,
    version NUMBER,
    payload VARIANT,
    created_at TIMESTAMP_TZ DEFAULT CURRENT_TIMESTAMP(),
    /* Block comment with SQL-looking text: SELECT * FROM fake WHERE x = 'not real'; */
    CONSTRAINT PK_MEGA_FORMATTER_INPUT PRIMARY KEY (tenant_id, document_id, version) NOT ENFORCED
)
COMMENT = 'Formatter stress table: 日本語, 한국어, العربية, emoji-like text inside strings only';

MERGE INTO QA.MEGA_FORMATTER_RESULT AS target
USING (
    WITH base AS (
        SELECT
            tenant_id,
            document_id,
            version,
            payload,
            created_at,
            payload:meta:source::STRING AS source_name,
            COALESCE(payload:meta:locale::STRING, 'und') AS locale,
            ROW_NUMBER() OVER (
                PARTITION BY tenant_id, document_id
                ORDER BY version DESC, created_at DESC
            ) AS version_rank
        FROM QA.MEGA_FORMATTER_INPUT
        WHERE payload IS NOT NULL
        QUALIFY version_rank = 1
    ), sections AS (
        SELECT
            b.*,
            s.index AS section_index,
            s.value:title::STRING AS section_title,
            s.value:body::STRING AS section_body,
            s.value:tokens AS section_tokens
        FROM base AS b,
            LATERAL FLATTEN(INPUT => b.payload:sections, OUTER => TRUE) AS s
    ), tokens AS (
        SELECT
            s.tenant_id,
            s.document_id,
            s.version,
            s.locale,
            s.source_name,
            s.section_index,
            s.section_title,
            t.index AS token_index,
            t.value:text::STRING AS token_text,
            t.value:kind::STRING AS token_kind,
            TRY_TO_DOUBLE(t.value:score::STRING) AS token_score
        FROM sections AS s,
            LATERAL FLATTEN(INPUT => s.section_tokens, OUTER => TRUE) AS t
    ), scored AS (
        SELECT
            tenant_id,
            document_id,
            version,
            locale,
            source_name,
            section_index,
            section_title,
            COUNT_IF(token_kind = 'keyword') AS keyword_count,
            COUNT_IF(token_kind = 'identifier') AS identifier_count,
            AVG(token_score) AS avg_token_score,
            ARRAY_AGG(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'text', token_text,
                    'kind', token_kind,
                    'score', token_score
                )
            ) WITHIN GROUP (ORDER BY token_index) AS tokens
        FROM tokens
        GROUP BY tenant_id, document_id, version, locale, source_name, section_index, section_title
    ), rolled AS (
        SELECT
            tenant_id,
            document_id,
            version,
            locale,
            source_name,
            ARRAY_AGG(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'index', section_index,
                    'title', section_title,
                    'keywords', keyword_count,
                    'identifiers', identifier_count,
                    'avg_score', avg_token_score,
                    'tokens', tokens
                )
            ) WITHIN GROUP (ORDER BY section_index) AS section_summary,
            SUM(keyword_count) AS keyword_count,
            SUM(identifier_count) AS identifier_count,
            AVG(avg_token_score) AS document_score
        FROM scored
        GROUP BY tenant_id, document_id, version, locale, source_name
    )
    SELECT
        tenant_id,
        document_id,
        version,
        locale,
        source_name,
        section_summary,
        keyword_count,
        identifier_count,
        document_score,
        SHA2_HEX(
            TO_JSON(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'version', version,
                    'locale', locale,
                    'source', source_name,
                    'sections', section_summary,
                    'keyword_count', keyword_count,
                    'identifier_count', identifier_count,
                    'document_score', document_score
                )
            ),
            256
        ) AS result_hash
    FROM rolled
) AS source
    ON target.tenant_id = source.tenant_id
    AND target.document_id = source.document_id
WHEN MATCHED AND target.result_hash IS DISTINCT FROM source.result_hash THEN
    UPDATE SET
        target.version = source.version,
        target.locale = source.locale,
        target.source_name = source.source_name,
        target.section_summary = source.section_summary,
        target.keyword_count = source.keyword_count,
        target.identifier_count = source.identifier_count,
        target.document_score = source.document_score,
        target.result_hash = source.result_hash,
        target.updated_at = CURRENT_TIMESTAMP()
WHEN NOT MATCHED THEN
    INSERT (
        tenant_id,
        document_id,
        version,
        locale,
        source_name,
        section_summary,
        keyword_count,
        identifier_count,
        document_score,
        result_hash,
        created_at,
        updated_at
    )
    VALUES (
        source.tenant_id,
        source.document_id,
        source.version,
        source.locale,
        source.source_name,
        source.section_summary,
        source.keyword_count,
        source.identifier_count,
        source.document_score,
        source.result_hash,
        CURRENT_TIMESTAMP(),
        CURRENT_TIMESTAMP()
    );

CALL OPS.SP_RECORD_FORMATTER_CASE_RESULT(
    'case_030',
    OBJECT_CONSTRUCT(
        'note', 'Nested formatter stress case completed / 多重ネスト完了',
        'query_id', LAST_QUERY_ID(),
        'checked_at', CURRENT_TIMESTAMP()
    )
);
