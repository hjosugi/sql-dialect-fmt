# Snowflake Formatter / Syntax Highlighter Complex Test Suite

Generated for formatter and syntax highlighter quality testing.

## Contents

- `cases/case_XXX_*/input.sql`: intentionally compact / hard-to-read SQL input.
- `cases/case_XXX_*/expected.sql`: formatted expected output.
- `flat/`: same files flattened as `input_XXX_*.sql` and `expected_XXX_*.sql`.
- `all/input_all.sql`: all input cases concatenated.
- `all/expected_all.sql`: all expected cases concatenated.
- `manifest.json`: metadata, feature tags, and SHA-256 checksums.

## Design goals

- Snowflake-specific syntax coverage, not generic ANSI SQL only.
- Deep nested CTEs, `LATERAL FLATTEN`, semi-structured `VARIANT` paths, `QUALIFY`, windows, and `MERGE` branches.
- Stored procedures in Snowflake Scripting, JavaScript, and Python Snowpark.
- DDL / DML / DCL-adjacent cases: policies, tags, dynamic tables, streams, tasks, alerts, pipes, stages, time travel, semantic views.
- Multilingual strings and quoted/unquoted identifier traps.

## Important note

These files are intended as parser/formatter/highlighter fixtures. They were authored against Snowflake documentation patterns, but they are not executed here against a live Snowflake account. Environment-specific objects such as warehouses, integrations, policies, stages, and roles must exist before execution.

## Case index

| Case | Slug | Title | Features |
|---|---|---|---|
| case_001 | deep_json_lateral_flatten | deep semi-structured SELECT with nested LATERAL FLATTEN and QUALIFY | WITH CTE, VARIANT path, LATERAL FLATTEN, nested CASE, window, QUALIFY, TABLESAMPLE |
| case_002 | recursive_org_rollup | recursive CTE with cycle guard, arrays, rollups, and nested windows | WITH RECURSIVE, ARRAY, cycle guard, ROLLUP, window, GROUPING |
| case_003 | match_recognize_session_funnel | MATCH_RECOGNIZE funnel pattern with nested input and post-filtering | MATCH_RECOGNIZE, PATTERN, MEASURES, DEFINE, QUALIFY, window |
| case_004 | pivot_unpivot_grouping_sets | PIVOT, UNPIVOT, and GROUPING SETS in one report | PIVOT, UNPIVOT, GROUPING SETS, WHERE filter, GROUPING |
| case_005 | merge_nested_source | MERGE with nested source CTE, lateral flatten, deletes, updates, and inserts | MERGE, nested CTE, LATERAL FLATTEN, DELETE branch, UPDATE, INSERT |
| case_006 | multi_table_insert_first | multi-table INSERT FIRST with conditional routing and nested SELECT | INSERT FIRST, conditional INSERT, nested SELECT, CASE, QUALIFY |
| case_007 | copy_into_transform_metadata | COPY INTO table with nested SELECT transformations and metadata columns | COPY INTO table, FILE_FORMAT, METADATA$ columns, PATTERN, ON_ERROR |
| case_008 | copy_unload_partitioned | COPY INTO location unload with partitioning and complex SELECT | COPY INTO location, PARTITION BY, FILE_FORMAT, HEADER, OVERWRITE |
| case_009 | security_policies_table | masking policy, row access policy, tags, and protected table DDL | CREATE MASKING POLICY, CREATE ROW ACCESS POLICY, CREATE TABLE, TAG, ROW ACCESS POLICY |
| case_010 | dynamic_table_nested_refresh | dynamic table with nested query, target lag, cluster key, and frozen region | CREATE DYNAMIC TABLE, TARGET_LAG, REFRESH_MODE, INITIALIZE, FROZEN WHERE, nested CTE |
| case_011 | task_graph_streams_finalizer | streams and task graph with WHEN conditions and finalizer task | CREATE STREAM, CREATE TASK, AFTER, WHEN, FINALIZE, SYSTEM$STREAM_HAS_DATA |
| case_012 | sql_scripting_nested_procedure | Snowflake Scripting procedure with nested loops, transactions, cursors, and exceptions | CREATE PROCEDURE LANGUAGE SQL, DECLARE, CURSOR, FOR loop, EXCEPTION, EXECUTE IMMEDIATE |
| case_013 | javascript_procedure_dynamic_sql | JavaScript stored procedure with dynamic SQL, template strings, binds, and result handling | CREATE PROCEDURE LANGUAGE JAVASCRIPT, template string, binds, snowflake.createStatement, regex |
| case_014 | python_snowpark_procedure | Python Snowpark procedure with multilingual text profiling and nested DataFrame SQL | CREATE PROCEDURE LANGUAGE PYTHON, Snowpark, packages, handler, Unicode |
| case_015 | anonymous_procedure_call_with | anonymous procedure with WITH ... AS PROCEDURE and nested SQL block | CALL WITH anonymous procedure, LANGUAGE SQL, DECLARE, RETURN VARIANT |
| case_016 | udf_udtf_mixed_languages | SQL, JavaScript, and Python UDF/UDTF definitions in one file | CREATE FUNCTION, SQL UDTF, JavaScript UDF, Python UDF, LATERAL FLATTEN |
| case_017 | snowpipe_create_pipe | Snowpipe CREATE PIPE with auto ingest and complex COPY body | CREATE PIPE, AUTO_INGEST, COPY INTO, FILE_FORMAT, metadata columns |
| case_018 | alert_exists_notification | CREATE ALERT with EXISTS condition and notification procedure call | CREATE ALERT, EXISTS, SCHEDULE, CALL, nested SELECT |
| case_019 | materialized_view_search_optimization | materialized view plus search optimization and clustering-heavy query | CREATE MATERIALIZED VIEW, ALTER TABLE, SEARCH OPTIMIZATION, window, QUALIFY |
| case_020 | time_travel_clone_swap | time travel, clone, swap, result scan, and rollback-friendly repair SQL | CLONE, AT, BEFORE, SWAP WITH, RESULT_SCAN, LAST_QUERY_ID |
| case_021 | secure_view_policy_query | secure view with nested CTEs, lateral flatten, masking-aware columns, and comments | CREATE SECURE VIEW, nested CTE, LATERAL FLATTEN, QUALIFY, COPY GRANTS |
| case_022 | asof_join_resample_timeseries | ASOF JOIN, RESAMPLE, window interpolation, and device time-series cleanup | ASOF JOIN, RESAMPLE, MATCH_CONDITION, INTERPOLATE_FFILL, window |
| case_023 | semantic_view_complex | CREATE SEMANTIC VIEW with logical tables, relationships, facts, dimensions, metrics, and verified queries | CREATE SEMANTIC VIEW, TABLES, RELATIONSHIPS, FACTS, DIMENSIONS, METRICS, AI_VERIFIED_QUERIES |
| case_024 | tag_classification_alter | tags, column comments, masking assignment, and table alterations | CREATE TAG, ALTER TABLE, MODIFY COLUMN, SET TAG, UNSET TAG |
| case_025 | stage_file_ops_directory | stage directory operations, LIST/REMOVE, and RESULT_SCAN inspection | CREATE STAGE, DIRECTORY, LIST, REMOVE, RESULT_SCAN, PATTERN |
| case_026 | transaction_result_scan_query_history | transaction block, query history, RESULT_SCAN, and session variables | SET, BEGIN, COMMIT, RESULT_SCAN, QUERY_HISTORY, IDENTIFIER |
| case_027 | stream_merge_metadata_actions | stream consumption MERGE using METADATA$ACTION and METADATA$ISUPDATE | STREAM, METADATA$ACTION, METADATA$ISUPDATE, MERGE, DELETE |
| case_028 | complex_set_ops_windows | set operations with nested windows, MINUS, INTERSECT, and ordered final output | UNION ALL, INTERSECT, MINUS, window, QUALIFY |
| case_029 | cursor_exception_nested_blocks | nested Snowflake Scripting block with cursor, exception handlers, and dynamic table names | EXECUTE IMMEDIATE block, CURSOR, nested BEGIN, EXCEPTION, IDENTIFIER |
| case_030 | mega_formatter_scenario | mega scenario combining DDL, CTE, MERGE, task call, comments, Unicode, and deep nesting | DDL, CTE, LATERAL FLATTEN, MERGE, CALL, Unicode, block comments, deep nesting |
