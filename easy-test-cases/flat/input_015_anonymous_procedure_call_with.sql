-- Case 015: anonymous procedure with WITH ... AS PROCEDURE and nested SQL block
WITH RUN_BACKFILL AS PROCEDURE(P_TENANT_ID STRING,P_BACKFILL_DATE DATE) RETURNS VARIANT LANGUAGE SQL AS
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
$$ CALL RUN_BACKFILL( 'TENANT-JP-001',DATE '2026-06-21');
