update t set a = 1 where id = 5;
commit work;
undrop table db.s.archived;
rollback to savepoint sp1;
