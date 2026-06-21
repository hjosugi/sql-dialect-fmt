-- 00_bootstrap.sql
-- Requires a role that can create a database and an X-SMALL warehouse.
create or replace warehouse FORMATTER_TEST_WH WAREHOUSE_SIZE = 'XSMALL' AUTO_SUSPEND = 60 AUTO_RESUME = True INITIALLY_SUSPENDED = True Comment = 'Temporary warehouse for Snowflake formatter E2E tests';
create or replace database SNOWFLAKE_FORMATTER_LAB comment = 'Disposable multilingual formatter and procedure scenario';
create or replace schema SNOWFLAKE_FORMATTER_LAB.RAW;
Create Or Replace Schema SNOWFLAKE_FORMATTER_LAB.CORE;
create or replace schema SNOWFLAKE_FORMATTER_LAB.MART;
Create Or Replace Schema SNOWFLAKE_FORMATTER_LAB.OPS;
create or replace schema SNOWFLAKE_FORMATTER_LAB.UTIL;
Use Warehouse FORMATTER_TEST_WH;
use database SNOWFLAKE_FORMATTER_LAB;
Use Schema OPS;
alter SESSION set TIMEZONE = 'Asia/Tokyo', WEEK_START = 1, TIMESTAMP_OUTPUT_FORMAT = 'YYYY-MM-DD HH24:MI:SS.FF3 TZH:TZM', QUERY_TAG = 'snowflake-formatter-torture-suite/e2e';
-- 01_schema.sql
Create Or Replace Table OPS.PIPELINE_RUNS (run_id VARCHAR not null, batch_id VARCHAR, pipeline_name VARCHAR not null, status VARCHAR Not Null, started_at TIMESTAMP_LTZ not null, finished_at TIMESTAMP_LTZ, details variant, PRIMARY KEY (run_id) Not ENFORCED);
Create Or Replace Table OPS.ERROR_LOG (error_id VARCHAR default UUID_STRING(), run_id VARCHAR, batch_id VARCHAR, component VARCHAR, error_type VARCHAR, error_code NUMBER, error_state VARCHAR, error_message VARCHAR, context variant, created_at TIMESTAMP_LTZ Default CURRENT_TIMESTAMP());
Create Or Replace Table OPS.ASSERTION_RESULTS (assertion_name VARCHAR not null, expected_value Variant, actual_value variant, passed BOOLEAN Not Null, details variant, asserted_at TIMESTAMP_LTZ Default CURRENT_TIMESTAMP());
Create Or Replace Table OPS.SCHEMA_INVENTORY_SNAPSHOTS (snapshot_id VARCHAR default UUID_STRING(), captured_at TIMESTAMP_LTZ Default CURRENT_TIMESTAMP(), payload variant);
create or replace table RAW.CUSTOMER_EVENTS (event_id VARCHAR Not Null, batch_id VARCHAR not null, event_time TIMESTAMP_TZ Not Null, event_type VARCHAR not null, source_system VARCHAR Not Null, payload variant not null, ingested_at TIMESTAMP_LTZ Default CURRENT_TIMESTAMP(), PRIMARY KEY (event_id) not ENFORCED);
create or replace table RAW.ORDER_EVENTS (event_id VARCHAR Not Null, batch_id VARCHAR not null, event_time TIMESTAMP_TZ Not Null, event_type VARCHAR not null, source_system VARCHAR Not Null, payload variant not null, ingested_at TIMESTAMP_LTZ Default CURRENT_TIMESTAMP(), PRIMARY KEY (event_id) not ENFORCED);
create or replace table RAW.CUSTOMER_CSV_LANDING (customer_id VARCHAR, display_name VARCHAR, locale VARCHAR, email VARCHAR, marketing_opt_in BOOLEAN, source_filename VARCHAR, source_row_number NUMBER, loaded_at TIMESTAMP_LTZ);
create or replace table RAW.EVENT_LANDING (event_id VARCHAR, event_time TIMESTAMP_TZ, event_type VARCHAR, payload variant, source_filename VARCHAR, file_content_key VARCHAR, loaded_at TIMESTAMP_LTZ);
Create Or Replace Table CORE.CUSTOMERS (customer_id VARCHAR not null, display_name VARCHAR, email VARCHAR, locale VARCHAR, region VARCHAR, marketing_opt_in BOOLEAN, attributes variant, row_hash VARCHAR, source_event_id VARCHAR, created_at TIMESTAMP_LTZ Not Null, updated_at TIMESTAMP_LTZ not null, PRIMARY KEY (customer_id) Not ENFORCED);
Create Or Replace Table CORE.PRODUCTS (product_id VARCHAR not null, sku VARCHAR Not Null, product_name variant not null, category VARCHAR, unit_price NUMBER(18, 2), currency VARCHAR, active BOOLEAN default true, created_at TIMESTAMP_LTZ Default CURRENT_TIMESTAMP(), PRIMARY KEY (product_id) not ENFORCED);
create or replace table CORE.ORDERS (order_id VARCHAR Not Null, customer_id VARCHAR not null, order_status VARCHAR Not Null, order_time TIMESTAMP_TZ not null, currency VARCHAR Not Null, gross_amount NUMBER(18, 2), discount_amount NUMBER(18, 2), net_amount NUMBER(18, 2), shipping_address Variant, source_event_id VARCHAR, created_at TIMESTAMP_LTZ Not Null, updated_at TIMESTAMP_LTZ not null, PRIMARY KEY (order_id) Not ENFORCED);
Create Or Replace Table CORE.ORDER_ITEMS (order_id VARCHAR not null, line_number NUMBER Not Null, product_id VARCHAR not null, quantity NUMBER(18, 3) Not Null, unit_price NUMBER(18, 2) not null, line_amount NUMBER(18, 2) Not Null, attributes variant, created_at TIMESTAMP_LTZ Default CURRENT_TIMESTAMP(), PRIMARY KEY (order_id, line_number) Not ENFORCED);
Create Or Replace Table CORE.ORDER_STATUS_HISTORY (order_id VARCHAR not null, status VARCHAR Not Null, status_at TIMESTAMP_TZ not null, source_event_id VARCHAR Not Null);
Create Or Replace Table CORE.MULTILINGUAL_TEXTS (text_id VARCHAR not null, language_tag VARCHAR, text_value VARCHAR, metadata Variant, created_at TIMESTAMP_LTZ default CURRENT_TIMESTAMP(), PRIMARY KEY (text_id) Not ENFORCED);
Create Or Replace Stream RAW.CUSTOMER_EVENTS_STREAM On Table RAW.CUSTOMER_EVENTS APPEND_ONLY = False SHOW_INITIAL_ROWS = False Comment = 'Customer CDC stream / 顧客変更ストリーム';
create or replace stream RAW.ORDER_EVENTS_STREAM on table RAW.ORDER_EVENTS APPEND_ONLY = false SHOW_INITIAL_ROWS = false comment = 'Order CDC stream / 주문 변경 스트림';
create or replace stream CORE.ORDERS_STREAM on table CORE.ORDERS APPEND_ONLY = false SHOW_INITIAL_ROWS = false;
Create Or Replace FILE FORMAT RAW.FF_JSON Type = JSON COMPRESSION = AUTO STRIP_OUTER_ARRAY = False ALLOW_DUPLICATE = False IGNORE_UTF8_ERRORS = False SKIP_BYTE_ORDER_MARK = True;
create or replace FILE FORMAT RAW.FF_CSV type = CSV COMPRESSION = AUTO FIELD_DELIMITER = ',' RECORD_DELIMITER = '\n' SKIP_HEADER = 1 FIELD_OPTIONALLY_ENCLOSED_BY = '"' TRIM_SPACE = True NULL_IF = ('', 'NULL', 'null', '\\N') EMPTY_FIELD_AS_NULL = True ENCODING = 'UTF8';
Create Or Replace Stage RAW.INGEST_STAGE Directory = (ENABLE = true) ENCRYPTION = (type = 'SNOWFLAKE_SSE') Comment = 'Optional internal stage for COPY formatter tests';
-- 02_seed.sql
Insert Into CORE.PRODUCTS (product_id, sku, product_name, category, unit_price, currency) select column1, column2, PARSE_JSON(column3), column4, column5, column6 from values ('P001', 'BOOK-JP-001', '{"ja":"分散システム入門","en":"Introduction to Distributed Systems"}', 'BOOKS', 3200.00, 'JPY'), ('P002', 'TEA-KYOTO-02', '{"ja":"宇治抹茶","en":"Uji Matcha","ko":"우지 말차"}', 'FOOD', 1800.00, 'JPY'), ('P003', 'CAFÉ-FR-03', '{"fr":"Café de spécialité","en":"Specialty Coffee"}', 'FOOD', 18.50, 'EUR'), ('P004', 'DATA-KR-04', '{"ko":"데이터 파이프라인 노트","en":"Data Pipeline Notebook"}', 'STATIONERY', 22000.00, 'KRW'), ('P005', 'MAP-AR-05', '{"ar":"خريطة السفر الذكية","en":"Smart Travel Map"}', 'TRAVEL', 75.00, 'SAR');
insert into CORE.MULTILINGUAL_TEXTS (text_id, language_tag, text_value, metadata) Select column1, column2, column3, PARSE_JSON(column4) From Values ('T001', 'ja-JP', '東京都で雪と桜を見る。', '{"script":"Jpan","emoji":"🌸"}'), ('T002', 'ko-KR', '서울에서 데이터 파이프라인을 운영합니다.', '{"script":"Kore"}'), ('T003', 'zh-CN', '你好，世界；数据工程。', '{"script":"Hans"}'), ('T004', 'ar-SA', 'مرحبًا بالعالم — هندسة البيانات', '{"script":"Arab","direction":"rtl"}'), ('T005', 'he-IL', 'שלום עולם — הנדסת נתונים', '{"script":"Hebr","direction":"rtl"}'), ('T006', 'hi-IN', 'नमस्ते दुनिया — डेटा इंजीनियरिंग', '{"script":"Deva"}'), ('T007', 'th-TH', 'สวัสดีชาวโลก — วิศวกรรมข้อมูล', '{"script":"Thai"}'), ('T008', 'fr-FR', 'L''été, le café et la crème brûlée.', '{"script":"Latn","accented":true}'), ('T009', 'und', 'ＡＢＣ１２３ / Café / 👩🏽‍💻🚀', '{"normalization":"mixed","contains_nfd":true}');
insert into RAW.CUSTOMER_EVENTS (event_id, batch_id, event_time, event_type, source_system, payload) Select column1, column2, column3::TIMESTAMP_TZ, column4, column5, PARSE_JSON(column6) from values ('CE001', 'BATCH-001', '2026-06-01 09:00:00 +09:00', 'UPSERT', 'web-ja', '{"customer_id":"C001","display_name":"杉野尾 広貴","email":"HIROKI@example.jp ","locale":"ja-JP","region":"JP","marketing_opt_in":true,"attributes":{"interests":["cloud","分散システム"],"tier":"gold"}}'), ('CE002', 'BATCH-001', '2026-06-01 10:00:00 +02:00', 'UPSERT', 'web-fr', '{"customer_id":"C002","display_name":"Élodie d''Arcy","email":"elodie@example.fr","locale":"fr-FR","region":"FR","marketing_opt_in":false,"attributes":{"interests":["café","data"],"tier":"silver"}}'), ('CE003', 'BATCH-001', '2026-06-01 17:00:00 +09:00', 'UPSERT', 'mobile-ko', '{"customer_id":"C003","display_name":"김민수","email":"minsu@example.kr","locale":"ko-KR","region":"KR","marketing_opt_in":true,"attributes":{"interests":["데이터","AI"],"tier":"gold"}}'), ('CE004', 'BATCH-001', '2026-06-01 12:00:00 +03:00', 'UPSERT', 'mobile-ar', '{"customer_id":"C004","display_name":"ليان أحمد","email":"layan@example.sa","locale":"ar-SA","region":"SA","marketing_opt_in":true,"attributes":{"interests":["السفر","الذكاء الاصطناعي"],"tier":"bronze"}}'), ('CE005', 'BATCH-001', '2026-06-01 14:00:00 +05:30', 'UPSERT', 'web-hi', '{"customer_id":"C005","display_name":"आरव शर्मा","email":"aarav@example.in","locale":"hi-IN","region":"IN","marketing_opt_in":false,"attributes":{"interests":["क्लाउड","डेटा"],"tier":"silver"}}'), ('CE006', 'BATCH-001', '2026-06-01 13:00:00 +03:00', 'UPSERT', 'web-he', '{"customer_id":"C006","display_name":"נועה לוי","email":"noa@example.il","locale":"he-IL","region":"IL","marketing_opt_in":true,"attributes":{"interests":["ענן","נתונים"],"tier":"gold"}}');
insert into RAW.ORDER_EVENTS (event_id, batch_id, event_time, event_type, source_system, payload) Select column1, column2, column3::TIMESTAMP_TZ, column4, column5, PARSE_JSON(column6) from values ('OE001', 'BATCH-001', '2026-06-02 09:05:00 +09:00', 'UPSERT', 'checkout-ja', '{"order_id":"O1001","customer_id":"C001","status":"PAID","order_time":"2026-06-02T09:00:00+09:00","currency":"JPY","discount_amount":500,"shipping_address":{"country":"JP","city":"西東京市","line1":"下保谷3-17-14"},"items":[{"line":1,"product_id":"P001","quantity":1,"unit_price":3200},{"line":2,"product_id":"P002","quantity":2,"unit_price":1800}]}'), ('OE002', 'BATCH-001', '2026-06-02 10:10:00 +02:00', 'UPSERT', 'checkout-fr', '{"order_id":"O1002","customer_id":"C002","status":"PAID","order_time":"2026-06-02T10:00:00+02:00","currency":"EUR","discount_amount":0,"shipping_address":{"country":"FR","city":"Lyon"},"items":[{"line":1,"product_id":"P003","quantity":3,"unit_price":18.5}]}'), ('OE003', 'BATCH-001', '2026-06-02 18:30:00 +09:00', 'UPSERT', 'checkout-ko', '{"order_id":"O1003","customer_id":"C003","status":"SHIPPED","order_time":"2026-06-02T18:00:00+09:00","currency":"KRW","discount_amount":2000,"shipping_address":{"country":"KR","city":"서울"},"items":[{"line":1,"product_id":"P004","quantity":2,"unit_price":22000},{"line":2,"product_id":"P002","quantity":1,"unit_price":1800}]}'), ('OE004', 'BATCH-001', '2026-06-02 13:00:00 +03:00', 'UPSERT', 'checkout-ar', '{"order_id":"O1004","customer_id":"C004","status":"CREATED","order_time":"2026-06-02T12:55:00+03:00","currency":"SAR","discount_amount":5,"shipping_address":{"country":"SA","city":"الرياض"},"items":[{"line":1,"product_id":"P005","quantity":1,"unit_price":75}]}'), ('OE005', 'BATCH-001', '2026-06-02 15:00:00 +05:30', 'UPSERT', 'checkout-hi', '{"order_id":"O1005","customer_id":"C005","status":"DELIVERED","order_time":"2026-06-02T14:45:00+05:30","currency":"JPY","discount_amount":200,"shipping_address":{"country":"IN","city":"दिल्ली"},"items":[{"line":1,"product_id":"P001","quantity":1,"unit_price":3200},{"line":2,"product_id":"P004","quantity":1,"unit_price":22000}]}');
-- 03_procedures.sql
Create Or Replace Procedure OPS.SP_APPLY_CUSTOMER_EVENTS(P_BATCH_ID VARCHAR, P_DRY_RUN BOOLEAN default false) Returns Variant Language SQL Strict Execute As Owner As $$
DECLARE
    v_run_id VARCHAR DEFAULT UUID_STRING();
    v_started_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP();
    v_finished_at TIMESTAMP_LTZ;
    v_rows_affected NUMBER DEFAULT 0;
BEGIN
    INSERT INTO OPS.PIPELINE_RUNS (
        run_id,
        batch_id,
        pipeline_name,
        status,
        started_at,
        details
    )
    SELECT
        :v_run_id,
        :P_BATCH_ID,
        'APPLY_CUSTOMER_EVENTS',
        'RUNNING',
        :v_started_at,
        OBJECT_CONSTRUCT('dry_run', :P_DRY_RUN);

    BEGIN TRANSACTION;

    MERGE INTO CORE.CUSTOMERS AS target
    USING (
        SELECT
            payload:customer_id::STRING AS customer_id,
            payload:display_name::STRING AS display_name,
            LOWER(TRIM(payload:email::STRING)) AS email,
            payload:locale::STRING AS locale,
            payload:region::STRING AS region,
            COALESCE(payload:marketing_opt_in::BOOLEAN, FALSE) AS marketing_opt_in,
            payload:attributes AS attributes,
            event_id,
            event_type,
            event_time,
            SHA2_HEX(
                TO_JSON(
                    OBJECT_CONSTRUCT_KEEP_NULL(
                        'display_name', payload:display_name::STRING,
                        'email', LOWER(TRIM(payload:email::STRING)),
                        'locale', payload:locale::STRING,
                        'region', payload:region::STRING,
                        'marketing_opt_in', COALESCE(payload:marketing_opt_in::BOOLEAN, FALSE),
                        'attributes', payload:attributes
                    )
                ),
                256
            ) AS row_hash
        FROM RAW.CUSTOMER_EVENTS_STREAM
        WHERE batch_id = :P_BATCH_ID
        QUALIFY ROW_NUMBER() OVER (
            PARTITION BY payload:customer_id::STRING
            ORDER BY event_time DESC, event_id DESC
        ) = 1
    ) AS source
        ON target.customer_id = source.customer_id
    WHEN MATCHED AND source.event_type = 'DELETE' THEN
        DELETE
    WHEN MATCHED AND source.event_type <> 'DELETE' AND target.row_hash <> source.row_hash THEN
        UPDATE SET
            target.display_name = source.display_name,
            target.email = source.email,
            target.locale = source.locale,
            target.region = source.region,
            target.marketing_opt_in = source.marketing_opt_in,
            target.attributes = source.attributes,
            target.row_hash = source.row_hash,
            target.source_event_id = source.event_id,
            target.updated_at = CURRENT_TIMESTAMP()
    WHEN NOT MATCHED AND source.event_type <> 'DELETE' THEN
        INSERT (
            customer_id,
            display_name,
            email,
            locale,
            region,
            marketing_opt_in,
            attributes,
            row_hash,
            source_event_id,
            created_at,
            updated_at
        )
        VALUES (
            source.customer_id,
            source.display_name,
            source.email,
            source.locale,
            source.region,
            source.marketing_opt_in,
            source.attributes,
            source.row_hash,
            source.event_id,
            CURRENT_TIMESTAMP(),
            CURRENT_TIMESTAMP()
        );

    v_rows_affected := SQLROWCOUNT;

    IF (P_DRY_RUN) THEN
        ROLLBACK;
    ELSE
        COMMIT;
    END IF;

    v_finished_at := CURRENT_TIMESTAMP();

    UPDATE OPS.PIPELINE_RUNS
    SET
        status = IFF(:P_DRY_RUN, 'DRY_RUN', 'SUCCEEDED'),
        finished_at = :v_finished_at,
        details = OBJECT_CONSTRUCT(
            'rows_affected', :v_rows_affected,
            'dry_run', :P_DRY_RUN,
            'message_ja', '顧客イベント処理完了',
            'message_ar', 'اكتملت معالجة أحداث العملاء'
        )
    WHERE run_id = :v_run_id;

    RETURN OBJECT_CONSTRUCT(
        'status', IFF(P_DRY_RUN, 'DRY_RUN', 'SUCCEEDED'),
        'run_id', v_run_id,
        'batch_id', P_BATCH_ID,
        'rows_affected', v_rows_affected,
        'elapsed_ms', DATEDIFF('millisecond', v_started_at, v_finished_at)
    );
EXCEPTION
    WHEN OTHER THEN
        ROLLBACK;
        INSERT INTO OPS.ERROR_LOG (
            run_id,
            batch_id,
            component,
            error_type,
            error_code,
            error_state,
            error_message,
            context
        )
        SELECT
            :v_run_id,
            :P_BATCH_ID,
            'OPS.SP_APPLY_CUSTOMER_EVENTS',
            'UNHANDLED',
            :SQLCODE,
            :SQLSTATE,
            :SQLERRM,
            OBJECT_CONSTRUCT('dry_run', :P_DRY_RUN);
        RAISE;
END;
$$;
create or replace procedure OPS.SP_APPLY_ORDER_EVENTS(P_BATCH_ID VARCHAR, P_DRY_RUN BOOLEAN Default False) returns variant language SQL strict execute as owner as $$
DECLARE
    v_run_id VARCHAR DEFAULT UUID_STRING();
    v_started_at TIMESTAMP_LTZ DEFAULT CURRENT_TIMESTAMP();
    v_finished_at TIMESTAMP_LTZ;
    v_order_rows NUMBER DEFAULT 0;
    v_deleted_item_rows NUMBER DEFAULT 0;
    v_inserted_item_rows NUMBER DEFAULT 0;
BEGIN
    INSERT INTO OPS.PIPELINE_RUNS (
        run_id,
        batch_id,
        pipeline_name,
        status,
        started_at,
        details
    )
    VALUES (
        :v_run_id,
        :P_BATCH_ID,
        'APPLY_ORDER_EVENTS',
        'RUNNING',
        :v_started_at,
        OBJECT_CONSTRUCT('dry_run', :P_DRY_RUN)
    );

    CREATE OR REPLACE TEMPORARY TABLE OPS.TMP_ORDER_SOURCE AS
    SELECT
        payload:order_id::STRING AS order_id,
        payload:customer_id::STRING AS customer_id,
        payload:status::STRING AS order_status,
        TRY_TO_TIMESTAMP_TZ(payload:order_time::STRING) AS order_time,
        payload:currency::STRING AS currency,
        COALESCE(payload:discount_amount::NUMBER(18, 2), 0) AS discount_amount,
        payload:shipping_address AS shipping_address,
        payload:items AS items,
        event_id,
        event_type,
        event_time
    FROM RAW.ORDER_EVENTS_STREAM
    WHERE batch_id = :P_BATCH_ID
    QUALIFY ROW_NUMBER() OVER (
        PARTITION BY payload:order_id::STRING
        ORDER BY event_time DESC, event_id DESC
    ) = 1;

    BEGIN TRANSACTION;

    MERGE INTO CORE.ORDERS AS target
    USING (
        SELECT
            source.*,
            item_totals.gross_amount,
            item_totals.gross_amount - source.discount_amount AS net_amount
        FROM OPS.TMP_ORDER_SOURCE AS source
        LEFT JOIN LATERAL (
            SELECT
                SUM(
                    item.value:quantity::NUMBER(18, 3)
                    * item.value:unit_price::NUMBER(18, 2)
                ) AS gross_amount
            FROM TABLE(FLATTEN(INPUT => source.items)) AS item
        ) AS item_totals
            ON TRUE
    ) AS source
        ON target.order_id = source.order_id
    WHEN MATCHED AND source.event_type = 'DELETE' THEN
        DELETE
    WHEN MATCHED AND source.event_type <> 'DELETE' THEN
        UPDATE SET
            target.customer_id = source.customer_id,
            target.order_status = source.order_status,
            target.order_time = source.order_time,
            target.currency = source.currency,
            target.gross_amount = source.gross_amount,
            target.discount_amount = source.discount_amount,
            target.net_amount = source.net_amount,
            target.shipping_address = source.shipping_address,
            target.source_event_id = source.event_id,
            target.updated_at = CURRENT_TIMESTAMP()
    WHEN NOT MATCHED AND source.event_type <> 'DELETE' THEN
        INSERT (
            order_id,
            customer_id,
            order_status,
            order_time,
            currency,
            gross_amount,
            discount_amount,
            net_amount,
            shipping_address,
            source_event_id,
            created_at,
            updated_at
        )
        VALUES (
            source.order_id,
            source.customer_id,
            source.order_status,
            source.order_time,
            source.currency,
            source.gross_amount,
            source.discount_amount,
            source.net_amount,
            source.shipping_address,
            source.event_id,
            CURRENT_TIMESTAMP(),
            CURRENT_TIMESTAMP()
        );

    v_order_rows := SQLROWCOUNT;

    DELETE FROM CORE.ORDER_ITEMS
    WHERE order_id IN (
        SELECT order_id
        FROM OPS.TMP_ORDER_SOURCE
    );

    v_deleted_item_rows := SQLROWCOUNT;

    INSERT INTO CORE.ORDER_ITEMS (
        order_id,
        line_number,
        product_id,
        quantity,
        unit_price,
        line_amount,
        attributes
    )
    SELECT
        source.order_id,
        item.value:line::NUMBER,
        item.value:product_id::STRING,
        item.value:quantity::NUMBER(18, 3),
        item.value:unit_price::NUMBER(18, 2),
        item.value:quantity::NUMBER(18, 3)
            * item.value:unit_price::NUMBER(18, 2),
        OBJECT_DELETE(item.value, 'line', 'product_id', 'quantity', 'unit_price')
    FROM OPS.TMP_ORDER_SOURCE AS source,
        LATERAL FLATTEN(INPUT => source.items) AS item
    WHERE source.event_type <> 'DELETE';

    v_inserted_item_rows := SQLROWCOUNT;

    INSERT INTO CORE.ORDER_STATUS_HISTORY (
        order_id,
        status,
        status_at,
        source_event_id
    )
    SELECT
        order_id,
        order_status,
        event_time,
        event_id
    FROM OPS.TMP_ORDER_SOURCE
    WHERE event_type <> 'DELETE';

    IF (P_DRY_RUN) THEN
        ROLLBACK;
    ELSE
        COMMIT;
    END IF;

    v_finished_at := CURRENT_TIMESTAMP();

    UPDATE OPS.PIPELINE_RUNS
    SET
        status = IFF(:P_DRY_RUN, 'DRY_RUN', 'SUCCEEDED'),
        finished_at = :v_finished_at,
        details = OBJECT_CONSTRUCT(
            'orders_affected', :v_order_rows,
            'deleted_items', :v_deleted_item_rows,
            'inserted_items', :v_inserted_item_rows,
            'dry_run', :P_DRY_RUN
        )
    WHERE run_id = :v_run_id;

    RETURN OBJECT_CONSTRUCT(
        'status', IFF(P_DRY_RUN, 'DRY_RUN', 'SUCCEEDED'),
        'run_id', v_run_id,
        'batch_id', P_BATCH_ID,
        'orders_affected', v_order_rows,
        'deleted_items', v_deleted_item_rows,
        'inserted_items', v_inserted_item_rows
    );
EXCEPTION
    WHEN OTHER THEN
        ROLLBACK;
        INSERT INTO OPS.ERROR_LOG (
            run_id,
            batch_id,
            component,
            error_type,
            error_code,
            error_state,
            error_message,
            context
        )
        SELECT
            :v_run_id,
            :P_BATCH_ID,
            'OPS.SP_APPLY_ORDER_EVENTS',
            'UNHANDLED',
            :SQLCODE,
            :SQLSTATE,
            :SQLERRM,
            OBJECT_CONSTRUCT('dry_run', :P_DRY_RUN);
        RAISE;
END;
$$;
create or replace procedure OPS.SP_JS_CAPTURE_SCHEMA_INVENTORY(P_SCHEMA_PATTERN VARCHAR default '%') Returns Variant Language JAVASCRIPT Strict Execute As Owner As $$
const sqlText = `
    SELECT
        table_schema,
        table_name,
        table_type,
        row_count,
        bytes,
        created,
        last_altered
    FROM SNOWFLAKE_FORMATTER_LAB.INFORMATION_SCHEMA.TABLES
    WHERE table_schema ILIKE ?
    ORDER BY table_schema, table_name
`;

const statement = snowflake.createStatement({
    sqlText: sqlText,
    binds: [P_SCHEMA_PATTERN]
});
const resultSet = statement.execute();
const objects = [];
let totalBytes = 0;

while (resultSet.next()) {
    const bytes = resultSet.getColumnValue(5);
    objects.push({
        schema: resultSet.getColumnValue(1),
        name: resultSet.getColumnValue(2),
        type: resultSet.getColumnValue(3),
        row_count: resultSet.getColumnValue(4),
        bytes: bytes,
        created: resultSet.getColumnValue(6),
        last_altered: resultSet.getColumnValue(7)
    });
    totalBytes += bytes === null ? 0 : Number(bytes);
}

const payload = {
    status: "OK",
    schema_pattern: P_SCHEMA_PATTERN,
    object_count: objects.length,
    total_bytes: totalBytes,
    objects: objects,
    query_id: statement.getQueryId()
};

snowflake.createStatement({
    sqlText: "INSERT INTO OPS.SCHEMA_INVENTORY_SNAPSHOTS(payload) SELECT PARSE_JSON(?)",
    binds: [JSON.stringify(payload)]
}).execute();

return payload;
$$;
Create Or Replace Procedure OPS.SP_PY_PROFILE_MULTILINGUAL(P_LIMIT INTEGER Default 1000) returns variant language PYTHON RUNTIME_VERSION = '3.12' PACKAGES = ('snowflake-snowpark-python') HANDLER = 'main' execute as owner as $$
import re
import unicodedata
from collections import Counter
from typing import Any

from snowflake.snowpark import Session

SCRIPT_PATTERNS = {
    "hiragana": re.compile(r"[ぁ-ゟ]"),
    "katakana": re.compile(r"[ァ-ヿ]"),
    "han": re.compile(r"[一-鿿]"),
    "hangul": re.compile(r"[가-힣]"),
    "arabic": re.compile(r"[؀-ۿ]"),
    "hebrew": re.compile(r"[֐-׿]"),
    "devanagari": re.compile(r"[ऀ-ॿ]"),
    "thai": re.compile(r"[ก-฿]"),
}

def main(session: Session, p_limit: int) -> dict[str, Any]:
    rows = (
        session.table("CORE.MULTILINGUAL_TEXTS")
        .select("TEXT_ID", "LANGUAGE_TAG", "TEXT_VALUE")
        .limit(max(0, min(int(p_limit), 10000)))
        .collect()
    )

    scripts: Counter[str] = Counter()
    changed_by_nfkc = 0
    samples: list[dict[str, Any]] = []

    for row in rows:
        text = str(row["TEXT_VALUE"] or "")
        normalized = unicodedata.normalize("NFKC", text)
        detected = [
            name
            for name, pattern in SCRIPT_PATTERNS.items()
            if pattern.search(text)
        ]

        if text != normalized:
            changed_by_nfkc += 1

        for script in detected or ["latin_or_other"]:
            scripts[script] += 1

        if len(samples) < 10:
            samples.append(
                {
                    "text_id": row["TEXT_ID"],
                    "language_tag": row["LANGUAGE_TAG"],
                    "original": text,
                    "nfkc": normalized,
                    "scripts": detected,
                }
            )

    return {
        "status": "OK",
        "row_count": len(rows),
        "changed_by_nfkc": changed_by_nfkc,
        "script_counts": dict(sorted(scripts.items())),
        "samples": samples,
    }
$$;
-- 04_run_pipeline.sql
call OPS.SP_APPLY_CUSTOMER_EVENTS('BATCH-001', False);
Call OPS.SP_APPLY_ORDER_EVENTS('BATCH-001', false);
call OPS.SP_JS_CAPTURE_SCHEMA_INVENTORY('%');
call OPS.SP_PY_PROFILE_MULTILINGUAL(1000);
create or replace view MART.V_CUSTOMER_360 as with order_rollup as (Select customer_id, COUNT(*) As order_count, COUNT_IF(order_status = 'DELIVERED') As delivered_order_count, SUM(net_amount) As lifetime_net_amount, MAX(order_time) As last_order_time, ARRAY_AGG(OBJECT_CONSTRUCT('order_id', order_id, 'status', order_status, 'net_amount', net_amount, 'currency', currency, 'order_time', order_time)) within group (Order By order_time Desc) as orders from CORE.ORDERS group by customer_id), item_rollup as (Select orders.customer_id, COUNT(distinct items.product_id) As distinct_products, SUM(items.quantity) As total_item_quantity, ARRAY_AGG(distinct items.product_id) As product_ids From CORE.ORDERS As orders Inner Join CORE.ORDER_ITEMS As items On orders.order_id = items.order_id Group By orders.customer_id) select customers.customer_id, customers.display_name, customers.email, customers.locale, customers.region, customers.marketing_opt_in, customers.attributes, COALESCE(order_rollup.order_count, 0) As order_count, COALESCE(order_rollup.delivered_order_count, 0) as delivered_order_count, COALESCE(order_rollup.lifetime_net_amount, 0) As lifetime_net_amount, order_rollup.last_order_time, COALESCE(item_rollup.distinct_products, 0) As distinct_products, COALESCE(item_rollup.total_item_quantity, 0) as total_item_quantity, item_rollup.product_ids, order_rollup.orders, Case When COALESCE(order_rollup.lifetime_net_amount, 0) >= 10000 then 'HIGH' When COALESCE(order_rollup.lifetime_net_amount, 0) >= 1000 then 'MEDIUM' Else 'LOW' End As value_segment, customers.updated_at from CORE.CUSTOMERS as customers left join order_rollup on customers.customer_id = order_rollup.customer_id left join item_rollup on customers.customer_id = item_rollup.customer_id;
Create Or Replace View MART.V_ORDER_FACTS As Select orders.order_id, orders.customer_id, customers.display_name, customers.locale, customers.region, orders.order_status, orders.order_time, orders.order_time::DATE As order_date, orders.currency, orders.gross_amount, orders.discount_amount, orders.net_amount, orders.shipping_address:country::STRING As shipping_country, orders.shipping_address:city::STRING As shipping_city, COUNT(items.line_number) As line_count, SUM(items.quantity) As total_quantity, ARRAY_AGG(OBJECT_CONSTRUCT('line', items.line_number, 'product_id', items.product_id, 'quantity', items.quantity, 'line_amount', items.line_amount)) within group (Order By items.line_number) as items from CORE.ORDERS as orders inner join CORE.CUSTOMERS as customers on orders.customer_id = customers.customer_id left join CORE.ORDER_ITEMS as items on orders.order_id = items.order_id group by orders.order_id, orders.customer_id, customers.display_name, customers.locale, customers.region, orders.order_status, orders.order_time, orders.currency, orders.gross_amount, orders.discount_amount, orders.net_amount, orders.shipping_address;
-- Second delta batch: update Japanese customer, delete Hebrew customer, add Thai customer.
Insert Into RAW.CUSTOMER_EVENTS (event_id, batch_id, event_time, event_type, source_system, payload) select column1, column2, column3::TIMESTAMP_TZ, column4, column5, PARSE_JSON(column6) From Values ('CE101', 'BATCH-002', '2026-06-10 09:00:00 +09:00', 'UPSERT', 'web-ja', '{"customer_id":"C001","display_name":"杉野尾 広貴（更新）","email":"hiroki@example.jp","locale":"ja-JP","region":"JP","marketing_opt_in":false,"attributes":{"interests":["cloud","分散システム","Elixir"],"tier":"platinum"}}'), ('CE102', 'BATCH-002', '2026-06-10 10:00:00 +03:00', 'DELETE', 'privacy-he', '{"customer_id":"C006"}'), ('CE103', 'BATCH-002', '2026-06-10 11:00:00 +07:00', 'UPSERT', 'mobile-th', '{"customer_id":"C007","display_name":"พิมพ์ชนก ใจดี","email":"pim@example.th","locale":"th-TH","region":"TH","marketing_opt_in":true,"attributes":{"interests":["คลาวด์","ข้อมูล"],"tier":"silver"}}');
Insert Into RAW.ORDER_EVENTS (event_id, batch_id, event_time, event_type, source_system, payload) select column1, column2, column3::TIMESTAMP_TZ, column4, column5, PARSE_JSON(column6) From Values ('OE101', 'BATCH-002', '2026-06-10 10:30:00 +02:00', 'UPSERT', 'fulfillment-fr', '{"order_id":"O1002","customer_id":"C002","status":"SHIPPED","order_time":"2026-06-02T10:00:00+02:00","currency":"EUR","discount_amount":0,"shipping_address":{"country":"FR","city":"Lyon"},"items":[{"line":1,"product_id":"P003","quantity":3,"unit_price":18.5}]}'), ('OE102', 'BATCH-002', '2026-06-10 12:00:00 +03:00', 'DELETE', 'privacy-ar', '{"order_id":"O1004"}'), ('OE103', 'BATCH-002', '2026-06-10 15:00:00 +07:00', 'UPSERT', 'checkout-th', '{"order_id":"O1006","customer_id":"C007","status":"PAID","order_time":"2026-06-10T14:45:00+07:00","currency":"JPY","discount_amount":0,"shipping_address":{"country":"TH","city":"กรุงเทพมหานคร"},"items":[{"line":1,"product_id":"P002","quantity":1,"unit_price":1800}]}');
call OPS.SP_APPLY_CUSTOMER_EVENTS('BATCH-002', False);
Call OPS.SP_APPLY_ORDER_EVENTS('BATCH-002', false);
-- 05_analytics.sql
-- Complex analytical queries intended for formatter and scenario validation.
with ranked_customers as (Select *, DENSE_RANK() over (Partition By region Order By lifetime_net_amount Desc, customer_id) As region_value_rank, PERCENT_RANK() over (Order By lifetime_net_amount) as global_value_percentile from MART.V_CUSTOMER_360) Select region, customer_id, display_name, locale, order_count, lifetime_net_amount, region_value_rank, global_value_percentile, value_segment, ARRAY_SIZE(product_ids) As product_count From ranked_customers Qualify region_value_rank <= 3 order by region, region_value_rank, customer_id;
Select * From (select region, locale, net_amount from MART.V_ORDER_FACTS) Pivot (SUM(net_amount) For locale In ('ja-JP' as JA, 'fr-FR' As FR, 'ko-KR' as KO, 'hi-IN' As HI, 'th-TH' as TH)) order by region;
With metrics As (select customer_id, order_count, delivered_order_count, lifetime_net_amount, total_item_quantity from MART.V_CUSTOMER_360) Select customer_id, metric_name, metric_value From metrics Unpivot Include Nulls (metric_value for metric_name in (order_count, delivered_order_count, lifetime_net_amount, total_item_quantity)) order by customer_id, metric_name;
select order_id, match_number, first_status, last_status, first_status_at, last_status_at From CORE.ORDER_STATUS_HISTORY Match_Recognize (partition by order_id order by status_at measures MATCH_NUMBER() as match_number, FIRST(any_status.status) as first_status, LAST(any_status.status) as last_status, FIRST(any_status.status_at) as first_status_at, LAST(any_status.status_at) as last_status_at one row PER MATCH after MATCH SKIP PAST LAST row pattern (any_status+) DEFINE any_status As any_status.status Is Not Null) order by order_id, match_number;
select text_id, language_tag, text_value, LENGTH(text_value) as character_count, OCTET_LENGTH(text_value) as utf8_byte_count, REGEXP_COUNT(text_value, '[[:alpha:]]') As alphabetic_count, metadata:script::STRING As declared_script, metadata:direction::STRING As direction From CORE.MULTILINGUAL_TEXTS Where text_value Ilike Any ('%データ%', '%데이터%', '%بيانات%', '%נתונים%', '%डेटा%') or language_tag in ('ja-JP', 'ko-KR', 'ar-SA', 'he-IL', 'hi-IN') order by language_tag, text_id;
-- 06_tasks.sql
-- Tasks are created suspended by Snowflake. They are not resumed by this suite.
Create Or Replace Task OPS.TASK_PIPELINE_ROOT Warehouse = FORMATTER_TEST_WH Schedule = 'USING CRON 0 * * * * UTC' USER_TASK_TIMEOUT_MS = 1800000 SUSPEND_TASK_AFTER_NUM_FAILURES = 3 TASK_AUTO_RETRY_ATTEMPTS = 1 comment = 'Root task; formatter E2E leaves it suspended' As Insert Into OPS.PIPELINE_RUNS (run_id, batch_id, pipeline_name, status, started_at, details) select UUID_STRING(), 'TASK-' || TO_VARCHAR(CURRENT_TIMESTAMP(), 'YYYYMMDDHH24MISS'), 'SCHEDULED_ROOT', 'SUCCEEDED', CURRENT_TIMESTAMP(), OBJECT_CONSTRUCT('message', 'root heartbeat / ルート起動');
create or replace task OPS.TASK_APPLY_CUSTOMERS warehouse = FORMATTER_TEST_WH after OPS.TASK_PIPELINE_ROOT when SYSTEM$STREAM_HAS_DATA('RAW.CUSTOMER_EVENTS_STREAM') as call OPS.SP_APPLY_CUSTOMER_EVENTS('TASK-BATCH', false);
create or replace task OPS.TASK_APPLY_ORDERS warehouse = FORMATTER_TEST_WH after OPS.TASK_PIPELINE_ROOT when SYSTEM$STREAM_HAS_DATA('RAW.ORDER_EVENTS_STREAM') As Call OPS.SP_APPLY_ORDER_EVENTS('TASK-BATCH', False);
Create Or Replace Task OPS.TASK_CAPTURE_INVENTORY Warehouse = FORMATTER_TEST_WH After OPS.TASK_APPLY_CUSTOMERS, OPS.TASK_APPLY_ORDERS as call OPS.SP_JS_CAPTURE_SCHEMA_INVENTORY('%');
create or replace task OPS.TASK_PIPELINE_FINALIZER warehouse = FORMATTER_TEST_WH finalize = OPS.TASK_PIPELINE_ROOT as insert into OPS.PIPELINE_RUNS (run_id, batch_id, pipeline_name, status, started_at, finished_at, details) select UUID_STRING(), 'TASK-FINALIZER', 'SCHEDULED_FINALIZER', 'SUCCEEDED', CURRENT_TIMESTAMP(), CURRENT_TIMESTAMP(), OBJECT_CONSTRUCT('message_ja', 'タスクグラフ完了', 'message_ko', '태스크 그래프 완료', 'message_ar', 'اكتمل مخطط المهام');
-- 07_assertions.sql
Create Or Replace Procedure OPS.SP_VALIDATE_E2E() Returns Table (assertion_name VARCHAR, expected_value Variant, actual_value variant, passed BOOLEAN, details variant) Language SQL Execute As Owner As $$
DECLARE
    v_failure_count NUMBER DEFAULT 0;
    v_results RESULTSET;
    e_assertion_failed EXCEPTION (-20100, 'One or more E2E assertions failed.');
BEGIN
    TRUNCATE TABLE OPS.ASSERTION_RESULTS;

    INSERT INTO OPS.ASSERTION_RESULTS (
        assertion_name,
        expected_value,
        actual_value,
        passed,
        details
    )
    SELECT
        'customer_count_after_delta',
        6::VARIANT,
        COUNT(*)::VARIANT,
        COUNT(*) = 6,
        OBJECT_CONSTRUCT('table', 'CORE.CUSTOMERS')
    FROM CORE.CUSTOMERS

    UNION ALL

    SELECT
        'order_count_after_delta',
        5::VARIANT,
        COUNT(*)::VARIANT,
        COUNT(*) = 5,
        OBJECT_CONSTRUCT('table', 'CORE.ORDERS')
    FROM CORE.ORDERS

    UNION ALL

    SELECT
        'order_item_count_after_delta',
        8::VARIANT,
        COUNT(*)::VARIANT,
        COUNT(*) = 8,
        OBJECT_CONSTRUCT('table', 'CORE.ORDER_ITEMS')
    FROM CORE.ORDER_ITEMS

    UNION ALL

    SELECT
        'deleted_hebrew_customer_absent',
        0::VARIANT,
        COUNT_IF(customer_id = 'C006')::VARIANT,
        COUNT_IF(customer_id = 'C006') = 0,
        OBJECT_CONSTRUCT('customer_id', 'C006')
    FROM CORE.CUSTOMERS

    UNION ALL

    SELECT
        'thai_customer_present',
        1::VARIANT,
        COUNT_IF(customer_id = 'C007' AND locale = 'th-TH')::VARIANT,
        COUNT_IF(customer_id = 'C007' AND locale = 'th-TH') = 1,
        OBJECT_CONSTRUCT('customer_id', 'C007')
    FROM CORE.CUSTOMERS

    UNION ALL

    SELECT
        'deleted_arabic_order_absent',
        0::VARIANT,
        COUNT_IF(order_id = 'O1004')::VARIANT,
        COUNT_IF(order_id = 'O1004') = 0,
        OBJECT_CONSTRUCT('order_id', 'O1004')
    FROM CORE.ORDERS

    UNION ALL

    SELECT
        'updated_french_order_shipped',
        'SHIPPED'::VARIANT,
        MAX(IFF(order_id = 'O1002', order_status, NULL))::VARIANT,
        MAX(IFF(order_id = 'O1002', order_status, NULL)) = 'SHIPPED',
        OBJECT_CONSTRUCT('order_id', 'O1002')
    FROM CORE.ORDERS

    UNION ALL

    SELECT
        'multilingual_text_count',
        9::VARIANT,
        COUNT(*)::VARIANT,
        COUNT(*) = 9,
        OBJECT_CONSTRUCT('table', 'CORE.MULTILINGUAL_TEXTS')
    FROM CORE.MULTILINGUAL_TEXTS

    UNION ALL

    SELECT
        'customer_360_count',
        6::VARIANT,
        COUNT(*)::VARIANT,
        COUNT(*) = 6,
        OBJECT_CONSTRUCT('view', 'MART.V_CUSTOMER_360')
    FROM MART.V_CUSTOMER_360

    UNION ALL

    SELECT
        'pipeline_errors',
        0::VARIANT,
        COUNT(*)::VARIANT,
        COUNT(*) = 0,
        OBJECT_CONSTRUCT('table', 'OPS.ERROR_LOG')
    FROM OPS.ERROR_LOG;

    SELECT COUNT_IF(NOT passed)
    INTO :v_failure_count
    FROM OPS.ASSERTION_RESULTS;

    IF (v_failure_count > 0) THEN
        RAISE e_assertion_failed;
    END IF;

    v_results := (
        SELECT
            assertion_name,
            expected_value,
            actual_value,
            passed,
            details
        FROM OPS.ASSERTION_RESULTS
        ORDER BY assertion_name
    );

    RETURN TABLE(v_results);
END;
$$;
call OPS.SP_VALIDATE_E2E();
Select customer_id, display_name, locale, region, order_count, lifetime_net_amount, value_segment From MART.V_CUSTOMER_360 Order By customer_id;
