use sql_dialect_fmt_hover::{hover_at, HoverKind, CREATE_PROCEDURE_DOCS, CREATE_TASK_DOCS};

fn hover_on(sql: &str, needle: &str) -> sql_dialect_fmt_hover::Hover {
    let offset = sql.find(needle).expect("needle exists");
    hover_at(sql, offset).expect("hover is available")
}

#[test]
fn procedure_name_hover_summarizes_signature() {
    let sql = r#"
CREATE OR REPLACE PROCEDURE normalize_events(src VARIANT, limit_rows NUMBER)
RETURNS NUMBER
LANGUAGE JAVASCRIPT
HANDLER = 'run'
AS $$
return 1;
$$;
"#;

    let hover = hover_on(sql, "normalize_events");

    assert_eq!(hover.kind, HoverKind::Procedure);
    assert_eq!(hover.title, "Stored procedure `normalize_events`");
    assert!(hover
        .body
        .contains("Arguments: `src VARIANT, limit_rows NUMBER`."));
    assert!(hover.body.contains("Returns: `NUMBER`."));
    assert!(hover.body.contains("Language: `JAVASCRIPT`."));
    assert_eq!(hover.docs_url, Some(CREATE_PROCEDURE_DOCS));
    assert_eq!(&sql[hover.range], "normalize_events");
}

#[test]
fn procedure_name_hover_summarizes_external_language_options() {
    let sql = r#"
CREATE OR REPLACE PROCEDURE train_model(src STRING)
RETURNS STRING
LANGUAGE SCALA
RUNTIME_VERSION = '2.12'
PACKAGES = ('com.snowflake:snowpark:latest', 'com.acme:ml:1.0')
IMPORTS = ('@code/jars/lib.jar')
HANDLER = 'Model.run'
TARGET_PATH = '@code/jars/model.jar'
AS $$
class Model {}
$$;
"#;

    let hover = hover_on(sql, "train_model");

    assert_eq!(hover.kind, HoverKind::Procedure);
    assert!(hover.body.contains("Language: `SCALA`."));
    assert!(hover.body.contains("Handler: `'Model.run'`."));
    assert!(hover.body.contains("Runtime: `'2.12'`."));
    assert!(hover
        .body
        .contains("Packages: `('com.snowflake:snowpark:latest', 'com.acme:ml:1.0')`."));
    assert!(hover.body.contains("Imports: `('@code/jars/lib.jar')`."));
    assert!(hover
        .body
        .contains("Target path: `'@code/jars/model.jar'`."));
}

#[test]
fn task_name_hover_summarizes_compute_schedule_and_condition() {
    let sql = r#"
CREATE TASK load_events
  WAREHOUSE = etl_wh
  SCHEDULE = 'USING CRON 0 * * * * UTC'
  WHEN SYSTEM$STREAM_HAS_DATA('raw_events')
AS
  CALL normalize_events();
"#;

    let hover = hover_on(sql, "load_events");

    assert_eq!(hover.kind, HoverKind::Task);
    assert_eq!(hover.title, "Task `load_events`");
    assert!(hover.body.contains("Compute: `etl_wh`."));
    assert!(hover
        .body
        .contains("Schedule: `'USING CRON 0 * * * * UTC'`."));
    assert!(hover
        .body
        .contains("Condition: `SYSTEM$STREAM_HAS_DATA('raw_events')`."));
    assert_eq!(hover.docs_url, Some(CREATE_TASK_DOCS));
}

#[test]
fn task_properties_have_direct_hover() {
    let sql = "CREATE TASK t WAREHOUSE = wh SCHEDULE = '5 MINUTES' AS SELECT 1;";

    let schedule = hover_on(sql, "SCHEDULE");
    assert_eq!(schedule.kind, HoverKind::Property);
    assert!(schedule.body.contains("interval strings"));

    let warehouse = hover_on(sql, "WAREHOUSE");
    assert_eq!(warehouse.kind, HoverKind::Property);
    assert!(warehouse.body.contains("Virtual warehouse"));
}

#[test]
fn procedure_properties_and_languages_have_direct_hover() {
    let sql = "CREATE PROCEDURE p() RETURNS STRING LANGUAGE PYTHON RUNTIME_VERSION = '3.12' PACKAGES = ('snowflake-snowpark-python') IMPORTS = ('@code/app.py') HANDLER = 'run' EXTERNAL_ACCESS_INTEGRATIONS = (net) SECRETS = ('cred'=my_secret) AS $$pass$$;";

    let returns = hover_on(sql, "RETURNS");
    assert_eq!(returns.kind, HoverKind::Property);
    assert!(returns.body.contains("procedure result type"));

    let python = hover_on(sql, "PYTHON");
    assert_eq!(python.kind, HoverKind::Language);
    assert!(python.body.contains("Snowpark Python"));

    let runtime = hover_on(sql, "RUNTIME_VERSION");
    assert_eq!(runtime.kind, HoverKind::Property);
    assert!(runtime.body.contains("runtime version"));

    let packages = hover_on(sql, "PACKAGES");
    assert_eq!(packages.kind, HoverKind::Property);
    assert!(packages.body.contains("runtime packages"));

    let imports = hover_on(sql, "IMPORTS");
    assert_eq!(imports.kind, HoverKind::Property);
    assert!(imports.body.contains("staged files"));

    let external = hover_on(sql, "EXTERNAL_ACCESS_INTEGRATIONS");
    assert_eq!(external.kind, HoverKind::Property);
    assert!(external.body.contains("outbound network access"));

    let secrets = hover_on(sql, "SECRETS");
    assert_eq!(secrets.kind, HoverKind::Property);
    assert!(secrets.body.contains("Snowflake secrets"));

    let scala = hover_on("CREATE PROCEDURE p() LANGUAGE SCALA AS $$x$$;", "SCALA");
    assert_eq!(scala.kind, HoverKind::Language);
    assert!(scala.body.contains("Snowpark Scala"));

    let sql_language = hover_on(
        "CREATE PROCEDURE p() LANGUAGE SQL AS BEGIN RETURN 1; END",
        "SQL",
    );
    assert_eq!(sql_language.kind, HoverKind::Language);
    assert!(sql_language.body.contains("Snowflake Scripting"));
}

#[test]
fn snowflake_types_have_hover_even_in_unicode_and_crlf_source() {
    let sql = "SELECT '長芋'::VARIANT AS payload\r\nSELECT 1::NUMBER;";

    let variant = hover_on(sql, "VARIANT");
    assert_eq!(variant.kind, HoverKind::Type);
    assert!(variant.body.contains("Semi-structured"));

    let number = hover_on(sql, "NUMBER");
    assert_eq!(number.kind, HoverKind::Type);
    assert!(number.body.contains("fixed-point"));
}

#[test]
fn broken_mid_edit_sql_does_not_panic() {
    let sql = "CREATE TASK t SCHEDULE = '5 MINUTES AS SELECT";
    let hover = hover_on(sql, "TASK");

    assert_eq!(hover.kind, HoverKind::Task);
}

#[test]
fn spec_feature_keywords_have_hover() {
    let sql = "SELECT c FROM t QUALIFY ROW_NUMBER() OVER (ORDER BY c) = 1;";
    let hover = hover_on(sql, "QUALIFY");

    assert_eq!(hover.kind, HoverKind::Feature);
    assert_eq!(hover.title, "QUALIFY");
    assert!(hover.body.contains("QUALIFY <expr>"));
    assert!(hover.docs_url.is_some());
    assert_eq!(&sql[hover.range], "QUALIFY");
}

#[test]
fn multi_word_feature_phrases_cover_the_whole_phrase() {
    let sql = "SELECT a FROM t GROUP BY a;";
    let hover = hover_on(sql, "BY");

    assert_eq!(hover.kind, HoverKind::Feature);
    assert_eq!(hover.title, "GROUP BY");
    assert_eq!(&sql[hover.range], "GROUP BY");
}

#[test]
fn longest_feature_phrase_wins() {
    let sql = "SELECT * FROM a LEFT OUTER JOIN b ON a.id = b.id;";
    let hover = hover_on(sql, "JOIN");

    assert_eq!(hover.title, "OUTER JOIN");
    assert_eq!(&sql[hover.range], "LEFT OUTER JOIN");

    let plain = hover_on("SELECT * FROM a JOIN b ON a.id = b.id;", "JOIN");
    assert_eq!(plain.title, "INNER JOIN");
}

#[test]
fn fillers_between_phrase_words_do_not_break_the_match() {
    let sql = "CREATE OR REPLACE TABLE t AS SELECT 1;";
    let hover = hover_on(sql, "TABLE");

    assert_eq!(hover.kind, HoverKind::Feature);
    assert_eq!(hover.title, "CREATE TABLE");
    assert_eq!(&sql[hover.range], "CREATE OR REPLACE TABLE");

    let negated = hover_on("SELECT a IS NOT NULL FROM t;", "NULL");
    assert_eq!(negated.title, "IS NULL");
}

#[test]
fn unparsed_coverage_is_called_out() {
    let hover = hover_on("SELECT a FROM t FETCH FIRST 5 ROWS ONLY;", "FETCH");

    assert_eq!(hover.kind, HoverKind::Feature);
    assert!(hover.body.contains("does not parse this yet"));

    let partial = hover_on("SELECT 1 WINDOW w AS (ORDER BY x);", "WINDOW");
    assert!(partial.body.contains("parses this partially"));
}

#[test]
fn function_calls_have_signature_hover() {
    let sql = "SELECT DATEADD(day, 1, event_date) FROM t;";
    let hover = hover_on(sql, "DATEADD");

    assert_eq!(hover.kind, HoverKind::Function);
    assert_eq!(hover.title, "Snowflake function `DATEADD`");
    assert!(hover.body.contains("DATEADD( <date_or_time_part>"));
    assert!(hover.body.contains("Returns `DATE | TIME | TIMESTAMP`."));
    assert_eq!(
        hover.docs_url,
        Some("https://docs.snowflake.com/en/sql-reference/functions/dateadd")
    );
    assert_eq!(&sql[hover.range], "DATEADD");
}

#[test]
fn function_names_without_a_call_do_not_match() {
    let sql = "SELECT dateadd FROM t;";
    let offset = sql.find("dateadd").expect("needle exists");

    assert_eq!(hover_at(sql, offset), None);
}

#[test]
fn parenless_context_functions_have_hover() {
    let hover = hover_on("SELECT CURRENT_TIMESTAMP;", "CURRENT_TIMESTAMP");

    assert_eq!(hover.kind, HoverKind::Function);
    assert!(hover.body.contains("session time zone"));
}

#[test]
fn qualified_function_names_have_hover() {
    let sql = "SELECT SNOWFLAKE.CORTEX.SENTIMENT(review) FROM reviews;";
    let hover = hover_on(sql, "CORTEX");

    assert_eq!(hover.kind, HoverKind::Function);
    assert_eq!(
        hover.title,
        "Snowflake function `SNOWFLAKE.CORTEX.SENTIMENT`"
    );
    assert_eq!(&sql[hover.range], "SNOWFLAKE.CORTEX.SENTIMENT");
    assert_eq!(
        hover.docs_url,
        Some("https://docs.snowflake.com/en/sql-reference/functions/sentiment-snowflake-cortex")
    );
}

#[test]
fn table_functions_are_labelled_as_such() {
    let sql = "SELECT f.value FROM t, LATERAL FLATTEN(INPUT => t.payload) f;";
    let hover = hover_on(sql, "FLATTEN");

    assert_eq!(hover.kind, HoverKind::Function);
    assert_eq!(hover.title, "Snowflake table function `FLATTEN`");
}
