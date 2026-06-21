-- 06_tasks.sql
-- Tasks are created suspended by Snowflake. They are not resumed by this suite.
CREATE OR REPLACE TASK OPS.TASK_PIPELINE_ROOT
    WAREHOUSE = FORMATTER_TEST_WH
    SCHEDULE = 'USING CRON 0 * * * * UTC'
    USER_TASK_TIMEOUT_MS = 1800000
    SUSPEND_TASK_AFTER_NUM_FAILURES = 3
    TASK_AUTO_RETRY_ATTEMPTS = 1
    COMMENT = 'Root task; formatter E2E leaves it suspended'
AS
    INSERT INTO OPS.PIPELINE_RUNS (
        run_id,
        batch_id,
        pipeline_name,
        status,
        started_at,
        details
    )
    SELECT
        UUID_STRING(),
        'TASK-' || TO_VARCHAR(CURRENT_TIMESTAMP(), 'YYYYMMDDHH24MISS'),
        'SCHEDULED_ROOT',
        'SUCCEEDED',
        CURRENT_TIMESTAMP(),
        OBJECT_CONSTRUCT('message', 'root heartbeat / ルート起動');

CREATE OR REPLACE TASK OPS.TASK_APPLY_CUSTOMERS
    WAREHOUSE = FORMATTER_TEST_WH
    AFTER OPS.TASK_PIPELINE_ROOT
    WHEN SYSTEM$STREAM_HAS_DATA('RAW.CUSTOMER_EVENTS_STREAM')
AS
    CALL OPS.SP_APPLY_CUSTOMER_EVENTS('TASK-BATCH', FALSE);

CREATE OR REPLACE TASK OPS.TASK_APPLY_ORDERS
    WAREHOUSE = FORMATTER_TEST_WH
    AFTER OPS.TASK_PIPELINE_ROOT
    WHEN SYSTEM$STREAM_HAS_DATA('RAW.ORDER_EVENTS_STREAM')
AS
    CALL OPS.SP_APPLY_ORDER_EVENTS('TASK-BATCH', FALSE);

CREATE OR REPLACE TASK OPS.TASK_CAPTURE_INVENTORY
    WAREHOUSE = FORMATTER_TEST_WH
    AFTER OPS.TASK_APPLY_CUSTOMERS, OPS.TASK_APPLY_ORDERS
AS
    CALL OPS.SP_JS_CAPTURE_SCHEMA_INVENTORY('%');

CREATE OR REPLACE TASK OPS.TASK_PIPELINE_FINALIZER
    WAREHOUSE = FORMATTER_TEST_WH
    FINALIZE = OPS.TASK_PIPELINE_ROOT
AS
    INSERT INTO OPS.PIPELINE_RUNS (
        run_id,
        batch_id,
        pipeline_name,
        status,
        started_at,
        finished_at,
        details
    )
    SELECT
        UUID_STRING(),
        'TASK-FINALIZER',
        'SCHEDULED_FINALIZER',
        'SUCCEEDED',
        CURRENT_TIMESTAMP(),
        CURRENT_TIMESTAMP(),
        OBJECT_CONSTRUCT(
            'message_ja', 'タスクグラフ完了',
            'message_ko', '태스크 그래프 완료',
            'message_ar', 'اكتمل مخطط المهام'
        );
