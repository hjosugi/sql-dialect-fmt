USE role sysadmin;

USE WAREHOUSE compute_wh;

USE schema db.analytics;

SHOW tables IN schema db.analytics;

DESCRIBE TABLE db.analytics.orders;

DESC WAREHOUSE compute_wh;

TRUNCATE TABLE db.analytics.staging;
