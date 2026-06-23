//! Embedded golden fixtures for tests.
//!
//! `cargo test --workspace` must remain self-contained. Keep the curated golden
//! set here; broader/generated corpora can still be exercised manually through
//! the CLI `--fixtures` flag without committing large external directories.

#[derive(Clone, Copy, Debug)]
pub struct GoldenCase {
    pub name: &'static str,
    pub input: &'static str,
    pub expected_full: &'static str,
    pub expected_sql_only: Option<&'static str>,
}

impl GoldenCase {
    pub fn expected(self, profile: GoldenProfile) -> &'static str {
        match profile {
            GoldenProfile::Full => self.expected_full,
            GoldenProfile::SqlOnly => self.expected_sql_only.unwrap_or(self.expected_full),
        }
    }

    pub fn sqls(self) -> impl Iterator<Item = (&'static str, &'static str)> {
        [
            Some(("input", self.input)),
            Some(("expected_full", self.expected_full)),
            self.expected_sql_only.map(|sql| ("expected_sql_only", sql)),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoldenProfile {
    Full,
    SqlOnly,
}

macro_rules! golden_case {
    ($name:literal) => {
        GoldenCase {
            name: $name,
            input: include_str!(concat!("../fixtures/easy/", $name, "/input.sql")),
            expected_full: include_str!(concat!("../fixtures/easy/", $name, "/expected.sql")),
            expected_sql_only: None,
        }
    };
    ($name:literal, sql_only) => {
        GoldenCase {
            name: $name,
            input: include_str!(concat!("../fixtures/easy/", $name, "/input.sql")),
            expected_full: include_str!(concat!("../fixtures/easy/", $name, "/expected.sql")),
            expected_sql_only: Some(include_str!(concat!(
                "../fixtures/easy/",
                $name,
                "/expected_sql_only.sql"
            ))),
        }
    };
}

pub const EASY_CASES: &[GoldenCase] = &[
    golden_case!("01_select_semistructured"),
    golden_case!("02_sql_scripting_procedure"),
    golden_case!("03_javascript_procedure", sql_only),
    golden_case!("04_python_procedure", sql_only),
    golden_case!("05_copy_merge_stream_task"),
    golden_case!("06_multilingual_unicode"),
    golden_case!("07_e2e_final_scenario", sql_only),
    golden_case!("case_001_deep_json_lateral_flatten"),
    golden_case!("case_002_recursive_org_rollup"),
    golden_case!("case_003_match_recognize_session_funnel"),
    golden_case!("case_004_pivot_unpivot_grouping_sets"),
    golden_case!("case_005_merge_nested_source"),
    golden_case!("case_006_multi_table_insert_first"),
    golden_case!("case_007_copy_into_transform_metadata"),
    golden_case!("case_008_copy_unload_partitioned"),
    golden_case!("case_009_security_policies_table"),
    golden_case!("case_010_dynamic_table_nested_refresh"),
    golden_case!("case_011_task_graph_streams_finalizer"),
    golden_case!("case_012_sql_scripting_nested_procedure"),
    golden_case!("case_013_javascript_procedure_dynamic_sql"),
    golden_case!("case_014_python_snowpark_procedure"),
    golden_case!("case_015_anonymous_procedure_call_with"),
    golden_case!("case_016_udf_udtf_mixed_languages"),
    golden_case!("case_017_snowpipe_create_pipe"),
    golden_case!("case_018_alert_exists_notification"),
    golden_case!("case_019_materialized_view_search_optimization"),
    golden_case!("case_020_time_travel_clone_swap"),
    golden_case!("case_021_secure_view_policy_query"),
    golden_case!("case_022_asof_join_resample_timeseries"),
    golden_case!("case_023_semantic_view_complex"),
    golden_case!("case_024_tag_classification_alter"),
    golden_case!("case_025_stage_file_ops_directory"),
    golden_case!("case_026_transaction_result_scan_query_history"),
    golden_case!("case_027_stream_merge_metadata_actions"),
    golden_case!("case_028_complex_set_ops_windows"),
    golden_case!("case_029_cursor_exception_nested_blocks"),
    golden_case!("case_030_mega_formatter_scenario"),
    golden_case!("case_031_grant_revoke_privileges"),
    golden_case!("case_032_create_object_kinds"),
    golden_case!("case_033_call_procedure"),
    golden_case!("case_034_use_show_describe_truncate"),
];

pub const MINIMUM_EMBEDDED_EASY_CASES: usize = 41;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_cases_are_nonempty_and_named() {
        assert!(EASY_CASES.len() >= MINIMUM_EMBEDDED_EASY_CASES);
        for case in EASY_CASES {
            assert!(!case.name.is_empty());
            assert!(case.input.ends_with('\n'));
            assert!(case.expected_full.ends_with('\n'));
            if let Some(sql_only) = case.expected_sql_only {
                assert!(sql_only.ends_with('\n'));
            }
        }
    }
}
