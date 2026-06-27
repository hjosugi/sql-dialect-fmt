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
