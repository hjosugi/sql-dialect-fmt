-- Case 029: nested Snowflake Scripting block with cursor, exception handlers, and dynamic table names
EXECUTE IMMEDIATE
$$
DECLARE
    v_schema STRING DEFAULT 'MART';
    v_table STRING;
    v_sql STRING;
    v_ok_count NUMBER DEFAULT 0;
    v_error_count NUMBER DEFAULT 0;
    c_tables CURSOR FOR
        SELECT table_name
        FROM INFORMATION_SCHEMA.TABLES
        WHERE
            table_schema = ?
            AND table_type = 'BASE TABLE'
            AND table_name ILIKE 'FACT_%'
        ORDER BY table_name;
BEGIN
    FOR t IN c_tables USING (v_schema) DO
        v_table := v_schema || '.' || t.table_name;
        BEGIN
            v_sql := 'INSERT INTO OPS.TABLE_HEALTH_CHECK(table_name, row_count, null_key_count, checked_at) '
                || 'SELECT ?, COUNT(*), COUNT_IF(id IS NULL), CURRENT_TIMESTAMP() FROM ' || v_table;
            EXECUTE IMMEDIATE :v_sql USING (v_table);
            v_ok_count := v_ok_count + 1;
        EXCEPTION
            WHEN STATEMENT_ERROR CONTINUE THEN
                v_error_count := v_error_count + 1;
                INSERT INTO OPS.TABLE_HEALTH_CHECK_ERROR (
                    table_name,
                    error_code,
                    error_message,
                    error_state,
                    checked_at
                )
                SELECT
                    :v_table,
                    :SQLCODE,
                    :SQLERRM,
                    :SQLSTATE,
                    CURRENT_TIMESTAMP();
        END;
    END FOR;

    RETURN OBJECT_CONSTRUCT('ok_count', v_ok_count, 'error_count', v_error_count);
END;
$$;
