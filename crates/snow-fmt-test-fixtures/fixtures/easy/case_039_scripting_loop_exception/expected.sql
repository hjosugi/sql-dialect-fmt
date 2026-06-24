BEGIN
    FOR rec IN (SELECT id FROM tasks) DO
        INSERT INTO processed (id)
        VALUES (rec.id);
    END FOR;
EXCEPTION
    WHEN statement_error THEN
        ROLLBACK;
        RETURN 'failed';
    WHEN other THEN
        RETURN 'unknown';
END;
