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
