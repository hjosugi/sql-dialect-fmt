GRANT SELECT, INSERT, UPDATE ON TABLE db.sch.orders to role analyst;

GRANT SELECT (c1, c2) ON VIEW db.sch.v to role reader;

GRANT usage ON WAREHOUSE compute_wh to role analyst;

GRANT ownership ON TABLE t to role admin COPY CURRENT GRANTS;

REVOKE INSERT ON TABLE db.sch.orders FROM role analyst;

GRANT SELECT ON future tables IN schema db.sch to role reader;
