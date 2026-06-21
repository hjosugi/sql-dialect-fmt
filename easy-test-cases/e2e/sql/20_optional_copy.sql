-- 20_optional_copy.sql
-- This file is not required by the deterministic INSERT-based E2E path.
-- Upload e2e/data/customers.csv and e2e/data/events.jsonl to RAW.INGEST_STAGE,
-- then adjust FILES/PATTERN as needed.
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
        LOWER(TRIM($4::STRING)),
        TRY_TO_BOOLEAN($5::STRING),
        METADATA$FILENAME,
        METADATA$FILE_ROW_NUMBER,
        METADATA$START_SCAN_TIME
    FROM @RAW.INGEST_STAGE/customers/
        (FILE_FORMAT => RAW.FF_CSV)
)
PATTERN = '.*customers\\.csv'
ON_ERROR = 'CONTINUE'
PURGE = FALSE
FORCE = TRUE;

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
        (FILE_FORMAT => RAW.FF_JSON)
)
FILES = ('events.jsonl')
ON_ERROR = 'CONTINUE'
PURGE = FALSE
FORCE = TRUE;
