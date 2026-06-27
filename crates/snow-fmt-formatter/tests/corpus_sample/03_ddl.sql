-- Representative DDL: table, view, and warehouse objects.
CREATE OR REPLACE TABLE ANALYTICS.ORDER_FACTS (
    order_id NUMBER(38, 0) NOT NULL,
    customer_id NUMBER(38, 0),
    order_date DATE,
    net_amount NUMBER(18, 2) DEFAULT 0,
    payload VARIANT,
    PRIMARY KEY (order_id)
) COMMENT = 'Order fact table';

CREATE OR REPLACE VIEW ANALYTICS.ACTIVE_CUSTOMERS AS
SELECT customer_id, email, status
FROM SALES.CUSTOMERS
WHERE status = 'ACTIVE';

CREATE WAREHOUSE IF NOT EXISTS LOAD_WH
    WAREHOUSE_SIZE = 'XSMALL'
    AUTO_SUSPEND = 60
    AUTO_RESUME = TRUE;
