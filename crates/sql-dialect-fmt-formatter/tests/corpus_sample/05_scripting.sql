-- Representative Snowflake Scripting procedure with a SQL body.
CREATE OR REPLACE PROCEDURE ANALYTICS.REFRESH_SUMMARY (
    target_date DATE
) RETURNS STRING LANGUAGE SQL AS $$
BEGIN
    DELETE FROM ANALYTICS.DAILY_SUMMARY
    WHERE summary_date = :target_date;
    INSERT INTO ANALYTICS.DAILY_SUMMARY (summary_date, order_count)
    SELECT :target_date, COUNT(*)
    FROM SALES.ORDERS
    WHERE order_date = :target_date;
    RETURN 'ok';
END;
$$;
