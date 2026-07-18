USE ROLE SYSADMIN;
USE WAREHOUSE DEV_TEST;
CREATE OR REPLACE PROCEDURE APP.ANALYTICS.LOAD_EVENTS (
    P_ENV STRING,
    P_YEAR STRING,
    P_FORCE BOOLEAN
) RETURNS VARIANT LANGUAGE JAVASCRIPT EXECUTE AS CALLER AS $$
var env = (P_ENV || "").toUpperCase();
var year = P_YEAR || "";
if (!/^[0-9]{4}$/.test(year)) {
    throw "P_YEAR must be a 4-digit year: " + year;
}
var cfg = {};
if (env === "STAGE") {
    cfg.table = "APP.ANALYTICS.EVENTS";
    cfg.stage = "@APP.RAW.EVENT_STAGE/year=" + year;
    cfg.format = "APP.RAW.PARQUET_FORMAT";
} else {
    throw "P_ENV must be STAGE: " + P_ENV;
}
var force = P_FORCE ? "TRUE" : "FALSE";
var copySql = `COPY INTO ${cfg.table} (
    EVENT_ID,
    EVENT_TIME,
    EVENT_DATE
)
FROM (
    SELECT
        $1:event_id::STRING AS EVENT_ID,
        $1:event_time::TIMESTAMP_NTZ AS EVENT_TIME,
        $1:event_date::DATE AS EVENT_DATE
    FROM ${cfg.stage} (FILE_FORMAT => ${cfg.format})
)
PATTERN = '.*year=${year}/[0-9]+_[0-9]+$'
ON_ERROR = ABORT_STATEMENT
FORCE = ${force};`;
var copyRows = [];
var stmt = snowflake.createStatement({ sqlText: copySql });
var rs = stmt.execute();
while (rs.next()) {
    if (stmt.getColumnCount() >= 2) {
        copyRows.push({
            file: rs.getColumnValue(1),
            status: rs.getColumnValue(2),
        });
    } else {
__SQL_DIALECT_FMT_TRAILING_WHITESPACE__
            copyRows.push({ status: rs.getColumnValue(1) });
    }
}
return {
    status: "EXECUTED",
    environment: env,
    target: cfg.table,

    files: copyRows,
};
$$;
