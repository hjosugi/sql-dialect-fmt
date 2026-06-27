-- MERGE with matched update/delete and not-matched insert.
MERGE INTO main.default.dim_user AS target
USING main.default.user_delta AS source
ON target.id = source.id
WHEN MATCHED THEN UPDATE SET target.email = source.email
WHEN NOT MATCHED THEN INSERT (id, email) VALUES (source.id, source.email);
