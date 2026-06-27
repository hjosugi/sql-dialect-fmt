-- Case 03: JavaScript stored procedure with dynamic SQL and bind variables.
CREATE OR REPLACE PROCEDURE OPS.SP_JS_SCHEMA_INVENTORY(
    P_DATABASE VARCHAR,
    P_SCHEMA_PATTERN VARCHAR DEFAULT '%',
    P_INCLUDE_VIEWS BOOLEAN DEFAULT TRUE
)
RETURNS VARIANT
LANGUAGE JAVASCRIPT
STRICT
EXECUTE AS CALLER
AS
$$
function quoteIdentifier(value) {
    if (value === null || value === undefined || value.length === 0) {
        throw new Error("Identifier must not be empty / 識別子が空です");
    }

    return '"' + String(value).replace(/"/g, '""') + '"';
}

function asIsoString(value) {
    return value === null ? null : new Date(value).toISOString();
}

const databaseIdentifier = quoteIdentifier(P_DATABASE);
const sqlText = `
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
    FROM ${databaseIdentifier}.INFORMATION_SCHEMA.TABLES
    WHERE
        table_schema ILIKE ?
        AND (
            table_type = 'BASE TABLE'
            OR (? AND table_type = 'VIEW')
        )
    QUALIFY size_rank <= 50
    ORDER BY table_schema, size_rank, table_name
`;

const statement = snowflake.createStatement({
    sqlText: sqlText,
    binds: [P_SCHEMA_PATTERN, P_INCLUDE_VIEWS]
});
const resultSet = statement.execute();
const schemas = {};
let objectCount = 0;
let totalBytes = 0;

while (resultSet.next()) {
    const schemaName = resultSet.getColumnValue(2);
    const rowCount = resultSet.getColumnValue(5);
    const bytes = resultSet.getColumnValue(6);

    if (!schemas[schemaName]) {
        schemas[schemaName] = {
            objects: [],
            total_bytes: 0,
            notes: {
                ja: "スキーマ別インベントリ",
                fr: "inventaire par schéma",
                ar: "جرد حسب المخطط"
            }
        };
    }

    schemas[schemaName].objects.push({
        catalog: resultSet.getColumnValue(1),
        name: resultSet.getColumnValue(3),
        type: resultSet.getColumnValue(4),
        row_count: rowCount,
        bytes: bytes,
        created_at: asIsoString(resultSet.getColumnValue(7)),
        last_altered_at: asIsoString(resultSet.getColumnValue(8)),
        comment: resultSet.getColumnValue(9),
        size_rank: resultSet.getColumnValue(10)
    });

    schemas[schemaName].total_bytes += bytes === null ? 0 : Number(bytes);
    totalBytes += bytes === null ? 0 : Number(bytes);
    objectCount += 1;
}

snowflake.log(
    "info",
    `Inventory completed: database=${P_DATABASE}, pattern=${P_SCHEMA_PATTERN}, objects=${objectCount}`
);

return {
    status: "OK",
    database: P_DATABASE,
    schema_pattern: P_SCHEMA_PATTERN,
    include_views: P_INCLUDE_VIEWS,
    object_count: objectCount,
    total_bytes: totalBytes,
    schemas: schemas,
    query_id: statement.getQueryId(),
    generated_at: new Date().toISOString()
};
$$;
