//! Case-insensitive recognition of keyword text, and its **dialect-aware reservation**.
//!
//! A single table — [`KEYWORDS`] — is the one source of truth: every reserved keyword appears
//! exactly once as `(lowercase text, SyntaxKind, DialectSet)`. From it we derive both the
//! text→kind lookup ([`keyword_kind`]) and the dialect-aware variant ([`keyword_kind_for`]), and a
//! completeness test proves the table covers the whole `__KW_START..__KW_END` block so the two can
//! never drift.
//!
//! ## Why a dialect dimension
//! Whether a word is *reserved* (forced into the grammar instead of being a plain identifier)
//! differs by dialect. Snowflake reserves words like `TASK`, `WAREHOUSE`, `FLATTEN`, `QUALIFY` that
//! Databricks treats as ordinary identifiers (they are Snowflake-specific DDL/feature words and do
//! not appear in the Spark SQL keyword table). [`DialectSet`] records, per keyword, *which*
//! dialects reserve it; the parser consults this so `SELECT task, flatten FROM t` parses clean
//! under Databricks while Snowflake's reservation is unchanged.

use crate::{Dialect, DialectSet, SyntaxKind};

// Per-keyword dialect reservation is a [`DialectSet`]: a bitmask over [`Dialect`]. `DialectSet::ALL`
// is the standard-SQL "reserved everywhere" set; Snowflake-only words use
// `DialectSet::SNOWFLAKE_ONLY`, and so on. See `dialect.rs`.

/// The single source of truth for keyword recognition: `(lowercase text, kind, dialect set)`.
///
/// Each entry's text must be ASCII lowercase (lookups lowercase the input before matching). The
/// `every_keyword_variant_is_mapped` test asserts this list covers exactly the `SyntaxKind` keyword
/// block, so a keyword cannot be added to the enum without a matching entry (and dialect class)
/// here. Entries must remain sorted by text because lookup uses binary search.
const KEYWORDS: &[(&str, SyntaxKind, DialectSet)] = {
    use SyntaxKind::*;
    &[
        ("after", AFTER_KW, DialectSet::ALL),
        ("all", ALL_KW, DialectSet::ALL),
        ("alter", ALTER_KW, DialectSet::ALL),
        ("and", AND_KW, DialectSet::ALL),
        ("any", ANY_KW, DialectSet::ALL),
        ("as", AS_KW, DialectSet::ALL),
        ("asc", ASC_KW, DialectSet::ALL),
        ("begin", BEGIN_KW, DialectSet::ALL),
        ("between", BETWEEN_KW, DialectSet::ALL),
        ("by", BY_KW, DialectSet::ALL),
        ("call", CALL_KW, DialectSet::ALL),
        ("called", CALLED_KW, DialectSet::SNOWFLAKE_ONLY),
        ("caller", CALLER_KW, DialectSet::SNOWFLAKE_ONLY),
        ("case", CASE_KW, DialectSet::ALL),
        ("cast", CAST_KW, DialectSet::ALL),
        ("commit", COMMIT_KW, DialectSet::ALL),
        (
            "conflict",
            CONFLICT_KW,
            DialectSet::of(&[Dialect::PostgreSql, Dialect::Sqlite, Dialect::DuckDb]),
        ),
        // CONNECT/PRIOR: Snowflake hierarchical `CONNECT BY`; absent from the Spark keyword table.
        ("connect", CONNECT_KW, DialectSet::SNOWFLAKE_ONLY),
        ("copy", COPY_KW, DialectSet::SNOWFLAKE_ONLY),
        ("create", CREATE_KW, DialectSet::ALL),
        ("cross", CROSS_KW, DialectSet::ALL),
        ("current", CURRENT_KW, DialectSet::ALL),
        (
            "cursor",
            CURSOR_KW,
            DialectSet::of(&[Dialect::Snowflake, Dialect::PlSql]),
        ),
        ("declare", DECLARE_KW, DialectSet::ALL),
        ("delete", DELETE_KW, DialectSet::ALL),
        ("desc", DESC_KW, DialectSet::ALL),
        ("describe", DESCRIBE_KW, DialectSet::ALL),
        ("distinct", DISTINCT_KW, DialectSet::ALL),
        ("do", DO_KW, DialectSet::ALL),
        ("drop", DROP_KW, DialectSet::ALL),
        (
            "duplicate",
            DUPLICATE_KW,
            DialectSet::of(&[
                Dialect::MySql,
                Dialect::MariaDb,
                Dialect::TiDb,
                Dialect::SingleStoreDb,
            ]),
        ),
        ("else", ELSE_KW, DialectSet::ALL),
        ("elseif", ELSEIF_KW, DialectSet::SNOWFLAKE_ONLY),
        ("end", END_KW, DialectSet::ALL),
        ("except", EXCEPT_KW, DialectSet::ALL),
        (
            "exception",
            EXCEPTION_KW,
            DialectSet::of(&[Dialect::Snowflake, Dialect::PlSql]),
        ),
        ("execute", EXECUTE_KW, DialectSet::ALL),
        ("exists", EXISTS_KW, DialectSet::ALL),
        ("false", FALSE_KW, DialectSet::ALL),
        ("fetch", FETCH_KW, DialectSet::ALL),
        ("first", FIRST_KW, DialectSet::ALL),
        // FLATTEN: Snowflake table function; not a Spark keyword.
        ("flatten", FLATTEN_KW, DialectSet::SNOWFLAKE_ONLY),
        ("following", FOLLOWING_KW, DialectSet::ALL),
        ("for", FOR_KW, DialectSet::ALL),
        ("from", FROM_KW, DialectSet::ALL),
        ("full", FULL_KW, DialectSet::ALL),
        ("function", FUNCTION_KW, DialectSet::ALL),
        ("grant", GRANT_KW, DialectSet::ALL),
        ("grants", GRANTS_KW, DialectSet::SNOWFLAKE_ONLY),
        ("group", GROUP_KW, DialectSet::ALL),
        ("handler", HANDLER_KW, DialectSet::SNOWFLAKE_ONLY),
        ("having", HAVING_KW, DialectSet::ALL),
        ("if", IF_KW, DialectSet::ALL),
        // ILIKE/RLIKE/REGEXP: Snowflake operators; non-reserved in Spark, so identifiers there.
        (
            "ilike",
            ILIKE_KW,
            DialectSet::of(&[
                Dialect::Snowflake,
                Dialect::Databricks,
                Dialect::Spark,
                Dialect::Hive,
                Dialect::PostgreSql,
                Dialect::Redshift,
                Dialect::DuckDb,
                Dialect::ClickHouse,
                Dialect::Trino,
            ]),
        ),
        ("immediate", IMMEDIATE_KW, DialectSet::SNOWFLAKE_ONLY),
        ("imports", IMPORTS_KW, DialectSet::SNOWFLAKE_ONLY),
        ("in", IN_KW, DialectSet::ALL),
        ("inner", INNER_KW, DialectSet::ALL),
        ("input", INPUT_KW, DialectSet::ALL),
        ("insert", INSERT_KW, DialectSet::ALL),
        ("intersect", INTERSECT_KW, DialectSet::ALL),
        ("into", INTO_KW, DialectSet::ALL),
        ("is", IS_KW, DialectSet::ALL),
        ("java", JAVA_KW, DialectSet::ALL),
        // JAVASCRIPT/SCALA: Snowflake `LANGUAGE` values, not Spark keywords. (JAVA/PYTHON/SQL also
        // serve as type/language words common to both, so they stay shared.)
        ("javascript", JAVASCRIPT_KW, DialectSet::SNOWFLAKE_ONLY),
        ("join", JOIN_KW, DialectSet::ALL),
        ("language", LANGUAGE_KW, DialectSet::ALL),
        ("last", LAST_KW, DialectSet::ALL),
        ("lateral", LATERAL_KW, DialectSet::ALL),
        ("left", LEFT_KW, DialectSet::ALL),
        ("let", LET_KW, DialectSet::ALL),
        ("like", LIKE_KW, DialectSet::ALL),
        ("limit", LIMIT_KW, DialectSet::ALL),
        ("loop", LOOP_KW, DialectSet::ALL),
        ("matched", MATCHED_KW, DialectSet::ALL),
        ("merge", MERGE_KW, DialectSet::ALL),
        ("minus", MINUS_KW, DialectSet::ALL),
        ("natural", NATURAL_KW, DialectSet::ALL),
        ("not", NOT_KW, DialectSet::ALL),
        (
            "nothing",
            NOTHING_KW,
            DialectSet::of(&[Dialect::PostgreSql, Dialect::Sqlite, Dialect::DuckDb]),
        ),
        ("null", NULL_KW, DialectSet::ALL),
        ("nulls", NULLS_KW, DialectSet::ALL),
        ("offset", OFFSET_KW, DialectSet::ALL),
        ("on", ON_KW, DialectSet::ALL),
        ("or", OR_KW, DialectSet::ALL),
        ("order", ORDER_KW, DialectSet::ALL),
        ("out", OUT_KW, DialectSet::ALL),
        ("outer", OUTER_KW, DialectSet::ALL),
        ("output", OUTPUT_KW, DialectSet::ALL),
        ("over", OVER_KW, DialectSet::ALL),
        ("overwrite", OVERWRITE_KW, DialectSet::ALL),
        ("owner", OWNER_KW, DialectSet::SNOWFLAKE_ONLY),
        ("packages", PACKAGES_KW, DialectSet::SNOWFLAKE_ONLY),
        ("partition", PARTITION_KW, DialectSet::ALL),
        ("pivot", PIVOT_KW, DialectSet::ALL),
        ("preceding", PRECEDING_KW, DialectSet::ALL),
        (
            "prewhere",
            PREWHERE_KW,
            DialectSet::of(&[Dialect::ClickHouse]),
        ),
        ("prior", PRIOR_KW, DialectSet::SNOWFLAKE_ONLY),
        ("procedure", PROCEDURE_KW, DialectSet::ALL),
        ("python", PYTHON_KW, DialectSet::ALL),
        // QUALIFY: a window-filter clause in BOTH dialects. Databricks SQL supports `SELECT ...
        // QUALIFY <predicate>` (Databricks Runtime 10.4 LTS+), so it must stay reserved under
        // Databricks too — otherwise the parser treats it as a plain identifier and mis-splits the
        // query. Reserving it in both dialects leaves Snowflake byte-identical (it was reserved
        // there already).
        (
            "qualify",
            QUALIFY_KW,
            DialectSet::of(&[
                Dialect::Snowflake,
                Dialect::Databricks,
                Dialect::Spark,
                Dialect::BigQuery,
                Dialect::DuckDb,
                Dialect::Trino,
            ]),
        ),
        ("range", RANGE_KW, DialectSet::ALL),
        ("recursive", RECURSIVE_KW, DialectSet::ALL),
        (
            "regexp",
            REGEXP_KW,
            DialectSet::of(&[
                Dialect::Snowflake,
                Dialect::MySql,
                Dialect::MariaDb,
                Dialect::TiDb,
                Dialect::SingleStoreDb,
            ]),
        ),
        ("repeat", REPEAT_KW, DialectSet::ALL),
        ("replace", REPLACE_KW, DialectSet::ALL),
        ("resultset", RESULTSET_KW, DialectSet::SNOWFLAKE_ONLY),
        ("return", RETURN_KW, DialectSet::ALL),
        (
            "returning",
            RETURNING_KW,
            DialectSet::of(&[
                Dialect::PostgreSql,
                Dialect::Sqlite,
                Dialect::DuckDb,
                Dialect::MariaDb,
                Dialect::N1ql,
            ]),
        ),
        ("returns", RETURNS_KW, DialectSet::ALL),
        ("revoke", REVOKE_KW, DialectSet::ALL),
        ("right", RIGHT_KW, DialectSet::ALL),
        (
            "rlike",
            RLIKE_KW,
            DialectSet::of(&[
                Dialect::Snowflake,
                Dialect::Databricks,
                Dialect::Spark,
                Dialect::Hive,
            ]),
        ),
        ("rollback", ROLLBACK_KW, DialectSet::ALL),
        ("row", ROW_KW, DialectSet::ALL),
        ("rows", ROWS_KW, DialectSet::ALL),
        (
            "runtime_version",
            RUNTIME_VERSION_KW,
            DialectSet::SNOWFLAKE_ONLY,
        ),
        // SAMPLE: Snowflake spelling; absent from the Spark keyword table (`TABLESAMPLE` is shared).
        ("sample", SAMPLE_KW, DialectSet::SNOWFLAKE_ONLY),
        ("scala", SCALA_KW, DialectSet::SNOWFLAKE_ONLY),
        ("schedule", SCHEDULE_KW, DialectSet::SNOWFLAKE_ONLY),
        // Snowflake table-property words; not Spark keywords.
        ("secure", SECURE_KW, DialectSet::SNOWFLAKE_ONLY),
        ("select", SELECT_KW, DialectSet::ALL),
        ("set", SET_KW, DialectSet::ALL),
        ("show", SHOW_KW, DialectSet::ALL),
        ("sql", SQL_KW, DialectSet::ALL),
        ("start", START_KW, DialectSet::ALL),
        ("strict", STRICT_KW, DialectSet::SNOWFLAKE_ONLY),
        ("table", TABLE_KW, DialectSet::ALL),
        ("tablesample", TABLESAMPLE_KW, DialectSet::ALL),
        // Snowflake object DDL / scripting words absent from the Spark keyword table.
        ("task", TASK_KW, DialectSet::SNOWFLAKE_ONLY),
        ("temp", TEMP_KW, DialectSet::ALL),
        ("temporary", TEMPORARY_KW, DialectSet::ALL),
        ("then", THEN_KW, DialectSet::ALL),
        // Snowflake's row-limiting `TOP n`; not a Spark keyword.
        (
            "top",
            TOP_KW,
            DialectSet::of(&[Dialect::Snowflake, Dialect::TransactSql]),
        ),
        ("transient", TRANSIENT_KW, DialectSet::SNOWFLAKE_ONLY),
        ("true", TRUE_KW, DialectSet::ALL),
        ("truncate", TRUNCATE_KW, DialectSet::ALL),
        // TRY_CAST is the function `try_cast(...)` in Spark, not a structural keyword.
        ("try_cast", TRY_CAST_KW, DialectSet::SNOWFLAKE_ONLY),
        ("unbounded", UNBOUNDED_KW, DialectSet::ALL),
        // Snowflake scripting / object words absent from the Spark keyword table.
        ("undrop", UNDROP_KW, DialectSet::SNOWFLAKE_ONLY),
        ("union", UNION_KW, DialectSet::ALL),
        ("unpivot", UNPIVOT_KW, DialectSet::ALL),
        ("until", UNTIL_KW, DialectSet::ALL),
        ("update", UPDATE_KW, DialectSet::ALL),
        ("use", USE_KW, DialectSet::ALL),
        ("using", USING_KW, DialectSet::ALL),
        ("values", VALUES_KW, DialectSet::ALL),
        ("view", VIEW_KW, DialectSet::ALL),
        ("volatile", VOLATILE_KW, DialectSet::SNOWFLAKE_ONLY),
        ("warehouse", WAREHOUSE_KW, DialectSet::SNOWFLAKE_ONLY),
        ("when", WHEN_KW, DialectSet::ALL),
        ("where", WHERE_KW, DialectSet::ALL),
        ("while", WHILE_KW, DialectSet::ALL),
        ("window", WINDOW_KW, DialectSet::ALL),
        ("with", WITH_KW, DialectSet::ALL),
        ("within", WITHIN_KW, DialectSet::ALL),
    ]
};

const MAX_KEYWORD_LEN: usize = 16;

/// Lowercase `ident` into a stack buffer, returning the byte length — or `None` if it cannot be a
/// keyword (empty, or longer than the longest keyword). Allocation-free: keeps the lexer's hot path
/// off the heap. ASCII-lowercasing a valid `&str` byte-by-byte yields valid UTF-8, so the caller's
/// `from_utf8` never errors.
#[inline]
fn lower_for_lookup(ident: &str, buf: &mut [u8; MAX_KEYWORD_LEN]) -> Option<usize> {
    let bytes = ident.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_KEYWORD_LEN {
        return None;
    }
    for (slot, &b) in buf.iter_mut().zip(bytes) {
        *slot = b.to_ascii_lowercase();
    }
    Some(bytes.len())
}

/// Look up a keyword and its dialect classification from `ident`, case-insensitively.
#[inline]
fn lookup(ident: &str) -> Option<(SyntaxKind, DialectSet)> {
    let mut buf = [0u8; MAX_KEYWORD_LEN];
    let len = lower_for_lookup(ident, &mut buf)?;
    let lower = std::str::from_utf8(&buf[..len]).ok()?;
    KEYWORDS
        .binary_search_by(|(text, _, _)| text.cmp(&lower))
        .ok()
        .map(|index| {
            let (_, kind, dialect) = KEYWORDS[index];
            (kind, dialect)
        })
}

/// Map an identifier's text to its keyword kind, case-insensitively, using **Snowflake** semantics.
///
/// Snowflake folds unquoted identifiers and matches keywords without regard to case. Returns `None`
/// for plain identifiers — the lexer emits [`SyntaxKind::IDENT`] for every word and the parser uses
/// this to reclassify keywords contextually. Kept for backward compatibility; prefer
/// [`keyword_kind_for`] when a [`Dialect`] is in hand.
#[must_use]
pub fn keyword_kind(ident: &str) -> Option<SyntaxKind> {
    keyword_kind_for(ident, Dialect::Snowflake)
}

/// Like [`keyword_kind`], but a word counts as a keyword only when `dialect` reserves it.
///
/// A Snowflake-only word (e.g. `TASK`, `FLATTEN`) returns its keyword kind under
/// [`Dialect::Snowflake`] but `None` under [`Dialect::Databricks`], where it is an ordinary
/// identifier. DialectSet::ALL keywords behave identically in every dialect, so under
/// [`Dialect::Snowflake`] this is byte-for-byte equivalent to [`keyword_kind`].
#[must_use]
pub fn keyword_kind_for(ident: &str, dialect: Dialect) -> Option<SyntaxKind> {
    let (kind, kw_dialect) = lookup(ident)?;
    kw_dialect.reserved_in(dialect).then_some(kind)
}

/// Canonical lowercase keyword spellings, in the same sorted order as the lookup table.
///
/// This lets editor integrations and completion providers share the parser's keyword source
/// instead of carrying independent reserved-word lists.
pub fn keyword_texts() -> impl ExactSizeIterator<Item = &'static str> {
    KEYWORDS.iter().map(|(text, _, _)| *text)
}

#[cfg(test)]
mod tests {
    use super::{keyword_kind, keyword_kind_for, keyword_texts, KEYWORDS};
    use crate::{Dialect, SyntaxKind};

    #[test]
    fn keyword_lookup_is_case_insensitive() {
        assert_eq!(keyword_kind("select"), Some(SyntaxKind::SELECT_KW));
        assert_eq!(keyword_kind("SeLeCt"), Some(SyntaxKind::SELECT_KW));
        assert_eq!(keyword_kind("QUALIFY"), Some(SyntaxKind::QUALIFY_KW));
        assert_eq!(keyword_kind("javascript"), Some(SyntaxKind::JAVASCRIPT_KW));
        assert_eq!(keyword_kind("try_cast"), Some(SyntaxKind::TRY_CAST_KW));
        assert_eq!(keyword_kind("TASK"), Some(SyntaxKind::TASK_KW));
        assert_eq!(
            keyword_kind("runtime_version"),
            Some(SyntaxKind::RUNTIME_VERSION_KW)
        );
        assert_eq!(keyword_kind("definitely_not_a_keyword"), None);
    }

    #[test]
    fn every_keyword_variant_is_mapped() {
        // Completeness: the table must have exactly one entry per keyword enum variant.
        let range_count = SyntaxKind::__KW_END as u16 - SyntaxKind::__KW_START as u16 - 1;
        assert_eq!(
            KEYWORDS.len() as u16,
            range_count,
            "KEYWORDS table is out of sync with the SyntaxKind keyword block"
        );
        let mut seen = std::collections::HashSet::new();
        for (text, kind, dialects) in KEYWORDS {
            // The text maps to exactly this kind in every dialect that reserves it, and to no kind
            // (or never a different kind) elsewhere.
            let mut reserved_somewhere = false;
            for dialect in Dialect::ALL {
                if let Some(resolved) = keyword_kind_for(text, *dialect) {
                    assert_eq!(resolved, *kind, "keyword_kind_for({text:?}) is wrong");
                    reserved_somewhere = true;
                }
                assert_eq!(
                    keyword_kind_for(&text.to_uppercase(), *dialect),
                    keyword_kind_for(text, *dialect),
                    "keyword lookup is not case-insensitive for {text:?}"
                );
            }
            assert!(
                reserved_somewhere,
                "{text:?} ({kind:?}) is reserved in no dialect ({dialects:?})"
            );
            assert!(
                kind.is_keyword(),
                "{kind:?} should be inside the keyword range"
            );
            assert!(seen.insert(*kind), "duplicate keyword kind for {text:?}");
            assert!(
                text.bytes().all(|b| !b.is_ascii_uppercase()),
                "KEYWORDS text must be lowercase: {text:?}"
            );
        }
    }

    #[test]
    fn keywords_are_sorted_for_binary_search() {
        for window in KEYWORDS.windows(2) {
            let (left, _, _) = window[0];
            let (right, _, _) = window[1];
            assert!(
                left < right,
                "KEYWORDS must be sorted: {left:?} >= {right:?}"
            );
        }
    }

    #[test]
    fn keyword_texts_exposes_the_lookup_table_order() {
        let texts: Vec<_> = keyword_texts().collect();
        assert_eq!(texts.len(), KEYWORDS.len());
        assert_eq!(texts.first(), Some(&"after"));
        assert_eq!(texts.last(), Some(&"within"));
        for (text, (table_text, _, _)) in texts.iter().zip(KEYWORDS) {
            assert_eq!(text, table_text);
        }
    }

    #[test]
    fn every_keyword_has_a_dialect_classification() {
        // Completeness, dialect dimension: every keyword in the table is reserved in at least one
        // dialect, so the reservation set cannot silently drift as keywords are added.
        for (text, kind, dialects) in KEYWORDS {
            assert!(
                !dialects.is_empty(),
                "{text:?} ({kind:?}) is reserved in no dialect"
            );
        }
    }

    #[test]
    fn snowflake_classification_is_byte_identical_to_legacy_keyword_kind() {
        // Under Snowflake, the dialect-aware lookup must agree with the plain `keyword_kind` for
        // every keyword — the regression guard that Snowflake reservation is unchanged.
        for (text, kind, dialects) in KEYWORDS {
            let expected = dialects.reserved_in(Dialect::Snowflake).then_some(*kind);
            assert_eq!(
                keyword_kind_for(text, Dialect::Snowflake),
                expected,
                "Snowflake reservation changed for {text:?}"
            );
            assert_eq!(
                keyword_kind(text),
                keyword_kind_for(text, Dialect::Snowflake)
            );
        }
    }

    #[test]
    fn shared_keywords_are_reserved_in_every_dialect() {
        // Standard SQL words stay reserved under Databricks.
        for word in ["select", "from", "where", "join", "group", "order", "case"] {
            assert!(
                keyword_kind_for(word, Dialect::Databricks).is_some(),
                "{word}"
            );
            assert!(
                keyword_kind_for(word, Dialect::Snowflake).is_some(),
                "{word}"
            );
        }
    }

    #[test]
    fn snowflake_only_keywords_are_identifiers_under_databricks() {
        // The Snowflake-only DDL/feature words must drop their reservation under Databricks.
        for word in [
            "task",
            "flatten",
            "warehouse",
            "schedule",
            "transient",
            "volatile",
            "secure",
            "undrop",
            "elseif",
            "cursor",
            "resultset",
            "connect",
            "prior",
            "top",
            "copy",
            "owner",
        ] {
            assert!(
                keyword_kind_for(word, Dialect::Snowflake).is_some(),
                "{word} should be reserved in Snowflake"
            );
            assert_eq!(
                keyword_kind_for(word, Dialect::Databricks),
                None,
                "{word} must be a plain identifier under Databricks"
            );
        }
    }

    #[test]
    fn dialect_memberships_are_honored() {
        // TOP: Snowflake and Transact-SQL, not MySQL.
        assert_eq!(
            keyword_kind_for("top", Dialect::Snowflake),
            Some(SyntaxKind::TOP_KW)
        );
        assert_eq!(
            keyword_kind_for("top", Dialect::TransactSql),
            Some(SyntaxKind::TOP_KW)
        );
        assert_eq!(keyword_kind_for("top", Dialect::MySql), None);

        // QUALIFY: Snowflake/Spark/BigQuery/DuckDB/Trino, not MySQL/PostgreSQL.
        assert_eq!(
            keyword_kind_for("qualify", Dialect::BigQuery),
            Some(SyntaxKind::QUALIFY_KW)
        );
        assert_eq!(keyword_kind_for("qualify", Dialect::MySql), None);
        assert_eq!(keyword_kind_for("qualify", Dialect::PostgreSql), None);

        // ILIKE: PostgreSQL/Spark-family, not MySQL.
        assert_eq!(
            keyword_kind_for("ilike", Dialect::PostgreSql),
            Some(SyntaxKind::ILIKE_KW)
        );
        assert_eq!(keyword_kind_for("ilike", Dialect::MySql), None);

        // REGEXP: MySQL family, not PostgreSQL.
        assert_eq!(
            keyword_kind_for("regexp", Dialect::MySql),
            Some(SyntaxKind::REGEXP_KW)
        );
        assert_eq!(keyword_kind_for("regexp", Dialect::PostgreSql), None);
    }
}
