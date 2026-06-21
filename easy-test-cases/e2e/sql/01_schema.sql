-- 01_schema.sql
CREATE OR REPLACE TABLE OPS.PIPELINE_RUNS (
    run_id VARCHAR NOT NULL,
    batch_id VARCHAR,
    pipeline_name VARCHAR NOT NULL,
    status VARCHAR NOT NULL,
    started_at TIMESTAMP_LTZ NOT NULL,
    finished_at TIMESTAMP_LTZ,
    details VARIANT,
    PRIMARY KEY (run_id) NOT ENFORCED
);

CREATE OR REPLACE TABLE OPS.ERROR_LOG (
    error_id VARCHAR DEFAULT UUID_STRING(),
    run_id VARCHAR,
    batch_id VARCHAR,
    component VARCHAR,
    error_type VARCHAR,
    error_code NUMBER,
    error_state VARCHAR,
    error_message VARCHAR,
    context VARIANT,
    created_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP()
);

CREATE OR REPLACE TABLE OPS.ASSERTION_RESULTS (
    assertion_name VARCHAR NOT NULL,
    expected_value VARIANT,
    actual_value VARIANT,
    passed BOOLEAN NOT NULL,
    details VARIANT,
    asserted_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP()
);

CREATE OR REPLACE TABLE OPS.SCHEMA_INVENTORY_SNAPSHOTS (
    snapshot_id VARCHAR DEFAULT UUID_STRING(),
    captured_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP(),
    payload VARIANT
);

CREATE OR REPLACE TABLE RAW.CUSTOMER_EVENTS (
    event_id VARCHAR NOT NULL,
    batch_id VARCHAR NOT NULL,
    event_time TIMESTAMP_TZ NOT NULL,
    event_type VARCHAR NOT NULL,
    source_system VARCHAR NOT NULL,
    payload VARIANT NOT NULL,
    ingested_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP(),
    PRIMARY KEY (event_id) NOT ENFORCED
);

CREATE OR REPLACE TABLE RAW.ORDER_EVENTS (
    event_id VARCHAR NOT NULL,
    batch_id VARCHAR NOT NULL,
    event_time TIMESTAMP_TZ NOT NULL,
    event_type VARCHAR NOT NULL,
    source_system VARCHAR NOT NULL,
    payload VARIANT NOT NULL,
    ingested_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP(),
    PRIMARY KEY (event_id) NOT ENFORCED
);

CREATE OR REPLACE TABLE RAW.CUSTOMER_CSV_LANDING (
    customer_id VARCHAR,
    display_name VARCHAR,
    locale VARCHAR,
    email VARCHAR,
    marketing_opt_in BOOLEAN,
    source_filename VARCHAR,
    source_row_number NUMBER,
    loaded_at TIMESTAMP_LTZ
);

CREATE OR REPLACE TABLE RAW.EVENT_LANDING (
    event_id VARCHAR,
    event_time TIMESTAMP_TZ,
    event_type VARCHAR,
    payload VARIANT,
    source_filename VARCHAR,
    file_content_key VARCHAR,
    loaded_at TIMESTAMP_LTZ
);

CREATE OR REPLACE TABLE CORE.CUSTOMERS (
    customer_id VARCHAR NOT NULL,
    display_name VARCHAR,
    email VARCHAR,
    locale VARCHAR,
    region VARCHAR,
    marketing_opt_in BOOLEAN,
    attributes VARIANT,
    row_hash VARCHAR,
    source_event_id VARCHAR,
    created_at TIMESTAMP_LTZ NOT NULL,
    updated_at TIMESTAMP_LTZ NOT NULL,
    PRIMARY KEY (customer_id) NOT ENFORCED
);

CREATE OR REPLACE TABLE CORE.PRODUCTS (
    product_id VARCHAR NOT NULL,
    sku VARCHAR NOT NULL,
    product_name VARIANT NOT NULL,
    category VARCHAR,
    unit_price NUMBER(18, 2),
    currency VARCHAR,
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP(),
    PRIMARY KEY (product_id) NOT ENFORCED
);

CREATE OR REPLACE TABLE CORE.ORDERS (
    order_id VARCHAR NOT NULL,
    customer_id VARCHAR NOT NULL,
    order_status VARCHAR NOT NULL,
    order_time TIMESTAMP_TZ NOT NULL,
    currency VARCHAR NOT NULL,
    gross_amount NUMBER(18, 2),
    discount_amount NUMBER(18, 2),
    net_amount NUMBER(18, 2),
    shipping_address VARIANT,
    source_event_id VARCHAR,
    created_at TIMESTAMP_LTZ NOT NULL,
    updated_at TIMESTAMP_LTZ NOT NULL,
    PRIMARY KEY (order_id) NOT ENFORCED
);

CREATE OR REPLACE TABLE CORE.ORDER_ITEMS (
    order_id VARCHAR NOT NULL,
    line_number NUMBER NOT NULL,
    product_id VARCHAR NOT NULL,
    quantity NUMBER(18, 3) NOT NULL,
    unit_price NUMBER(18, 2) NOT NULL,
    line_amount NUMBER(18, 2) NOT NULL,
    attributes VARIANT,
    created_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP(),
    PRIMARY KEY (order_id, line_number) NOT ENFORCED
);

CREATE OR REPLACE TABLE CORE.ORDER_STATUS_HISTORY (
    order_id VARCHAR NOT NULL,
    status VARCHAR NOT NULL,
    status_at TIMESTAMP_TZ NOT NULL,
    source_event_id VARCHAR NOT NULL
);

CREATE OR REPLACE TABLE CORE.MULTILINGUAL_TEXTS (
    text_id VARCHAR NOT NULL,
    language_tag VARCHAR,
    text_value VARCHAR,
    metadata VARIANT,
    created_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP(),
    PRIMARY KEY (text_id) NOT ENFORCED
);

CREATE OR REPLACE STREAM RAW.CUSTOMER_EVENTS_STREAM
    ON TABLE RAW.CUSTOMER_EVENTS
    APPEND_ONLY = FALSE
    SHOW_INITIAL_ROWS = FALSE
    COMMENT = 'Customer CDC stream / 顧客変更ストリーム';

CREATE OR REPLACE STREAM RAW.ORDER_EVENTS_STREAM
    ON TABLE RAW.ORDER_EVENTS
    APPEND_ONLY = FALSE
    SHOW_INITIAL_ROWS = FALSE
    COMMENT = 'Order CDC stream / 주문 변경 스트림';

CREATE OR REPLACE STREAM CORE.ORDERS_STREAM
    ON TABLE CORE.ORDERS
    APPEND_ONLY = FALSE
    SHOW_INITIAL_ROWS = FALSE;

CREATE OR REPLACE FILE FORMAT RAW.FF_JSON
    TYPE = JSON
    COMPRESSION = AUTO
    STRIP_OUTER_ARRAY = FALSE
    ALLOW_DUPLICATE = FALSE
    IGNORE_UTF8_ERRORS = FALSE
    SKIP_BYTE_ORDER_MARK = TRUE;

CREATE OR REPLACE FILE FORMAT RAW.FF_CSV
    TYPE = CSV
    COMPRESSION = AUTO
    FIELD_DELIMITER = ','
    RECORD_DELIMITER = '\n'
    SKIP_HEADER = 1
    FIELD_OPTIONALLY_ENCLOSED_BY = '"'
    TRIM_SPACE = TRUE
    NULL_IF = ('', 'NULL', 'null', '\\N')
    EMPTY_FIELD_AS_NULL = TRUE
    ENCODING = 'UTF8';

CREATE OR REPLACE STAGE RAW.INGEST_STAGE
    DIRECTORY = (ENABLE = TRUE)
    ENCRYPTION = (TYPE = 'SNOWFLAKE_SSE')
    COMMENT = 'Optional internal stage for COPY formatter tests';
