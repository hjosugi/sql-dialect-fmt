//! Structured ALTER statement formatting (issue #30).
//!
//! A single action rides inline on the `ALTER … <name>` header; multiple comma-separated actions
//! each get their own indented line with the comma at the line end; a `SET` property list lays
//! out like a keyword item list (inline while it fits, one property per line when it overflows,
//! preserving the author's comma vs. space separators). Every case must be idempotent and
//! preserve its significant tokens; exact-string goldens pin the layout and casing opinions.

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::tokenize;
use sql_dialect_fmt_parser::parse;
use sql_dialect_fmt_syntax::SyntaxKind;

fn fmt(src: &str) -> String {
    format(src, &FormatOptions::default())
}

/// The signature a faithful formatter must preserve: meaningful tokens, upper-cased (formatting
/// only normalizes trivia and keyword/identifier casing), with the synthesized `;` dropped.
fn signature(sql: &str) -> Vec<String> {
    tokenize(sql)
        .tokens
        .into_iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::SEMICOLON)
        .map(|t| t.text.to_ascii_uppercase())
        .collect()
}

#[track_caller]
fn assert_invariants(src: &str) {
    let once = fmt(src);
    assert_eq!(once, fmt(&once), "not idempotent for {src:?}");
    assert_eq!(
        signature(src),
        signature(&once),
        "significant tokens changed for {src:?}\n--- out ---\n{once}"
    );
    assert!(
        parse(&once).errors().is_empty(),
        "formatting introduced parse errors for {src:?}\n--- out ---\n{once}"
    );
}

// ---- exact goldens: casing + spacing + layout ----

#[test]
fn single_action_stays_inline_and_cases_action_words() {
    assert_eq!(
        fmt("alter table t add column c int"),
        "ALTER TABLE t ADD COLUMN c INT;\n"
    );
    assert_eq!(
        fmt("alter table if exists db.sch.t drop column c"),
        "ALTER TABLE IF EXISTS db.sch.t DROP COLUMN c;\n"
    );
    assert_eq!(
        fmt("alter table t rename to u"),
        "ALTER TABLE t RENAME TO u;\n"
    );
    assert_eq!(
        fmt("alter table t swap with u"),
        "ALTER TABLE t SWAP WITH u;\n"
    );
    assert_eq!(
        fmt("alter task ops.t1 resume"),
        "ALTER TASK ops.t1 RESUME;\n"
    );
    assert_eq!(
        fmt("alter warehouse wh suspend"),
        "ALTER WAREHOUSE wh SUSPEND;\n"
    );
    assert_eq!(
        fmt("alter view v rename to v2"),
        "ALTER VIEW v RENAME TO v2;\n"
    );
    assert_eq!(
        fmt("alter dynamic table dt refresh"),
        "ALTER DYNAMIC TABLE dt REFRESH;\n"
    );
    assert_eq!(
        fmt("alter materialized view mv rename to mv2"),
        "ALTER MATERIALIZED VIEW mv RENAME TO mv2;\n"
    );
}

#[test]
fn multiple_actions_stack_one_per_line_with_trailing_commas() {
    assert_eq!(
        fmt("alter table t add column a int,add column b string,rename column c to d"),
        "ALTER TABLE t\n    ADD COLUMN a INT,\n    ADD COLUMN b STRING,\n    RENAME COLUMN c TO d;\n"
    );
}

#[test]
fn alter_session_set_expands_long_property_lists() {
    let src = "alter SESSION set TIMEZONE = 'Asia/Tokyo', WEEK_START = 1, \
               TIMESTAMP_OUTPUT_FORMAT = 'YYYY-MM-DD HH24:MI:SS.FF3 TZH:TZM', QUERY_TAG = 'e2e'";
    assert_eq!(
        fmt(src),
        "ALTER SESSION SET\n    TIMEZONE = 'Asia/Tokyo',\n    WEEK_START = 1,\n    \
         TIMESTAMP_OUTPUT_FORMAT = 'YYYY-MM-DD HH24:MI:SS.FF3 TZH:TZM',\n    QUERY_TAG = 'e2e';\n"
    );
}

#[test]
fn short_set_property_lists_stay_inline_and_upcase_known_keys() {
    assert_eq!(
        fmt("alter session set timezone = 'UTC'"),
        "ALTER SESSION SET TIMEZONE = 'UTC';\n"
    );
    assert_eq!(
        fmt("alter task t set schedule = '5 minutes'"),
        "ALTER TASK t SET SCHEDULE = '5 minutes';\n"
    );
}

#[test]
fn set_without_commas_never_gains_them() {
    // The space-separated warehouse/task property style must round-trip without invented commas.
    assert_eq!(
        fmt("alter warehouse wh set warehouse_size = 'LARGE' auto_suspend = 60"),
        "ALTER WAREHOUSE wh SET WAREHOUSE_SIZE = 'LARGE' AUTO_SUSPEND = 60;\n"
    );
}

#[test]
fn governance_actions_keep_action_boundaries() {
    let src = "ALTER TABLE p MODIFY COLUMN email SET MASKING POLICY g.m USING(email,tenant_id),\
               SET TAG g.r = 'standard'";
    assert_eq!(
        fmt(src),
        "ALTER TABLE p\n    MODIFY COLUMN email SET MASKING POLICY g.m USING (email, tenant_id),\n    \
         SET TAG g.r = 'standard';\n"
    );
}

#[test]
fn search_optimization_comma_stays_inside_the_single_action() {
    assert_eq!(
        fmt("alter table t add search optimization on equality(a, b), substring(c)"),
        "ALTER TABLE t ADD SEARCH OPTIMIZATION ON equality(a, b), substring(c);\n"
    );
}

#[test]
fn unmodeled_alter_kinds_keep_the_inline_lenient_lowering() {
    assert_eq!(
        fmt("alter user u set default_role = 'ANALYST'"),
        "ALTER USER u SET default_role = 'ANALYST';\n"
    );
    // `pipe` is not a recognized lenient word today; the historical verbatim casing is kept.
    assert_eq!(fmt("alter pipe p refresh"), "ALTER pipe p refresh;\n");
}

// ---- invariants over the matrix ----

#[test]
fn alter_matrix_is_idempotent_and_token_preserving() {
    for src in [
        "alter table t add column c int",
        "alter table t add column if not exists c number(10, 2) comment 'x'",
        "alter table t drop column a, b",
        "alter table t add column a int, drop column b, rename column c to d",
        "alter table t alter column c set default current_timestamp()",
        "alter table t modify column c set not null",
        "alter table t cluster by (a, b)",
        "alter table t drop constraint pk cascade",
        "alter table t swap with u",
        "alter session set timezone = 'UTC', week_start = 1, query_tag = 'etl'",
        "alter session unset query_tag, week_start",
        "alter warehouse wh set warehouse_size = 'LARGE' auto_suspend = 60 min_cluster_count = 1 max_cluster_count = 4",
        "alter warehouse if exists wh resume",
        "alter task t set schedule = '5 minutes', suspend_task_after_num_failures = 3",
        "alter task ops.t suspend",
        "alter view v set secure",
        "alter schema s rename to s2",
        "alter database d set data_retention_time_in_days = 7",
        "alter user u set password = 'x' must_change_password = true",
        "ALTER TABLE CORE.P MODIFY COLUMN email SET MASKING POLICY g.m USING(email,tenant_id),MODIFY COLUMN email SET TAG g.c = 'restricted',SET TAG g.r = 'standard'",
        "ALTER TABLE t ADD COLUMN IF NOT EXISTS h ARRAY COMMENT 'a',ADD COLUMN IF NOT EXISTS p OBJECT COMMENT 'b',ALTER COLUMN u SET DEFAULT CURRENT_TIMESTAMP()",
        // malformed tails must still format losslessly and idempotently
        "alter table t add column",
        "alter session set timezone =",
        "alter table t add column c int,",
    ] {
        assert_invariants(src);
    }
}

#[test]
fn multiline_alter_reflows_to_the_same_layout() {
    // An already-expanded statement (the fixture layout) reflows to the formatter's layout and
    // stays there.
    let src = "ALTER TABLE t\n    ADD COLUMN a INT,\n    DROP COLUMN b;\n";
    let out = fmt(src);
    assert_eq!(
        out,
        "ALTER TABLE t\n    ADD COLUMN a INT,\n    DROP COLUMN b;\n"
    );
    assert_eq!(fmt(&out), out);
}

#[test]
fn comments_inside_alter_fall_back_to_verbatim_and_stay_stable() {
    let src = "alter table t add column a int, -- keep\n drop column b;\n";
    let once = fmt(src);
    assert!(once.contains("-- keep"), "comment dropped: {once:?}");
    assert_eq!(once, fmt(&once), "comment fallback not idempotent");
}
