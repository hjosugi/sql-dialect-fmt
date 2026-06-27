-- Parquet external table with USING + LOCATION.
CREATE OR REPLACE TABLE analytics.raw (id bigint, body string)
    USING parquet
    LOCATION '/mnt/raw';
