-- Plain DML.
INSERT INTO main.default.summary (dt, n)
SELECT dt, count(*)
FROM main.default.events
GROUP BY dt;

UPDATE main.default.users
SET active = FALSE
WHERE last_seen < '2023-01-01';

DELETE FROM main.default.staging
WHERE ingested < '2024-01-01';
