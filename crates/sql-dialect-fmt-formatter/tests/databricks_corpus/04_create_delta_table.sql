-- Delta table DDL with USING / LOCATION / PARTITIONED BY / TBLPROPERTIES.
CREATE TABLE main.default.events (id bigint, payload string, dt string)
    USING delta
    LOCATION '/mnt/events'
    PARTITIONED BY (dt)
    TBLPROPERTIES ('delta.enableChangeDataFeed' = 'true');
