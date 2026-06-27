UPDATE t
SET a = 1
WHERE id = 5;

COMMIT work;

UNDROP TABLE db.s.archived;

ROLLBACK to savepoint sp1;
