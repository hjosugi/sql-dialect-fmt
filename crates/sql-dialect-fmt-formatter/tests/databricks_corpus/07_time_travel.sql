-- Delta time travel: VERSION AS OF and TIMESTAMP AS OF, on refs and in joins.
SELECT *
FROM main.default.events VERSION AS OF 12
WHERE id > 0;

SELECT a.id, b.name
FROM main.default.events AS a
JOIN main.default.users TIMESTAMP AS OF '2024-01-01' AS b ON a.user_id = b.id;
