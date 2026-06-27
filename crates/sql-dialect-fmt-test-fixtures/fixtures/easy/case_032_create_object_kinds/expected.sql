CREATE schema IF NOT EXISTS analytics;

CREATE OR REPLACE database d clone src;

CREATE WAREHOUSE wh WITH warehouse_size = xsmall auto_suspend = 60 auto_resume = TRUE;

CREATE sequence seq START = 1 increment = 1;

CREATE stage st url = 's3://bucket/path' file_format = (type = csv);

CREATE OR REPLACE file format ff type = json strip_outer_array = TRUE;
