//! Phase 7 object DDL + access control parsing: `CREATE SCHEMA/DATABASE/WAREHOUSE/STAGE/FILE FORMAT/
//! SEQUENCE/STREAM/TASK/DYNAMIC TABLE` and `GRANT`/`REVOKE`.
//!
//! Every accepted form must parse diagnostic-free, round-trip byte-for-byte, and expose the
//! structural nodes the formatter relies on (OBJECT_PROPERTY, STREAM_SOURCE, TASK_AFTER, the
//! structural `AS` body; PRIV_LIST / GRANT_TARGET / GRANTEE). Broken/partial input must still
//! round-trip and never panic.

use sql_dialect_fmt_parser::SyntaxKind;
use sql_dialect_fmt_test_support::parser::{
    assert_has_node_kind, assert_parse_clean as clean, assert_parse_recovers as recovers,
};

#[test]
fn create_schema_database_warehouse_parse_clean() {
    clean("CREATE SCHEMA s");
    clean("CREATE OR REPLACE SCHEMA s");
    clean("CREATE SCHEMA IF NOT EXISTS s");
    clean("CREATE OR REPLACE SCHEMA IF NOT EXISTS analytics COMMENT = 'core'");
    clean("CREATE DATABASE d");
    clean("CREATE TRANSIENT SCHEMA staging DATA_RETENTION_TIME_IN_DAYS = 1");
    clean("CREATE WAREHOUSE w");
    clean("CREATE OR REPLACE WAREHOUSE wh WAREHOUSE_SIZE = 'LARGE' AUTO_SUSPEND = 60 AUTO_RESUME = TRUE");
    clean("CREATE WAREHOUSE w WITH WAREHOUSE_SIZE = 'XSMALL' AUTO_SUSPEND = 300");
}

#[test]
fn create_sequence_both_property_forms_parse_clean() {
    clean("CREATE SEQUENCE seq");
    clean("CREATE SEQUENCE seq START = 1 INCREMENT = 1");
    clean("CREATE OR REPLACE SEQUENCE seq START WITH 100 INCREMENT BY 5 NOORDER COMMENT = 'ids'");
    clean("CREATE SEQUENCE IF NOT EXISTS seq START = 1 INCREMENT = 1 ORDER");
}

#[test]
fn create_file_format_and_stage_with_sub_options_parse_clean() {
    clean("CREATE FILE FORMAT ff TYPE = 'CSV'");
    clean("CREATE OR REPLACE FILE FORMAT ff TYPE = 'CSV' FIELD_DELIMITER = ',' SKIP_HEADER = 1");
    clean("CREATE STAGE st");
    clean("CREATE STAGE st URL = 's3://bucket/path/'");
    clean("CREATE OR REPLACE STAGE ext URL = 's3://b/' FILE_FORMAT = (TYPE = 'CSV') DIRECTORY = (ENABLE = TRUE)");
    clean("CREATE TEMPORARY STAGE tmp FILE_FORMAT = (TYPE = 'JSON' STRIP_OUTER_ARRAY = TRUE)");
}

#[test]
fn object_property_nodes_are_emitted() {
    let sql = "CREATE WAREHOUSE w WAREHOUSE_SIZE = 'XSMALL' AUTO_SUSPEND = 60";
    clean(sql);
    assert_has_node_kind(sql, SyntaxKind::CREATE_STMT);
    assert_has_node_kind(sql, SyntaxKind::OBJECT_PROPERTY);
}

#[test]
fn create_stream_sources_parse_and_expose_stream_source() {
    for sql in [
        "CREATE STREAM s ON TABLE t",
        "CREATE OR REPLACE STREAM s ON TABLE db.sch.t APPEND_ONLY = TRUE",
        "CREATE STREAM s ON VIEW v SHOW_INITIAL_ROWS = TRUE",
        "CREATE STREAM IF NOT EXISTS s ON TABLE t COMMENT = 'cdc'",
        "CREATE STREAM s ON STAGE st",
    ] {
        clean(sql);
        assert_has_node_kind(sql, SyntaxKind::STREAM_SOURCE);
    }
}

#[test]
fn create_task_parses_properties_after_list_and_structural_body() {
    let sql = "CREATE TASK t WAREHOUSE = w SCHEDULE = '5 minutes' AS SELECT 1";
    clean(sql);
    assert_has_node_kind(sql, SyntaxKind::OBJECT_PROPERTY);
    assert_has_node_kind(sql, SyntaxKind::SELECT_STMT);

    let after =
        "CREATE TASK child WAREHOUSE = w AFTER a, b, c AS INSERT INTO log SELECT * FROM src";
    clean(after);
    assert_has_node_kind(after, SyntaxKind::TASK_AFTER);
    assert_has_node_kind(after, SyntaxKind::INSERT_STMT);

    // WHEN guard + a DML body that is itself structured (MERGE).
    let guarded = "CREATE TASK g WAREHOUSE = w SCHEDULE = '1 minute' WHEN c AS UPDATE t SET v = 1 WHERE id > 0";
    clean(guarded);
    assert_has_node_kind(guarded, SyntaxKind::UPDATE_STMT);
}

#[test]
fn create_dynamic_table_keeps_query_body_structural() {
    let sql = "CREATE DYNAMIC TABLE dt TARGET_LAG = '1 minute' WAREHOUSE = w AS SELECT a FROM t";
    clean(sql);
    assert_has_node_kind(sql, SyntaxKind::OBJECT_PROPERTY);
    assert_has_node_kind(sql, SyntaxKind::SELECT_STMT);
    assert_has_node_kind(sql, SyntaxKind::FROM_CLAUSE);

    let cte = "CREATE DYNAMIC TABLE dt TARGET_LAG = '1 hour' WAREHOUSE = w AS WITH c AS (SELECT 1 AS n) SELECT n FROM c";
    clean(cte);
    assert_has_node_kind(cte, SyntaxKind::WITH_QUERY);

    let cols = "CREATE DYNAMIC TABLE dt (a, b) TARGET_LAG = '20 minutes' WAREHOUSE = w AS SELECT x, y FROM src";
    clean(cols);
    assert_has_node_kind(cols, SyntaxKind::COLUMN_DEF_LIST);
}

#[test]
fn create_semantic_view_parses_clauses_and_items() {
    let sql = "CREATE OR REPLACE SEMANTIC VIEW sv TABLES(orders AS mart.orders PRIMARY KEY(order_id)) RELATIONSHIPS(order_customer AS orders(customer_id) REFERENCES customers) FACTS(PUBLIC orders.net_amount AS net_amount) DIMENSIONS(PUBLIC orders.order_date AS order_date) METRICS(PUBLIC orders.revenue AS SUM(orders.net_amount)) COMMENT = 'semantic model' AI_SQL_GENERATION 'Use revenue for sales questions.' AI_QUESTION_CATEGORIZATION 'Classify revenue questions.' AI_VERIFIED_QUERIES(top_revenue AS(QUESTION 'Top revenue?' VERIFIED_AT 1767225600 ONBOARDING_QUESTION TRUE VERIFIED_BY 'analyst@example.com' SQL 'SELECT 1')) WITH TAG(governance.owner = 'analytics') COPY GRANTS";
    clean(sql);
    assert_has_node_kind(sql, SyntaxKind::CREATE_STMT);
    assert_has_node_kind(sql, SyntaxKind::SEMANTIC_VIEW_CLAUSE);
    assert_has_node_kind(sql, SyntaxKind::SEMANTIC_VIEW_ITEM);
    assert_has_node_kind(sql, SyntaxKind::OBJECT_PROPERTY);
}

#[test]
fn create_policy_and_tag_shapes_parse_clean() {
    for sql in [
        "CREATE MASKING POLICY mask_email AS (val STRING) RETURNS STRING -> CASE WHEN CURRENT_ROLE() IN ('ANALYST') THEN val ELSE '***' END",
        "CREATE OR REPLACE MASKING POLICY mask_email AS (val STRING) RETURNS STRING -> val COMMENT = 'mask'",
        "CREATE ROW ACCESS POLICY region_filter AS (region STRING) RETURNS BOOLEAN -> region = CURRENT_REGION()",
        "CREATE OR ALTER ROW ACCESS POLICY region_filter AS (id NUMBER) RETURNS BOOLEAN -> TRUE",
        "CREATE TAG cost_center ALLOWED_VALUES 'sales', 'engineering' COMMENT = 'owner'",
        "CREATE OR ALTER TAG classification PROPAGATE = ON_DEPENDENCY COMMENT = 'classification'",
    ] {
        clean(sql);
        assert_has_node_kind(sql, SyntaxKind::CREATE_STMT);
    }

    assert_has_node_kind(
        "CREATE TAG cost_center ALLOWED_VALUES 'sales', 'engineering' COMMENT = 'owner'",
        SyntaxKind::OBJECT_PROPERTY,
    );
}

#[test]
fn grant_shapes_parse_and_expose_grant_nodes() {
    for sql in [
        "GRANT SELECT ON TABLE t TO ROLE r",
        "GRANT SELECT, INSERT, UPDATE ON TABLE t TO ROLE analyst",
        "GRANT ALL PRIVILEGES ON SCHEMA s TO ROLE r",
        "GRANT USAGE ON DATABASE d TO ROLE r",
        "GRANT OPERATE ON WAREHOUSE wh TO ROLE ops WITH GRANT OPTION",
        "GRANT SELECT ON TABLE db.sch.t TO ROLE r WITH GRANT OPTION",
        "GRANT OWNERSHIP ON TABLE t TO ROLE r",
    ] {
        clean(sql);
        assert_has_node_kind(sql, SyntaxKind::GRANT_STMT);
        assert_has_node_kind(sql, SyntaxKind::PRIV_LIST);
        assert_has_node_kind(sql, SyntaxKind::GRANT_TARGET);
        assert_has_node_kind(sql, SyntaxKind::GRANTEE);
    }
}

#[test]
fn revoke_shapes_parse_and_expose_revoke_nodes() {
    for sql in [
        "REVOKE SELECT ON TABLE t FROM ROLE r",
        "REVOKE SELECT, INSERT ON TABLE t FROM ROLE r",
        "REVOKE ALL PRIVILEGES ON SCHEMA s FROM ROLE r",
        "REVOKE GRANT OPTION FOR OPERATE ON WAREHOUSE wh FROM ROLE ops CASCADE",
        "REVOKE USAGE ON DATABASE d FROM ROLE r RESTRICT",
    ] {
        clean(sql);
        assert_has_node_kind(sql, SyntaxKind::REVOKE_STMT);
        assert_has_node_kind(sql, SyntaxKind::PRIV_LIST);
        assert_has_node_kind(sql, SyntaxKind::GRANT_TARGET);
        assert_has_node_kind(sql, SyntaxKind::GRANTEE);
    }
}

#[test]
fn object_kind_words_are_still_usable_as_identifiers() {
    // None of the contextual object/grant words (schema, stage, stream, role, …) are reserved, so
    // they remain valid column/table names.
    clean("SELECT schema, stage, stream, dynamic, role, cascade FROM t");
    clean("SELECT t.sequence FROM warehouses t");
    clean("SELECT file, format FROM stage_table");
}

#[test]
fn case_insensitive_keywords_and_lowercase_spelling() {
    clean("create or replace task t warehouse = w schedule = '5 minutes' as select 1");
    clean("Grant Select On Table t To Role r With Grant Option");
    clean("ReVoKe Select On Table t From Role r");
}

#[test]
fn broken_and_partial_input_round_trips_without_panic() {
    // Must stay lossless and never panic, even when incomplete.
    for s in [
        "CREATE TASK",
        "CREATE TASK t WAREHOUSE =",
        "CREATE STREAM s ON",
        "CREATE STREAM s ON TABLE",
        "CREATE SEQUENCE seq START =",
        "CREATE FILE FORMAT",
        "CREATE STAGE st FILE_FORMAT = (",
        "GRANT",
        "GRANT SELECT ON",
        "GRANT SELECT ON TABLE t TO",
        "REVOKE GRANT OPTION FOR",
        "CREATE DYNAMIC TABLE dt TARGET_LAG = '1 minute' WAREHOUSE = w AS",
        "CREATE MASKING POLICY p AS (v STRING) RETURNS STRING ->",
        "CREATE TAG t ALLOWED_VALUES 'a',",
    ] {
        // round-trip (tolerating diagnostics); the helper panics if the tree loses bytes.
        recovers(s);
    }
}
