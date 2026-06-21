use snow_fmt_hover::{hover_at, HoverKind, CREATE_PROCEDURE_DOCS, CREATE_TASK_DOCS};

fn hover_on(sql: &str, needle: &str) -> snow_fmt_hover::Hover {
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
    let sql = "CREATE PROCEDURE p() RETURNS STRING LANGUAGE PYTHON RUNTIME_VERSION = '3.12' HANDLER = 'run' AS $$pass$$;";

    let returns = hover_on(sql, "RETURNS");
    assert_eq!(returns.kind, HoverKind::Property);
    assert!(returns.body.contains("procedure result type"));

    let python = hover_on(sql, "PYTHON");
    assert_eq!(python.kind, HoverKind::Language);
    assert!(python.body.contains("Snowpark Python"));

    let runtime = hover_on(sql, "RUNTIME_VERSION");
    assert_eq!(runtime.kind, HoverKind::Property);
    assert!(runtime.body.contains("runtime version"));
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
