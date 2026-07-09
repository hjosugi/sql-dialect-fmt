//! COPY INTO (load and unload) parser coverage.
//!
//! The headline guarantees: every well-formed COPY statement (1) parses with **no diagnostics**
//! and (2) round-trips **byte-for-byte**; broken/partial input still round-trips and never panics.
//! Plus structural checks that the expected nodes (COPY_STMT, COPY_LOCATION, COPY_OPTION,
//! STAGE_REF) are produced, and that a transform `SELECT ... FROM @stage` parses as a real query.

use sql_dialect_fmt_parser::{parse, SyntaxKind};
use sql_dialect_fmt_test_support::parser::{assert_parse_clean, assert_parse_roundtrip};

/// Clean, real-world COPY statements crossing direction × shape. Each must parse without errors
/// and round-trip exactly.
const CLEAN: &[&str] = &[
    // load: stage forms
    "COPY INTO t FROM @s",
    "COPY INTO t FROM @s/path/",
    "COPY INTO t FROM @~/staged/",
    "COPY INTO t FROM @%mytable/data/",
    "COPY INTO db.sch.t FROM @db.sch.stage/orders/2026/06/",
    // load: column list + file format + options
    "COPY INTO t (a, b, c) FROM @s",
    "COPY INTO t FROM @s FILE_FORMAT = (TYPE = CSV)",
    "COPY INTO t FROM @s FILE_FORMAT = (TYPE = CSV SKIP_HEADER = 1 FIELD_DELIMITER = ',')",
    "COPY INTO t FROM @s FILE_FORMAT = (FORMAT_NAME = my_ff)",
    "COPY INTO t FROM @s PATTERN = '.*[.]csv'",
    "COPY INTO t FROM @s FILES = ('a.csv', 'b.csv')",
    "COPY INTO t FROM @s ON_ERROR = CONTINUE",
    "COPY INTO t FROM @s ON_ERROR = 'SKIP_FILE_5%' PURGE = FALSE FORCE = FALSE",
    "COPY INTO t FROM @s MATCH_BY_COLUMN_NAME = CASE_INSENSITIVE",
    "COPY INTO t FROM @s VALIDATION_MODE = RETURN_ERRORS",
    "COPY INTO t FROM @raw.stage/orders/ FILE_FORMAT = (TYPE = JSON) ON_ERROR = CONTINUE",
    // load: transform query source reading FROM @stage
    "COPY INTO t (a, b) FROM (SELECT $1, $2 FROM @s) FILE_FORMAT = (TYPE = CSV)",
    "COPY INTO t FROM (SELECT $1:a::STRING FROM @s (FILE_FORMAT => my_ff))",
    "COPY INTO t FROM (SELECT METADATA$FILENAME, $1 FROM @raw.stage/events/)",
    // unload: stage / external location targets
    "COPY INTO @s FROM t",
    "COPY INTO @s/out/ FROM t",
    "COPY INTO @~/exports/ FROM t",
    "COPY INTO @mart.stage/daily/ FROM t",
    "COPY INTO 'gcs://bucket/path/' FROM t FILE_FORMAT = (TYPE = PARQUET)",
    "COPY INTO 's3://bucket/prefix/' FROM (SELECT * FROM t) OVERWRITE = TRUE",
    // unload: query source + PARTITION BY + HEADER/SINGLE/OVERWRITE
    "COPY INTO @s FROM (SELECT * FROM t)",
    "COPY INTO @s FROM t PARTITION BY (dt) HEADER = TRUE",
    "COPY INTO @s FROM (SELECT a, b FROM t) PARTITION BY (a) FILE_FORMAT = (TYPE = CSV) HEADER = TRUE",
    "COPY INTO @s/out/ FROM t HEADER = TRUE SINGLE = TRUE MAX_FILE_SIZE = 16777216",
    "COPY INTO @s FROM t OVERWRITE = TRUE SINGLE = FALSE INCLUDE_QUERY_ID = TRUE DETAILED_OUTPUT = TRUE",
];

/// Broken / partial COPY input. Must still round-trip exactly and never panic (no clean-parse
/// requirement) — the parser is total.
const BROKEN: &[&str] = &[
    "COPY",
    "COPY INTO",
    "COPY INTO t",
    "COPY INTO t FROM",
    "COPY INTO t FROM @",
    "COPY INTO t FROM @s FILE_FORMAT =",
    "COPY INTO t FROM @s FILE_FORMAT = (",
    "COPY INTO FROM @s",
    "COPY INTO @s FROM (SELECT",
];

#[test]
fn clean_copy_statements_parse_without_errors() {
    for sql in CLEAN {
        assert_parse_clean(sql);
    }
}

#[test]
fn broken_copy_statements_round_trip_and_do_not_panic() {
    for sql in BROKEN {
        assert_parse_roundtrip(sql);
    }
}

fn has(sql: &str, kind: SyntaxKind) -> bool {
    parse(sql).syntax().descendants().any(|n| n.kind() == kind)
}

#[test]
fn produces_expected_nodes() {
    // A load: COPY_STMT with a COPY_LOCATION target and a STAGE_REF source.
    let load = "COPY INTO t FROM @s/p FILE_FORMAT = (TYPE = CSV)";
    assert!(has(load, SyntaxKind::COPY_STMT));
    assert!(has(load, SyntaxKind::COPY_LOCATION));
    assert!(has(load, SyntaxKind::COPY_OPTION));

    // An unload: stage target, parenthesized query source.
    let unload = "COPY INTO @s/out/ FROM (SELECT * FROM t) PARTITION BY (dt)";
    assert!(has(unload, SyntaxKind::COPY_STMT));
    assert!(has(unload, SyntaxKind::SUBQUERY));
    assert!(has(unload, SyntaxKind::COPY_OPTION));

    // A transform load: the inner SELECT reads FROM @stage, which must parse as a STAGE_REF inside
    // a real SELECT_STMT (not a verbatim blob).
    let transform = "COPY INTO t (a) FROM (SELECT $1 FROM @s)";
    assert!(has(transform, SyntaxKind::SELECT_STMT));
    assert!(has(transform, SyntaxKind::STAGE_REF));
}

#[test]
fn stage_locations_are_structured_inside_copy_locations() {
    for sql in ["COPY INTO t FROM @s/p", "COPY INTO @s/out/ FROM t"] {
        let parsed = assert_parse_clean(sql);
        let has_stage_location = parsed
            .syntax()
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::COPY_LOCATION)
            .any(|node| {
                node.descendants()
                    .any(|child| child.kind() == SyntaxKind::STAGE_REF)
            });
        assert!(
            has_stage_location,
            "expected COPY_LOCATION to contain a STAGE_REF for {sql:?}"
        );
    }
}

#[test]
fn select_from_stage_parses_as_a_query() {
    // The bug this guards: `SELECT ... FROM @stage` is a first-class table source.
    for sql in [
        "SELECT * FROM @s",
        "SELECT $1 FROM @s/p",
        "SELECT $1 FROM @raw.stage/orders/ (FILE_FORMAT => ff)",
        "SELECT METADATA$FILENAME, $1 FROM @~/staged/",
    ] {
        let p = assert_parse_clean(sql);
        assert!(
            p.syntax()
                .descendants()
                .any(|n| n.kind() == SyntaxKind::STAGE_REF),
            "expected a STAGE_REF in {sql:?}"
        );
    }
}
