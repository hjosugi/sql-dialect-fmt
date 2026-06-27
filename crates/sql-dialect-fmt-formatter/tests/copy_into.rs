//! Exhaustive COPY INTO (load and unload) formatting coverage.
//!
//! Every case below is asserted to (1) parse with no errors, (2) format to valid SQL (reparses
//! clean), (3) be idempotent (`format(format(x)) == format(x)`), and (4) preserve its meaningful
//! tokens (formatting only changes trivia and keyword casing). The matrix crosses *direction*
//! (load `COPY INTO <table>` vs unload `COPY INTO <location>`) with *shape*: stage forms
//! (`@stage`, `@~`, `@%table`, namespaced, with/without trailing path), external locations
//! (`'gcs://…'`, `'s3://…'`, `'azure://…'`), query sources/targets, transform `SELECT`s reading
//! `FROM @stage`, column lists, and the option region (FILE_FORMAT nested, PATTERN, FILES,
//! ON_ERROR, VALIDATION_MODE, MATCH_BY_COLUMN_NAME, PARTITION BY, HEADER, SINGLE, OVERWRITE, …).

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::{tokenize, SyntaxKind};
use sql_dialect_fmt_parser::parse;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

/// The signature a faithful formatter must preserve: meaningful tokens, upper-cased, with the
/// synthesized `;` dropped.
fn signature(sql: &str) -> Vec<String> {
    tokenize(sql)
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

const CASES: &[&str] = &[
    // ---- load: bare stage sources, every stage form ----
    "copy into t from @s",
    "copy into t from @s/p",
    "copy into t from @s/path/to/files/",
    "copy into t from @~/staged/data",
    "copy into t from @%mytable",
    "copy into t from @%mytable/2026/06/",
    "copy into db.sch.t from @db.sch.stage/orders/",
    "copy into t from @\"My Stage\"/p",
    // ---- load: column list ----
    "copy into t (a) from @s",
    "copy into t (a, b, c) from @s",
    "copy into db.sch.t (col1, col2) from @db.sch.stage/path/",
    // ---- load: FILE_FORMAT (inline / nested options / FORMAT_NAME) ----
    "copy into t from @s file_format = (type = csv)",
    "copy into t from @s file_format = (type = csv skip_header = 1 field_delimiter = ',')",
    "copy into t from @s file_format = (type = json strip_outer_array = true)",
    "copy into t from @s file_format = (format_name = my_db.my_schema.my_ff)",
    "copy into t from @s file_format = (type = csv null_if = ('', 'null', '\\n'))",
    // ---- load: PATTERN / FILES ----
    "copy into t from @s pattern = '.*[.]csv'",
    "copy into t from @s files = ('a.csv', 'b.csv', 'c.csv')",
    "copy into t from @s files = ('only.parquet') file_format = (type = parquet)",
    // ---- load: copy options (each on its own line) ----
    "copy into t from @s on_error = continue",
    "copy into t from @s on_error = 'skip_file_5%'",
    "copy into t from @s on_error = abort_statement purge = true force = false",
    "copy into t from @s size_limit = 5368709120 return_failed_only = false",
    "copy into t from @s match_by_column_name = case_insensitive",
    "copy into t from @s validation_mode = return_errors",
    "copy into t from @s file_format = (type = json) on_error = continue purge = false force = false return_failed_only = false",
    // ---- load: transform query source reading FROM @stage ----
    "copy into t (a, b, c) from (select $1, $2, $3 from @s) file_format = (type = csv)",
    "copy into t from (select $1:a::string from @s (file_format => my_ff))",
    "copy into t (id, payload) from (select $1:id::string, $1 from @raw.stage/events/ (file_format => raw.ff_json)) pattern = '.*[.]jsonl'",
    "copy into t from (select metadata$filename, metadata$file_row_number, $1 from @s)",
    // ---- unload: stage targets, every form ----
    "copy into @s from t",
    "copy into @s/out/ from t",
    "copy into @~/exports/ from t",
    "copy into @%mytable/dump/ from t",
    "copy into @mart.export_stage/daily/ from t",
    // ---- unload: external locations ----
    "copy into 'gcs://bucket/path/' from t file_format = (type = parquet)",
    "copy into 's3://bucket/prefix/' from (select * from t) overwrite = true",
    "copy into 'azure://acct.blob.core.windows.net/cont/path/' from t single = true",
    // ---- unload: query source ----
    "copy into @s from (select * from t)",
    "copy into @s from (select a, b from t where a > 0)",
    "copy into @s from (with c as (select 1 n) select n from c)",
    // ---- unload: PARTITION BY + options + HEADER/SINGLE/OVERWRITE/MAX_FILE_SIZE ----
    "copy into @s from t partition by (dt) header = true",
    "copy into @s from (select a, dt from t) partition by ('d=' || to_varchar(dt)) file_format = (type = csv) header = true",
    "copy into @s/out/ from t header = true single = true max_file_size = 16777216",
    "copy into @s from t overwrite = true single = false include_query_id = true detailed_output = true",
    "copy into @s/out/ from t file_format = (type = csv compression = gzip field_delimiter = '|' record_delimiter = '\\n' field_optionally_enclosed_by = '\"' null_if = ('', 'null'))",
    // ---- canonical-cased, realistic end-to-end ----
    "COPY INTO RAW.ORDERS FROM @RAW.STAGE/orders/ FILE_FORMAT = (TYPE = JSON) ON_ERROR = SKIP_FILE",
    "COPY INTO @MART.EXPORT/out/ FROM (SELECT * FROM MART.T) PARTITION BY (DT) FILE_FORMAT = (TYPE = CSV) HEADER = TRUE OVERWRITE = TRUE",
];

#[test]
fn all_cases_parse_clean() {
    for sql in CASES {
        let errors = parse(sql).errors().to_vec();
        assert!(errors.is_empty(), "parse errors for {sql:?}: {errors:?}");
    }
}

#[test]
fn formatting_is_idempotent() {
    for sql in CASES {
        let once = fmt(sql);
        assert_eq!(once, fmt(&once), "not idempotent:\n{sql}\n---\n{once}");
    }
}

#[test]
fn formatted_output_is_valid_sql() {
    for sql in CASES {
        let formatted = fmt(sql);
        let errors = parse(&formatted).errors().to_vec();
        assert!(
            errors.is_empty(),
            "formatted output is invalid for {sql:?}: {errors:?}\n---\n{formatted}"
        );
    }
}

#[test]
fn formatting_preserves_tokens() {
    for sql in CASES {
        let formatted = fmt(sql);
        assert_eq!(
            signature(sql),
            signature(&formatted),
            "token sequence changed:\n{sql}\n---\n{formatted}"
        );
    }
}

// ---- a few exact-string goldens pinning the canonical layout (load + unload) ----

#[test]
fn golden_load_with_file_format_and_options() {
    assert_eq!(
        fmt("copy into raw.orders from @raw.stage/orders/ FILE_FORMAT = (TYPE = JSON) ON_ERROR = CONTINUE"),
        "COPY INTO raw.orders\n\
         FROM @raw.stage/orders/\n\
         FILE_FORMAT = (TYPE = JSON)\n\
         ON_ERROR = CONTINUE;\n"
    );
}

#[test]
fn golden_unload_with_partition_and_header() {
    assert_eq!(
        fmt("copy into @mart.stage/out/ from (select * from t) PARTITION BY (dt) FILE_FORMAT = (TYPE = CSV) HEADER = TRUE"),
        "COPY INTO @mart.stage/out/\n\
         FROM (\n    \
             SELECT *\n    \
             FROM t\n\
         )\n\
         PARTITION BY (dt)\n\
         FILE_FORMAT = (TYPE = CSV)\n\
         HEADER = TRUE;\n"
    );
}

#[test]
fn golden_load_transform_reading_from_stage() {
    assert_eq!(
        fmt("copy into t (a, b) from (select $1, $2 from @s) FILE_FORMAT = (TYPE = CSV)"),
        "COPY INTO t (a, b)\n\
         FROM (\n    \
             SELECT $1, $2\n    \
             FROM @s\n\
         )\n\
         FILE_FORMAT = (TYPE = CSV);\n"
    );
}
