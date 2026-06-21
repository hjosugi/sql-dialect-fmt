-- Case 02: Snowflake Scripting procedure with variables, loops, dynamic SQL,
-- transactions, RESULTSETs, MERGE, custom exceptions, and multilingual text.
create or replace procedure OPS.SP_RECONCILE_BATCH(P_BATCH_ID VARCHAR, P_MIN_SCORE NUMBER(9, 4) Default 0.7500, P_DRY_RUN BOOLEAN default false) Returns Variant Language SQL Strict Execute As Owner As $$
DECLARE
    v_started_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP();
    v_finished_at TIMESTAMP_LTZ;
    v_rows_merged NUMBER DEFAULT 0;
    v_regions_seen NUMBER DEFAULT 0;
    v_warning_count NUMBER DEFAULT 0;
    v_dynamic_sql VARCHAR;
    v_region_rows RESULTSET;
    e_missing_batch EXCEPTION (-20001, 'Batch does not exist / バッチが存在しません。');
    e_invalid_score EXCEPTION (-20002, 'Minimum score must be between 0 and 1.');
BEGIN
    IF (P_MIN_SCORE < 0 OR P_MIN_SCORE > 1) THEN
        RAISE e_invalid_score;
    END IF;

    IF (NOT EXISTS (
        SELECT 1
        FROM RAW.BATCH_CONTROL
        WHERE batch_id = :P_BATCH_ID
    )) THEN
        RAISE e_missing_batch;
    END IF;

    INSERT INTO OPS.PIPELINE_LOG (
        batch_id,
        step_name,
        status,
        message,
        logged_at
    )
    VALUES (
        :P_BATCH_ID,
        'RECONCILE',
        'STARTED',
        '照合処理を開始 / démarrage / بدء المعالجة',
        :v_started_at
    );

    v_region_rows := (
        SELECT
            COALESCE(payload:region::STRING, 'UNKNOWN') AS region,
            COUNT(*) AS event_count
        FROM RAW.CUSTOMER_EVENTS
        WHERE batch_id = :P_BATCH_ID
        GROUP BY COALESCE(payload:region::STRING, 'UNKNOWN')
        ORDER BY event_count DESC, region
    );

    FOR region_record IN v_region_rows DO
        v_regions_seen := v_regions_seen + 1;

        BEGIN
            INSERT INTO OPS.REGION_AUDIT (
                batch_id,
                region,
                event_count,
                note
            )
            VALUES (
                :P_BATCH_ID,
                :region_record.region,
                :region_record.event_count,
                CASE
                    WHEN region_record.region = 'JP' THEN '日本リージョン'
                    WHEN region_record.region = 'KR' THEN '대한민국 리전'
                    WHEN region_record.region = 'SA' THEN 'المنطقة العربية'
                    ELSE 'Other / その他'
                END
            );
        EXCEPTION
            WHEN STATEMENT_ERROR CONTINUE THEN
                v_warning_count := v_warning_count + 1;
                INSERT INTO OPS.ERROR_LOG (
                    batch_id,
                    error_type,
                    error_code,
                    error_message,
                    error_state,
                    context,
                    created_at
                )
                SELECT
                    :P_BATCH_ID,
                    'REGION_AUDIT_WARNING',
                    :SQLCODE,
                    :SQLERRM,
                    :SQLSTATE,
                    OBJECT_CONSTRUCT('region', :region_record.region),
                    CURRENT_TIMESTAMP();
        END;
    END FOR;

    BEGIN TRANSACTION;

    MERGE INTO CORE.CUSTOMER_SCORES AS target
    USING (
        SELECT
            payload:customer_id::STRING AS customer_id,
            payload:model_version::STRING AS model_version,
            TRY_TO_DECIMAL(payload:score::STRING, 9, 4) AS score,
            event_id,
            event_time,
            ROW_NUMBER() OVER (
                PARTITION BY payload:customer_id::STRING
                ORDER BY event_time DESC, event_id DESC
            ) AS row_num
        FROM RAW.CUSTOMER_EVENTS
        WHERE
            batch_id = :P_BATCH_ID
            AND payload:event_type::STRING = 'SCORE_UPDATED'
        QUALIFY row_num = 1
    ) AS source
        ON target.customer_id = source.customer_id
    WHEN MATCHED AND source.score IS NULL THEN
        DELETE
    WHEN MATCHED AND source.score >= :P_MIN_SCORE THEN
        UPDATE SET
            target.model_version = source.model_version,
            target.score = source.score,
            target.source_event_id = source.event_id,
            target.updated_at = CURRENT_TIMESTAMP()
    WHEN NOT MATCHED AND source.score >= :P_MIN_SCORE THEN
        INSERT (
            customer_id,
            model_version,
            score,
            source_event_id,
            created_at,
            updated_at
        )
        VALUES (
            source.customer_id,
            source.model_version,
            source.score,
            source.event_id,
            CURRENT_TIMESTAMP(),
            CURRENT_TIMESTAMP()
        );

    v_rows_merged := SQLROWCOUNT;

    v_dynamic_sql :=
        'INSERT INTO OPS.DYNAMIC_AUDIT (batch_id, object_name, matched_rows, created_at) '
        || 'SELECT ?, table_name, COUNT_IF(row_count >= ?), CURRENT_TIMESTAMP() '
        || 'FROM INFORMATION_SCHEMA.TABLES '
        || 'WHERE table_schema IN (''RAW'', ''CORE'', ''MART'') '
        || 'GROUP BY table_name';

    EXECUTE IMMEDIATE v_dynamic_sql USING (P_BATCH_ID, P_MIN_SCORE);

    IF (P_DRY_RUN) THEN
        ROLLBACK;
    ELSE
        COMMIT;
    END IF;

    v_finished_at := CURRENT_TIMESTAMP();

    UPDATE RAW.BATCH_CONTROL
    SET
        status = IFF(:P_DRY_RUN, 'DRY_RUN', 'COMPLETED'),
        processed_at = :v_finished_at,
        details = OBJECT_CONSTRUCT_KEEP_NULL(
            'rows_merged', :v_rows_merged,
            'regions_seen', :v_regions_seen,
            'warnings', :v_warning_count,
            'message_ja', '処理完了',
            'message_ar', 'اكتملت المعالجة'
        )
    WHERE batch_id = :P_BATCH_ID;

    RETURN OBJECT_CONSTRUCT_KEEP_NULL(
        'status', IFF(P_DRY_RUN, 'DRY_RUN', 'COMPLETED'),
        'batch_id', P_BATCH_ID,
        'minimum_score', P_MIN_SCORE,
        'rows_merged', v_rows_merged,
        'regions_seen', v_regions_seen,
        'warning_count', v_warning_count,
        'started_at', v_started_at,
        'finished_at', v_finished_at,
        'elapsed_ms', DATEDIFF('millisecond', v_started_at, v_finished_at)
    );
EXCEPTION
    WHEN e_missing_batch THEN
        ROLLBACK;
        RETURN OBJECT_CONSTRUCT(
            'status', 'REJECTED',
            'batch_id', P_BATCH_ID,
            'sqlcode', SQLCODE,
            'sqlstate', SQLSTATE,
            'message', SQLERRM
        );
    WHEN e_invalid_score THEN
        ROLLBACK;
        RAISE;
    WHEN OTHER THEN
        ROLLBACK;
        INSERT INTO OPS.ERROR_LOG (
            batch_id,
            error_type,
            error_code,
            error_message,
            error_state,
            context,
            created_at
        )
        SELECT
            :P_BATCH_ID,
            'UNHANDLED',
            :SQLCODE,
            :SQLERRM,
            :SQLSTATE,
            OBJECT_CONSTRUCT('procedure', 'OPS.SP_RECONCILE_BATCH'),
            CURRENT_TIMESTAMP();
        RAISE;
END;
$$;
