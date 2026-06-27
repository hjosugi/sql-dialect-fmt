-- Representative CREATE SEMANTIC VIEW with tables, relationships, facts, dimensions, and metrics.
CREATE OR REPLACE SEMANTIC VIEW SEMANTIC.SV_ORDER_REVENUE
    TABLES (
        orders AS MART.FACT_ORDER PRIMARY KEY (order_id) WITH SYNONYMS ('orders') COMMENT = 'Order fact',
        customers AS MART.DIM_CUSTOMER PRIMARY KEY (customer_id)
    )
    RELATIONSHIPS (
        order_customer AS orders(customer_id) REFERENCES customers
    )
    FACTS (
        PUBLIC orders.net_amount AS net_amount COMMENT = 'Net order amount'
    )
    DIMENSIONS (
        PUBLIC customers.region AS region WITH SYNONYMS ('area')
    )
    METRICS (
        PUBLIC orders.revenue AS SUM(orders.net_amount)
    )
    COMMENT = 'Semantic view for order revenue'
    COPY GRANTS;
