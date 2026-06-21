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
