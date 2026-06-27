-- Representative DML: INSERT, UPDATE, DELETE, and MERGE.
INSERT INTO ANALYTICS.DAILY_SUMMARY (summary_date, order_count, gross_amount)
SELECT order_date, COUNT(*), SUM(net_amount)
FROM SALES.ORDERS
GROUP BY order_date;

UPDATE SALES.CUSTOMERS
SET status = 'INACTIVE', updated_at = CURRENT_TIMESTAMP()
WHERE last_order_date < DATEADD('year', -1, CURRENT_DATE());

DELETE FROM STAGING.EVENTS
WHERE ingested_at < DATEADD('day', -7, CURRENT_DATE());

MERGE INTO MART.DIM_CUSTOMER AS target
USING STAGING.CUSTOMER_DELTA AS source
ON target.customer_id = source.customer_id
WHEN MATCHED THEN UPDATE SET target.email = source.email
WHEN NOT MATCHED THEN INSERT (customer_id, email) VALUES (source.customer_id, source.email);
