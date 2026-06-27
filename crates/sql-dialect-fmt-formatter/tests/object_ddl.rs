//! Phase 7 object DDL + access control: `CREATE SCHEMA/DATABASE/WAREHOUSE/STAGE/FILE FORMAT/
//! SEQUENCE/STREAM/TASK/DYNAMIC TABLE` and `GRANT`/`REVOKE`.
//!
//! The matrix crosses *object kind* (schema/database/warehouse/stage/file format/sequence/stream/
//! task/dynamic table) with *shape* (bare, `OR REPLACE`, `IF NOT EXISTS`, single/multi property,
//! parenthesized sub-option, `ON` source, `AFTER` predecessors, `WHEN` guard, `AS <query|dml>` body)
//! and *grant shape* (single/multi privilege, `ALL PRIVILEGES`, object types, `WITH GRANT OPTION`,
//! `GRANT OPTION FOR`, `CASCADE`/`RESTRICT`, role/user grantees). Every case is asserted to:
//!   1. parse with no errors,
//!   2. format to valid SQL (the output re-parses clean),
//!   3. be idempotent (`format(format(x)) == format(x)`), and
//!   4. preserve its meaningful tokens (formatting only changes trivia and casing).

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::tokenize;
use sql_dialect_fmt_parser::parse;
use sql_dialect_fmt_syntax::SyntaxKind;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

/// The significant-token *kind* sequence, dropping trivia and the synthesized statement terminator.
/// Formatting must never add, drop, or reorder a meaningful token, so this is invariant.
fn significant_kinds(src: &str) -> Vec<SyntaxKind> {
    tokenize(src)
        .tokens
        .iter()
        .map(|t| t.kind)
        .filter(|k| !k.is_trivia() && *k != SyntaxKind::SEMICOLON)
        .collect()
}

/// The significant-token text sequence, upper-cased — the formatter may re-case keywords and
/// re-space, but the underlying word/literal stream must be identical.
fn significant_text(src: &str) -> Vec<String> {
    tokenize(src)
        .tokens
        .iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

const CASES: &[&str] = &[
    // ---- CREATE SCHEMA / DATABASE: bare, replace, if-not-exists, with options ----
    "create schema s",
    "create or replace schema s",
    "create schema if not exists s",
    "create or replace schema if not exists analytics comment = 'core'",
    "create schema governed contact = 'analytics@example.com' classification_profile = 'pii'",
    "create database d",
    "create database if not exists d comment = 'warehouse db'",
    "create transient schema staging data_retention_time_in_days = 1",
    // ---- CREATE WAREHOUSE: property region (no commas, KEY = value pairs) ----
    "create warehouse w",
    "create warehouse w warehouse_size = 'XSMALL'",
    "create or replace warehouse wh warehouse_size = 'LARGE' auto_suspend = 60 auto_resume = true initially_suspended = true",
    "create warehouse w with warehouse_size = 'XSMALL' auto_suspend = 300 comment = 'etl'",
    // ---- CREATE SEQUENCE: =-form and the bare START WITH / INCREMENT BY form ----
    "create sequence seq",
    "create sequence seq start = 1 increment = 1",
    "create or replace sequence seq start with 100 increment by 5 noorder comment = 'ids'",
    "create sequence if not exists seq start = 1 increment = 1 order",
    // ---- CREATE FILE FORMAT (two-word kind) ----
    "create file format ff type = 'CSV'",
    "create or replace file format ff type = 'CSV' field_delimiter = ',' skip_header = 1",
    "create file format if not exists jf type = 'JSON' strip_outer_array = true",
    // ---- CREATE STAGE: parenthesized sub-option regions ----
    "create stage st",
    "create stage st url = 's3://bucket/path/'",
    "create or replace stage ext url = 's3://b/' file_format = (type = 'CSV') directory = (enable = true)",
    "create temporary stage tmp file_format = (type = 'JSON' strip_outer_array = true)",
    // ---- CREATE STREAM: ON TABLE / VIEW / STAGE sources + flags ----
    "create stream s on table t",
    "create or replace stream s on table db.sch.t append_only = true",
    "create stream s on view v show_initial_rows = true",
    "create stream if not exists s on table t comment = 'cdc'",
    "create stream s on stage st",
    // ---- CREATE TASK: WAREHOUSE / SCHEDULE / AFTER / WHEN + AS <sql> body ----
    "create task t warehouse = w schedule = '5 minutes' as select 1",
    "create or replace task t warehouse = w schedule = 'USING CRON 0 9 * * * UTC' as select current_timestamp()",
    "create task child warehouse = w after parent as insert into log select * from src",
    "create task fan after a, b, c as delete from staging where done",
    "create task guarded warehouse = w schedule = '1 minute' when system$stream_has_data('s') as merge into tgt using src on tgt.id = src.id when matched then update set tgt.v = src.v",
    "create task u warehouse = w as update t set c = 1 where id > 0",
    "create task resilient warehouse = w task_auto_retry_attempts = 3 user_task_minimum_trigger_interval_in_seconds = 60 trace_level = always as select 1",
    // ---- CREATE DYNAMIC TABLE: TARGET_LAG / WAREHOUSE + AS <query> ----
    "create dynamic table dt target_lag = '1 minute' warehouse = w as select a from t",
    "create or replace dynamic table dt target_lag = 'DOWNSTREAM' warehouse = w refresh_mode = auto as select a, b from t where a > 0",
    "create dynamic table dt (a, b) target_lag = '20 minutes' warehouse = w as select x, y from src",
    "create dynamic table dt target_lag = '1 hour' warehouse = w as with c as (select 1 as n) select n from c",
    // ---- CREATE SEMANTIC VIEW: semantic model clauses and AI instruction clauses ----
    "create semantic view sv tables(orders as mart.orders primary key(order_id), customers as mart.customers primary key(customer_id)) relationships(order_customer as orders(customer_id) references customers) facts(public orders.net_amount as net_amount) dimensions(public customers.region as region) metrics(public orders.revenue as sum(orders.net_amount)) comment = 'semantic model' ai_sql_generation 'Use revenue for sales questions.' ai_question_categorization 'Classify revenue questions.' ai_verified_queries(top_revenue as(question 'Top revenue?' verified_at 1767225600 onboarding_question true verified_by 'analyst@example.com' sql 'SELECT 1')) with tag(governance.owner = 'analytics') copy grants",
    // ---- CREATE MASKING / ROW ACCESS POLICY: inline policy signature/body, clean parse ----
    "create masking policy mask_email as (val STRING) returns STRING -> case when current_role() in ('ANALYST') then val else '***' end",
    "create or replace masking policy mask_email as (val STRING) returns STRING -> val comment = 'mask'",
    "create row access policy region_filter as (region STRING) returns BOOLEAN -> region = current_region()",
    "create or alter row access policy region_filter as (id NUMBER) returns BOOLEAN -> true",
    // ---- CREATE TAG: property region with allowed values and propagation ----
    "create tag cost_center allowed_values 'sales', 'engineering' comment = 'owner'",
    "create or alter tag classification propagate = ON_DEPENDENCY comment = 'classification'",
    // ---- GRANT: single / multi privileges, object types, WITH GRANT OPTION ----
    "grant select on table t to role r",
    "grant select, insert, update on table t to role analyst",
    "grant all privileges on schema s to role r",
    "grant usage on database d to role r",
    "grant operate on warehouse wh to role ops with grant option",
    "grant select on table db.sch.t to role r with grant option",
    "grant usage on schema mydb.myschema to role reporter",
    "grant role analyst to role manager",
    "grant ownership on table t to role r",
    // ---- REVOKE: GRANT OPTION FOR, CASCADE / RESTRICT, role/user ----
    "revoke select on table t from role r",
    "revoke select, insert on table t from role r",
    "revoke all privileges on schema s from role r",
    "revoke grant option for operate on warehouse wh from role ops cascade",
    "revoke usage on database d from role r restrict",
    "revoke operate on warehouse wh from role ops",
    // ---- multi-statement files mixing the new DDL with queries ----
    "create schema s; grant usage on schema s to role r; select 1",
    "create stream s on table t; create task t warehouse = w schedule = '1 minute' as select 1",
];

#[test]
fn all_cases_parse_clean() {
    for sql in CASES {
        let errors = parse(sql).errors().to_vec();
        assert!(errors.is_empty(), "parse errors for {sql:?}: {errors:?}");
    }
}

#[test]
fn all_cases_round_trip_losslessly() {
    for sql in CASES {
        assert_eq!(
            parse(sql).syntax().to_string(),
            *sql,
            "lossless round-trip failed for {sql:?}"
        );
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
fn formatting_preserves_token_kinds() {
    for sql in CASES {
        let formatted = fmt(sql);
        assert_eq!(
            significant_kinds(sql),
            significant_kinds(&formatted),
            "token-kind sequence changed:\n{sql}\n---\n{formatted}"
        );
    }
}

#[test]
fn formatting_preserves_token_text() {
    for sql in CASES {
        let formatted = fmt(sql);
        assert_eq!(
            significant_text(sql),
            significant_text(&formatted),
            "token text changed:\n{sql}\n---\n{formatted}"
        );
    }
}

// ---- exact-string goldens: pin the opinionated layout ----

#[test]
fn create_schema_stays_on_one_line() {
    assert_eq!(fmt("create schema s"), "CREATE SCHEMA s;\n");
}

#[test]
fn create_object_stacks_each_property() {
    assert_eq!(
        fmt("create warehouse w warehouse_size = 'XSMALL' auto_suspend = 60 auto_resume = true"),
        "CREATE WAREHOUSE w\n    \
           WAREHOUSE_SIZE = 'XSMALL'\n    \
           AUTO_SUSPEND = 60\n    \
           AUTO_RESUME = TRUE;\n",
    );
}

#[test]
fn newer_object_options_upcase_in_key_position() {
    assert_eq!(
        fmt("create task resilient warehouse = w task_auto_retry_attempts = 3 user_task_minimum_trigger_interval_in_seconds = 60 trace_level = always as select 1"),
        "CREATE TASK resilient\n    \
           WAREHOUSE = w\n    \
           TASK_AUTO_RETRY_ATTEMPTS = 3\n    \
           USER_TASK_MINIMUM_TRIGGER_INTERVAL_IN_SECONDS = 60\n    \
           TRACE_LEVEL = always\n    \
           AS\n    \
           SELECT 1;\n",
    );
}

#[test]
fn preview_object_options_upcase_in_key_position() {
    assert_eq!(
        fmt("create warehouse preview_wh min_nodes = 1 max_nodes = 3 instance_family = CPU_X64_XS auto_suspend_secs = 60 network_policy = np password_policy = pp session_policy = sp authentication_policy = ap default_warehouse = wh"),
        "CREATE WAREHOUSE preview_wh\n    \
           MIN_NODES = 1\n    \
           MAX_NODES = 3\n    \
           INSTANCE_FAMILY = CPU_X64_XS\n    \
           AUTO_SUSPEND_SECS = 60\n    \
           NETWORK_POLICY = np\n    \
           PASSWORD_POLICY = pp\n    \
           SESSION_POLICY = sp\n    \
           AUTHENTICATION_POLICY = ap\n    \
           DEFAULT_WAREHOUSE = wh;\n",
    );

    assert_eq!(
        fmt("create schema governed aggregation_policy = agg join_policy = jp projection_policy = pp data_metric_function = dm data_metric_schedule = '5 minutes'"),
        "CREATE SCHEMA governed\n    \
           AGGREGATION_POLICY = agg\n    \
           JOIN_POLICY = jp\n    \
           PROJECTION_POLICY = pp\n    \
           DATA_METRIC_FUNCTION = dm\n    \
           DATA_METRIC_SCHEDULE = '5 minutes';\n",
    );
}

#[test]
fn create_task_lays_out_body_structurally() {
    assert_eq!(
        fmt("create task t warehouse = w schedule = '5 minutes' as select 1"),
        "CREATE TASK t\n    \
           WAREHOUSE = w\n    \
           SCHEDULE = '5 minutes'\n    \
           AS\n    \
           SELECT 1;\n",
    );
}

#[test]
fn create_dynamic_table_keeps_query_structural() {
    assert_eq!(
        fmt("create dynamic table dt target_lag = '1 minute' warehouse = w as select a from t"),
        "CREATE DYNAMIC TABLE dt\n    \
           TARGET_LAG = '1 minute'\n    \
           WAREHOUSE = w\n    \
           AS\n    \
           SELECT a\n    \
           FROM t;\n",
    );
}

#[test]
fn create_semantic_view_stacks_model_clauses() {
    assert_eq!(
        fmt("create semantic view sv tables(orders as mart.orders primary key(order_id), customers as mart.customers primary key(customer_id)) metrics(public orders.revenue as sum(orders.net_amount)) ai_sql_generation 'Use revenue.' copy grants"),
        "CREATE SEMANTIC VIEW sv\n    \
           TABLES (\n        \
           orders AS mart.orders PRIMARY KEY (order_id),\n        \
           customers AS mart.customers PRIMARY KEY (customer_id)\n    \
           )\n    \
           METRICS (\n        \
           PUBLIC orders.revenue AS sum(orders.net_amount)\n    \
           )\n    \
           AI_SQL_GENERATION 'Use revenue.'\n    \
           COPY GRANTS;\n",
    );
}

#[test]
fn create_stream_source_on_its_own_line() {
    assert_eq!(
        fmt("create stream s on table t append_only = true"),
        "CREATE STREAM s\n    \
           ON TABLE t\n    \
           APPEND_ONLY = TRUE;\n",
    );
}

#[test]
fn create_policy_stays_inline_but_upcases_policy_words() {
    assert_eq!(
        fmt("create masking policy mask_email as (val STRING) returns STRING -> val comment = 'mask'"),
        "CREATE MASKING POLICY mask_email AS (val STRING) RETURNS STRING -> val COMMENT = 'mask';\n",
    );
}

#[test]
fn create_tag_stacks_allowed_values_and_comment() {
    assert_eq!(
        fmt("create tag cost_center allowed_values 'sales', 'engineering' comment = 'owner'"),
        "CREATE TAG cost_center\n    \
           ALLOWED_VALUES 'sales', 'engineering'\n    \
           COMMENT = 'owner';\n",
    );
}

#[test]
fn grant_stacks_target_and_grantee() {
    assert_eq!(
        fmt("grant select, insert on table t to role r with grant option"),
        "GRANT SELECT, INSERT\n    \
           ON TABLE t\n    \
           TO ROLE r WITH GRANT OPTION;\n",
    );
}

#[test]
fn grant_all_privileges_upper_cases_the_phrase() {
    assert_eq!(
        fmt("grant all privileges on schema s to role r"),
        "GRANT ALL PRIVILEGES\n    \
           ON SCHEMA s\n    \
           TO ROLE r;\n",
    );
}

#[test]
fn revoke_grant_option_for_with_cascade() {
    assert_eq!(
        fmt("revoke grant option for operate on warehouse wh from role ops cascade"),
        "REVOKE GRANT OPTION FOR operate\n    \
           ON WAREHOUSE wh\n    \
           FROM ROLE ops CASCADE;\n",
    );
}

#[test]
fn keyword_casing_can_be_disabled() {
    // With casing off, every keyword (reserved and contextual) keeps its source spelling — including
    // the synthesized `AS` and the contextual `TO`/`ROLE`/object-kind words.
    let opts = FormatOptions::default().with_uppercase_keywords(false);
    assert_eq!(
        format("grant select on table t to role r", &opts),
        "grant select\n    on table t\n    to role r;\n",
    );
    assert_eq!(
        format("create task t warehouse = w as select 1", &opts),
        "create task t\n    warehouse = w\n    as\n    select 1;\n",
    );
}
