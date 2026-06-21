-- 07_assertions.sql
CREATE OR REPLACE PROCEDURE OPS.SP_VALIDATE_E2E()
RETURNS TABLE (
    assertion_name VARCHAR,
    expected_value VARIANT,
    actual_value VARIANT,
    passed BOOLEAN,
    details VARIANT
)
LANGUAGE SQL
EXECUTE AS OWNER
AS
$$
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

CALL OPS.SP_VALIDATE_E2E();

SELECT
    customer_id,
    display_name,
    locale,
    region,
    order_count,
    lifetime_net_amount,
    value_segment
FROM MART.V_CUSTOMER_360
ORDER BY customer_id;
