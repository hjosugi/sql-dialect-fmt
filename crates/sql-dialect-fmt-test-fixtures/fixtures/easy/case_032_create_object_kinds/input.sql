create schema if not exists analytics;
create or replace database d clone src;
create warehouse wh with warehouse_size = xsmall auto_suspend = 60 auto_resume = true;
create sequence seq start = 1 increment = 1;
create stage st url = 's3://bucket/path' file_format = (type = csv);
create or replace file format ff type = json strip_outer_array = true;
