-- Case 05: DDL, COPY, MERGE, streams, and a task graph.
CREATE OR REPLACE FILE FORMAT RAW.FF_EVENTS_JSON
    TYPE = JSON
    COMPRESSION = AUTO
    STRIP_OUTER_ARRAY = FALSE
    ALLOW_DUPLICATE = FALSE
    IGNORE_UTF8_ERRORS = FALSE
    REPLACE_INVALID_CHARACTERS = FALSE
    SKIP_BYTE_ORDER_MARK = TRUE
    COMMENT = 'JSON Lines / 日本語・한국어・العربية';

CREATE OR REPLACE FILE FORMAT RAW.FF_CUSTOMERS_CSV
    TYPE = CSV
    COMPRESSION = AUTO
    FIELD_DELIMITER = ','
    RECORD_DELIMITER = '\n'
    SKIP_HEADER = 1
    FIELD_OPTIONALLY_ENCLOSED_BY = '"'
    ESCAPE_UNENCLOSED_FIELD = NONE
    TRIM_SPACE = TRUE
    NULL_IF = ('', 'NULL', 'null', '\\N')
    EMPTY_FIELD_AS_NULL = TRUE
    ENCODING = 'UTF8';

CREATE OR REPLACE STAGE RAW.INGEST_STAGE
    DIRECTORY = (
        ENABLE = TRUE,
        AUTO_REFRESH = FALSE
    )
    ENCRYPTION = (TYPE = 'SNOWFLAKE_SSE')
    COMMENT = 'Formatter E2E internal stage';

COPY INTO RAW.CUSTOMER_CSV_LANDING (
    customer_id,
    display_name,
    locale,
    email,
    marketing_opt_in,
    source_filename,
    source_row_number,
    loaded_at
)
FROM (
    SELECT
        $1::STRING,
        $2::STRING,
        $3::STRING,
        NULLIF(LOWER(TRIM($4::STRING)), ''),
        TRY_TO_BOOLEAN($5::STRING),
        METADATA$FILENAME,
        METADATA$FILE_ROW_NUMBER,
        METADATA$START_SCAN_TIME
    FROM @RAW.INGEST_STAGE/customers/
        (FILE_FORMAT => RAW.FF_CUSTOMERS_CSV)
)
PATTERN = '.*customers_[0-9]{8}\\.csv'
ON_ERROR = 'CONTINUE'
PURGE = FALSE
FORCE = FALSE
RETURN_FAILED_ONLY = FALSE;

COPY INTO RAW.EVENT_LANDING (
    event_id,
    event_time,
    event_type,
    payload,
    source_filename,
    file_content_key,
    loaded_at
)
FROM (
    SELECT
        $1:event_id::STRING,
        TRY_TO_TIMESTAMP_TZ($1:event_time::STRING),
        $1:event_type::STRING,
        $1,
        METADATA$FILENAME,
        METADATA$FILE_CONTENT_KEY,
        METADATA$START_SCAN_TIME
    FROM @RAW.INGEST_STAGE/events/
        (FILE_FORMAT => RAW.FF_EVENTS_JSON)
)
FILES = ('events_ja.jsonl', 'events_global.jsonl')
ON_ERROR = 'SKIP_FILE_10%'
SIZE_LIMIT = 5368709120
PURGE = FALSE;

MERGE INTO CORE.CUSTOMERS AS target
USING (
    SELECT
        customer_id,
        display_name,
        locale,
        email,
        marketing_opt_in,
        source_filename,
        loaded_at,
        SHA2_HEX(
            CONCAT_WS(
                '|',
                customer_id,
                COALESCE(display_name, '∅'),
                COALESCE(locale, 'und'),
                COALESCE(email, '∅'),
                COALESCE(marketing_opt_in::STRING, '∅')
            ),
            256
        ) AS row_hash
    FROM RAW.CUSTOMER_CSV_LANDING
    QUALIFY ROW_NUMBER() OVER (
        PARTITION BY customer_id
        ORDER BY loaded_at DESC, source_filename DESC
    ) = 1
) AS source
    ON target.customer_id = source.customer_id
WHEN MATCHED AND target.row_hash <> source.row_hash THEN
    UPDATE SET
        target.display_name = source.display_name,
        target.locale = source.locale,
        target.email = source.email,
        target.marketing_opt_in = source.marketing_opt_in,
        target.row_hash = source.row_hash,
        target.source_filename = source.source_filename,
        target.updated_at = CURRENT_TIMESTAMP()
WHEN NOT MATCHED THEN
    INSERT (
        customer_id,
        display_name,
        locale,
        email,
        marketing_opt_in,
        row_hash,
        source_filename,
        created_at,
        updated_at
    )
    VALUES (
        source.customer_id,
        source.display_name,
        source.locale,
        source.email,
        source.marketing_opt_in,
        source.row_hash,
        source.source_filename,
        CURRENT_TIMESTAMP(),
        CURRENT_TIMESTAMP()
    );

CREATE OR REPLACE STREAM RAW.EVENT_LANDING_STREAM
    ON TABLE RAW.EVENT_LANDING
    APPEND_ONLY = FALSE
    SHOW_INITIAL_ROWS = TRUE
    COMMENT = 'CDC stream / 변경 스트림 / تدفق التغيير';

CREATE OR REPLACE TASK OPS.TASK_PIPELINE_ROOT
    WAREHOUSE = FORMATTER_TEST_WH
    SCHEDULE = 'USING CRON 0 * * * * UTC'
    USER_TASK_TIMEOUT_MS = 1800000
    SUSPEND_TASK_AFTER_NUM_FAILURES = 3
    TASK_AUTO_RETRY_ATTEMPTS = 1
    COMMENT = 'Root task; intentionally left suspended after creation'
AS
    INSERT INTO OPS.PIPELINE_LOG (
        batch_id,
        step_name,
        status,
        message,
        logged_at
    )
    SELECT
        UUID_STRING(),
        'TASK_ROOT',
        'STARTED',
        'Scheduled graph started',
        CURRENT_TIMESTAMP();

CREATE OR REPLACE TASK OPS.TASK_APPLY_EVENTS
    WAREHOUSE = FORMATTER_TEST_WH
    AFTER OPS.TASK_PIPELINE_ROOT
    WHEN SYSTEM$STREAM_HAS_DATA('RAW.EVENT_LANDING_STREAM')
AS
    CALL OPS.SP_APPLY_EVENT_STREAM();

CREATE OR REPLACE TASK OPS.TASK_REFRESH_MARTS
    WAREHOUSE = FORMATTER_TEST_WH
    AFTER OPS.TASK_APPLY_EVENTS
AS
    CALL OPS.SP_REFRESH_MARTS();

CREATE OR REPLACE TASK OPS.TASK_PIPELINE_FINALIZER
    WAREHOUSE = FORMATTER_TEST_WH
    FINALIZE = OPS.TASK_PIPELINE_ROOT
AS
    INSERT INTO OPS.PIPELINE_LOG (
        batch_id,
        step_name,
        status,
        message,
        logged_at
    )
    SELECT
        UUID_STRING(),
        'TASK_FINALIZER',
        'COMPLETED',
        'Task graph finished / タスクグラフ完了',
        CURRENT_TIMESTAMP();
