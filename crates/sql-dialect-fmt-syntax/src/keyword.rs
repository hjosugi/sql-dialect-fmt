//! Case-insensitive recognition of keyword text, and its **dialect-aware reservation**.
//!
//! A single table — [`KEYWORDS`] — is the one source of truth: every reserved keyword appears
//! exactly once as `(lowercase text, SyntaxKind, KeywordDialect)`. From it we derive both the
//! text→kind lookup ([`keyword_kind`]) and the dialect-aware variant ([`keyword_kind_for`]), and a
//! completeness test proves the table covers the whole `__KW_START..__KW_END` block so the two can
//! never drift.
//!
//! ## Why a dialect dimension
//! Whether a word is *reserved* (forced into the grammar instead of being a plain identifier)
//! differs by dialect. Snowflake reserves words like `TASK`, `WAREHOUSE`, `FLATTEN`, `QUALIFY` that
//! Databricks treats as ordinary identifiers (they are Snowflake-specific DDL/feature words and do
//! not appear in the Spark SQL keyword table). [`KeywordDialect`] records, per keyword, *which*
//! dialects reserve it; the parser consults this so `SELECT task, flatten FROM t` parses clean
//! under Databricks while Snowflake's reservation is unchanged.

use crate::{Dialect, SyntaxKind};

/// Which dialect(s) reserve a given keyword. The grammar treats a word as a keyword only when the
/// active [`Dialect`] reserves it; otherwise the word is an ordinary identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeywordDialect {
    /// Reserved in every dialect (standard SQL words: `SELECT`, `FROM`, `JOIN`, …).
    Shared,
    /// Reserved in Snowflake only; a plain identifier under Databricks. These are Snowflake-specific
    /// DDL/feature/scripting words that are absent from the Spark SQL keyword table.
    SnowflakeOnly,
    /// Reserved in Databricks only; a plain identifier under Snowflake. (None today — the variant
    /// exists so a Databricks-specific reserved word can be added without reshaping the model.)
    #[allow(dead_code)]
    DatabricksOnly,
}

impl KeywordDialect {
    /// Does `dialect` reserve a keyword carrying this classification?
    #[inline]
    #[must_use]
    pub fn reserved_in(self, dialect: Dialect) -> bool {
        match self {
            KeywordDialect::Shared => true,
            KeywordDialect::SnowflakeOnly => matches!(dialect, Dialect::Snowflake),
            KeywordDialect::DatabricksOnly => matches!(dialect, Dialect::Databricks),
        }
    }
}

/// The single source of truth for keyword recognition: `(lowercase text, kind, dialect class)`.
///
/// Each entry's text must be ASCII lowercase (lookups lowercase the input before matching). The
/// `every_keyword_variant_is_mapped` test asserts this list covers exactly the `SyntaxKind` keyword
/// block, so a keyword cannot be added to the enum without a matching entry (and dialect class)
/// here.
const KEYWORDS: &[(&str, SyntaxKind, KeywordDialect)] = {
    use KeywordDialect::{Shared, SnowflakeOnly};
    use SyntaxKind::*;
    &[
        ("select", SELECT_KW, Shared),
        ("from", FROM_KW, Shared),
        ("where", WHERE_KW, Shared),
        ("group", GROUP_KW, Shared),
        ("by", BY_KW, Shared),
        ("having", HAVING_KW, Shared),
        ("order", ORDER_KW, Shared),
        ("limit", LIMIT_KW, Shared),
        ("offset", OFFSET_KW, Shared),
        ("fetch", FETCH_KW, Shared),
        // Snowflake's row-limiting `TOP n`; not a Spark keyword.
        ("top", TOP_KW, SnowflakeOnly),
        ("as", AS_KW, Shared),
        ("and", AND_KW, Shared),
        ("or", OR_KW, Shared),
        ("not", NOT_KW, Shared),
        ("null", NULL_KW, Shared),
        ("is", IS_KW, Shared),
        ("in", IN_KW, Shared),
        ("like", LIKE_KW, Shared),
        // ILIKE/RLIKE/REGEXP: Snowflake operators; non-reserved in Spark, so identifiers there.
        ("ilike", ILIKE_KW, SnowflakeOnly),
        ("rlike", RLIKE_KW, SnowflakeOnly),
        ("regexp", REGEXP_KW, SnowflakeOnly),
        ("between", BETWEEN_KW, Shared),
        ("case", CASE_KW, Shared),
        ("when", WHEN_KW, Shared),
        ("then", THEN_KW, Shared),
        ("else", ELSE_KW, Shared),
        ("end", END_KW, Shared),
        ("join", JOIN_KW, Shared),
        ("inner", INNER_KW, Shared),
        ("left", LEFT_KW, Shared),
        ("right", RIGHT_KW, Shared),
        ("full", FULL_KW, Shared),
        ("outer", OUTER_KW, Shared),
        ("cross", CROSS_KW, Shared),
        ("lateral", LATERAL_KW, Shared),
        ("natural", NATURAL_KW, Shared),
        ("on", ON_KW, Shared),
        ("using", USING_KW, Shared),
        ("with", WITH_KW, Shared),
        ("recursive", RECURSIVE_KW, Shared),
        ("union", UNION_KW, Shared),
        ("all", ALL_KW, Shared),
        ("any", ANY_KW, Shared),
        ("except", EXCEPT_KW, Shared),
        ("intersect", INTERSECT_KW, Shared),
        ("minus", MINUS_KW, Shared),
        ("distinct", DISTINCT_KW, Shared),
        // QUALIFY: a window-filter clause in BOTH dialects. Databricks SQL supports `SELECT ...
        // QUALIFY <predicate>` (Databricks Runtime 10.4 LTS+), so it must stay reserved under
        // Databricks too — otherwise the parser treats it as a plain identifier and mis-splits the
        // query. Reserving it in both dialects leaves Snowflake byte-identical (it was reserved
        // there already).
        ("qualify", QUALIFY_KW, Shared),
        ("over", OVER_KW, Shared),
        ("partition", PARTITION_KW, Shared),
        ("window", WINDOW_KW, Shared),
        ("rows", ROWS_KW, Shared),
        ("range", RANGE_KW, Shared),
        ("unbounded", UNBOUNDED_KW, Shared),
        ("preceding", PRECEDING_KW, Shared),
        ("following", FOLLOWING_KW, Shared),
        ("current", CURRENT_KW, Shared),
        ("row", ROW_KW, Shared),
        ("asc", ASC_KW, Shared),
        ("desc", DESC_KW, Shared),
        ("nulls", NULLS_KW, Shared),
        ("first", FIRST_KW, Shared),
        ("last", LAST_KW, Shared),
        ("true", TRUE_KW, Shared),
        ("false", FALSE_KW, Shared),
        ("cast", CAST_KW, Shared),
        // TRY_CAST is the function `try_cast(...)` in Spark, not a structural keyword.
        ("try_cast", TRY_CAST_KW, SnowflakeOnly),
        ("exists", EXISTS_KW, Shared),
        ("values", VALUES_KW, Shared),
        ("pivot", PIVOT_KW, Shared),
        ("unpivot", UNPIVOT_KW, Shared),
        // SAMPLE: Snowflake spelling; absent from the Spark keyword table (`TABLESAMPLE` is shared).
        ("sample", SAMPLE_KW, SnowflakeOnly),
        ("tablesample", TABLESAMPLE_KW, Shared),
        ("create", CREATE_KW, Shared),
        ("replace", REPLACE_KW, Shared),
        ("if", IF_KW, Shared),
        ("table", TABLE_KW, Shared),
        ("view", VIEW_KW, Shared),
        ("temporary", TEMPORARY_KW, Shared),
        ("temp", TEMP_KW, Shared),
        // Snowflake table-property words; not Spark keywords.
        ("transient", TRANSIENT_KW, SnowflakeOnly),
        ("volatile", VOLATILE_KW, SnowflakeOnly),
        ("secure", SECURE_KW, SnowflakeOnly),
        ("insert", INSERT_KW, Shared),
        ("into", INTO_KW, Shared),
        ("update", UPDATE_KW, Shared),
        ("delete", DELETE_KW, Shared),
        ("merge", MERGE_KW, Shared),
        ("set", SET_KW, Shared),
        // FLATTEN: Snowflake table function; not a Spark keyword.
        ("flatten", FLATTEN_KW, SnowflakeOnly),
        // CONNECT/PRIOR: Snowflake hierarchical `CONNECT BY`; absent from the Spark keyword table.
        ("connect", CONNECT_KW, SnowflakeOnly),
        ("start", START_KW, Shared),
        ("prior", PRIOR_KW, SnowflakeOnly),
        ("language", LANGUAGE_KW, Shared),
        // JAVASCRIPT/SCALA: Snowflake `LANGUAGE` values, not Spark keywords. (JAVA/PYTHON/SQL also
        // serve as type/language words common to both, so they stay shared.)
        ("javascript", JAVASCRIPT_KW, SnowflakeOnly),
        ("python", PYTHON_KW, Shared),
        ("java", JAVA_KW, Shared),
        ("scala", SCALA_KW, SnowflakeOnly),
        ("sql", SQL_KW, Shared),
        ("begin", BEGIN_KW, Shared),
        ("declare", DECLARE_KW, Shared),
        ("let", LET_KW, Shared),
        ("return", RETURN_KW, Shared),
        ("call", CALL_KW, Shared),
        ("procedure", PROCEDURE_KW, Shared),
        ("function", FUNCTION_KW, Shared),
        ("returns", RETURNS_KW, Shared),
        // Snowflake object DDL / scripting words absent from the Spark keyword table.
        ("task", TASK_KW, SnowflakeOnly),
        ("warehouse", WAREHOUSE_KW, SnowflakeOnly),
        ("schedule", SCHEDULE_KW, SnowflakeOnly),
        ("after", AFTER_KW, Shared),
        ("copy", COPY_KW, SnowflakeOnly),
        ("grants", GRANTS_KW, SnowflakeOnly),
        ("handler", HANDLER_KW, SnowflakeOnly),
        ("packages", PACKAGES_KW, SnowflakeOnly),
        ("imports", IMPORTS_KW, SnowflakeOnly),
        ("runtime_version", RUNTIME_VERSION_KW, SnowflakeOnly),
        ("execute", EXECUTE_KW, Shared),
        ("owner", OWNER_KW, SnowflakeOnly),
        ("caller", CALLER_KW, SnowflakeOnly),
        ("strict", STRICT_KW, SnowflakeOnly),
        ("called", CALLED_KW, SnowflakeOnly),
        ("input", INPUT_KW, Shared),
        ("output", OUTPUT_KW, Shared),
        ("out", OUT_KW, Shared),
        ("matched", MATCHED_KW, Shared),
        ("drop", DROP_KW, Shared),
        ("alter", ALTER_KW, Shared),
        ("within", WITHIN_KW, Shared),
        ("for", FOR_KW, Shared),
        ("immediate", IMMEDIATE_KW, SnowflakeOnly),
        ("overwrite", OVERWRITE_KW, Shared),
        ("grant", GRANT_KW, Shared),
        ("revoke", REVOKE_KW, Shared),
        ("use", USE_KW, Shared),
        ("show", SHOW_KW, Shared),
        ("describe", DESCRIBE_KW, Shared),
        ("truncate", TRUNCATE_KW, Shared),
        ("commit", COMMIT_KW, Shared),
        ("rollback", ROLLBACK_KW, Shared),
        // Snowflake scripting / object words absent from the Spark keyword table.
        ("undrop", UNDROP_KW, SnowflakeOnly),
        ("elseif", ELSEIF_KW, SnowflakeOnly),
        ("while", WHILE_KW, Shared),
        ("loop", LOOP_KW, Shared),
        ("repeat", REPEAT_KW, Shared),
        ("until", UNTIL_KW, Shared),
        ("do", DO_KW, Shared),
        ("exception", EXCEPTION_KW, SnowflakeOnly),
        ("cursor", CURSOR_KW, SnowflakeOnly),
        ("resultset", RESULTSET_KW, SnowflakeOnly),
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
fn lookup(ident: &str) -> Option<(SyntaxKind, KeywordDialect)> {
    let mut buf = [0u8; MAX_KEYWORD_LEN];
    let len = lower_for_lookup(ident, &mut buf)?;
    let lower = std::str::from_utf8(&buf[..len]).ok()?;
    KEYWORDS
        .iter()
        .find(|(text, _, _)| *text == lower)
        .map(|(_, kind, dialect)| (*kind, *dialect))
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
/// identifier. Shared keywords behave identically in every dialect, so under
/// [`Dialect::Snowflake`] this is byte-for-byte equivalent to [`keyword_kind`].
#[must_use]
pub fn keyword_kind_for(ident: &str, dialect: Dialect) -> Option<SyntaxKind> {
    let (kind, kw_dialect) = lookup(ident)?;
    kw_dialect.reserved_in(dialect).then_some(kind)
}

#[cfg(test)]
mod tests {
    use super::{keyword_kind, keyword_kind_for, KeywordDialect, KEYWORDS};
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
        for (text, kind, _) in KEYWORDS {
            assert_eq!(
                keyword_kind(text),
                Some(*kind),
                "keyword_kind({text:?}) is wrong"
            );
            assert_eq!(
                keyword_kind(&text.to_uppercase()),
                Some(*kind),
                "keyword_kind is not case-insensitive for {text:?}"
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
    fn every_keyword_has_a_dialect_classification() {
        // Completeness, dialect dimension: every keyword in the table carries one of the three
        // classifications, so the reservation set cannot silently drift as keywords are added.
        for (text, kind, dialect) in KEYWORDS {
            assert!(
                matches!(
                    dialect,
                    KeywordDialect::Shared
                        | KeywordDialect::SnowflakeOnly
                        | KeywordDialect::DatabricksOnly
                ),
                "{text:?} ({kind:?}) has no dialect classification"
            );
        }
    }

    #[test]
    fn snowflake_classification_is_byte_identical_to_legacy_keyword_kind() {
        // Under Snowflake, the dialect-aware lookup must agree with the plain `keyword_kind` for
        // every keyword — the regression guard that Snowflake reservation is unchanged.
        for (text, kind, _) in KEYWORDS {
            assert_eq!(
                keyword_kind_for(text, Dialect::Snowflake),
                Some(*kind),
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
}
