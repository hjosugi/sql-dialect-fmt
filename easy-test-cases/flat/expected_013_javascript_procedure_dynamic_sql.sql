-- Case 013: JavaScript stored procedure with dynamic SQL, template strings, binds, and result handling.
CREATE OR REPLACE PROCEDURE OPS.SP_JS_DEEP_SCHEMA_PROFILE(
    P_DATABASE STRING,
    P_SCHEMA_PATTERN STRING DEFAULT '%',
    P_INCLUDE_COLUMNS BOOLEAN DEFAULT TRUE
)
RETURNS VARIANT
LANGUAGE JAVASCRIPT
STRICT
EXECUTE AS CALLER
AS
$$
function quoteIdent(value) {
    if (value === null || value === undefined || !/^[A-Za-z_][A-Za-z0-9_$]*$/.test(String(value))) {
        throw new Error("Unsafe identifier / 危険な識別子: " + value);
    }
    return '"' + String(value).replace(/"/g, '""') + '"';
}

const db = quoteIdent(P_DATABASE);
const sqlText = `
    WITH tables AS (
        SELECT
            table_catalog,
            table_schema,
            table_name,
            table_type,
            row_count,
            bytes,
            created,
            last_altered,
            comment,
            ROW_NUMBER() OVER (
                PARTITION BY table_schema
                ORDER BY bytes DESC NULLS LAST, table_name
            ) AS size_rank
        FROM ${db}.INFORMATION_SCHEMA.TABLES
        WHERE table_schema ILIKE ?
        QUALIFY size_rank <= 100
    ), columns AS (
        SELECT
            table_catalog,
            table_schema,
            table_name,
            ARRAY_AGG(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'name', column_name,
                    'type', data_type,
                    'nullable', is_nullable,
                    'ordinal', ordinal_position,
                    'comment', comment
                )
            ) WITHIN GROUP (ORDER BY ordinal_position) AS column_docs
        FROM ${db}.INFORMATION_SCHEMA.COLUMNS
        WHERE ? AND table_schema ILIKE ?
        GROUP BY table_catalog, table_schema, table_name
    )
    SELECT
        t.table_schema,
        ARRAY_AGG(
            OBJECT_CONSTRUCT_KEEP_NULL(
                'name', t.table_name,
                'type', t.table_type,
                'rows', t.row_count,
                'bytes', t.bytes,
                'created', t.created,
                'last_altered', t.last_altered,
                'comment', t.comment,
                'columns', c.column_docs
            )
        ) WITHIN GROUP (ORDER BY t.size_rank, t.table_name) AS objects
    FROM tables AS t
        LEFT JOIN columns AS c
            ON t.table_catalog = c.table_catalog
            AND t.table_schema = c.table_schema
            AND t.table_name = c.table_name
    GROUP BY t.table_schema
    ORDER BY t.table_schema
`;

const stmt = snowflake.createStatement({
    sqlText: sqlText,
    binds: [P_SCHEMA_PATTERN, P_INCLUDE_COLUMNS, P_SCHEMA_PATTERN]
});
const rs = stmt.execute();
const schemas = {};
let totalObjects = 0;

while (rs.next()) {
    const schemaName = rs.getColumnValue(1);
    const objects = rs.getColumnValue(2) || [];
    schemas[schemaName] = {
        object_count: objects.length,
        objects: objects,
        note: "schema profile / スキーマプロファイル / ملف المخطط"
    };
    totalObjects += objects.length;
}

snowflake.log("info", `Profile completed: db=${P_DATABASE}, objects=${totalObjects}`);
return {
    status: "OK",
    database: P_DATABASE,
    schema_pattern: P_SCHEMA_PATTERN,
    include_columns: P_INCLUDE_COLUMNS,
    total_objects: totalObjects,
    schemas: schemas,
    query_id: stmt.getQueryId(),
    generated_at: new Date().toISOString()
};
$$;
