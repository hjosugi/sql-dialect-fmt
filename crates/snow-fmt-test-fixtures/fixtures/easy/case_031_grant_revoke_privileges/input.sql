grant select,insert,update on table db.sch.orders to role analyst;
grant select(c1,c2) on view db.sch.v to role reader;
grant usage on warehouse compute_wh to role  analyst;
grant ownership on table t to role admin copy current grants;
revoke insert on table db.sch.orders from role analyst;
grant select on future tables in schema db.sch to role reader;
