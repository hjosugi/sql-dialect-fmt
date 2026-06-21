-- 03_procedures.sql
CREATE OR REPLACE PROCEDURE OPS.SP_APPLY_CUSTOMER_EVENTS(
    P_BATCH_ID VARCHAR,
    P_DRY_RUN BOOLEAN DEFAULT FALSE
)
RETURNS VARIANT
LANGUAGE SQL
STRICT
EXECUTE AS OWNER
AS
$$
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

CREATE OR REPLACE PROCEDURE OPS.SP_APPLY_ORDER_EVENTS(
    P_BATCH_ID VARCHAR,
    P_DRY_RUN BOOLEAN DEFAULT FALSE
)
RETURNS VARIANT
LANGUAGE SQL
STRICT
EXECUTE AS OWNER
AS
$$
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

CREATE OR REPLACE PROCEDURE OPS.SP_JS_CAPTURE_SCHEMA_INVENTORY(
    P_SCHEMA_PATTERN VARCHAR DEFAULT '%'
)
RETURNS VARIANT
LANGUAGE JAVASCRIPT
STRICT
EXECUTE AS OWNER
AS
$$
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

CREATE OR REPLACE PROCEDURE OPS.SP_PY_PROFILE_MULTILINGUAL(
    P_LIMIT INTEGER DEFAULT 1000
)
RETURNS VARIANT
LANGUAGE PYTHON
RUNTIME_VERSION = '3.12'
PACKAGES = ('snowflake-snowpark-python')
HANDLER = 'main'
EXECUTE AS OWNER
AS
$$
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
