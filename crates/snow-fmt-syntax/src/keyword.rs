//! Case-insensitive recognition of keyword text.

use crate::SyntaxKind;

/// Map an identifier's text to its keyword kind, case-insensitively.
///
/// Snowflake folds unquoted identifiers and matches keywords without regard to case. Returns
/// `None` for plain identifiers — the lexer emits [`SyntaxKind::IDENT`] for every word and the
/// parser uses this to reclassify keywords contextually.
///
// Allocation-free: the longest keyword is 11 bytes ("tablesample"), so we lowercase into a
// fixed stack buffer and match the resulting `&str`. A perfect hash (phf) could shave more
// once the keyword set stabilizes, but this already keeps the lexer's hot path off the heap.
pub fn keyword_kind(ident: &str) -> Option<SyntaxKind> {
    use SyntaxKind::*;
    const MAX_KEYWORD_LEN: usize = 16;
    let bytes = ident.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_KEYWORD_LEN {
        return None;
    }
    let mut buf = [0u8; MAX_KEYWORD_LEN];
    for (slot, &b) in buf.iter_mut().zip(bytes) {
        *slot = b.to_ascii_lowercase();
    }
    // ASCII-lowercasing a valid `&str` byte-by-byte yields valid UTF-8, so this never errors.
    let lower = match std::str::from_utf8(&buf[..bytes.len()]) {
        Ok(s) => s,
        Err(_) => return None,
    };
    Some(match lower {
        "select" => SELECT_KW,
        "from" => FROM_KW,
        "where" => WHERE_KW,
        "group" => GROUP_KW,
        "by" => BY_KW,
        "having" => HAVING_KW,
        "order" => ORDER_KW,
        "limit" => LIMIT_KW,
        "offset" => OFFSET_KW,
        "fetch" => FETCH_KW,
        "top" => TOP_KW,
        "as" => AS_KW,
        "and" => AND_KW,
        "or" => OR_KW,
        "not" => NOT_KW,
        "null" => NULL_KW,
        "is" => IS_KW,
        "in" => IN_KW,
        "like" => LIKE_KW,
        "ilike" => ILIKE_KW,
        "rlike" => RLIKE_KW,
        "regexp" => REGEXP_KW,
        "between" => BETWEEN_KW,
        "case" => CASE_KW,
        "when" => WHEN_KW,
        "then" => THEN_KW,
        "else" => ELSE_KW,
        "end" => END_KW,
        "join" => JOIN_KW,
        "inner" => INNER_KW,
        "left" => LEFT_KW,
        "right" => RIGHT_KW,
        "full" => FULL_KW,
        "outer" => OUTER_KW,
        "cross" => CROSS_KW,
        "lateral" => LATERAL_KW,
        "natural" => NATURAL_KW,
        "on" => ON_KW,
        "using" => USING_KW,
        "with" => WITH_KW,
        "recursive" => RECURSIVE_KW,
        "union" => UNION_KW,
        "all" => ALL_KW,
        "any" => ANY_KW,
        "except" => EXCEPT_KW,
        "intersect" => INTERSECT_KW,
        "minus" => MINUS_KW,
        "distinct" => DISTINCT_KW,
        "qualify" => QUALIFY_KW,
        "over" => OVER_KW,
        "partition" => PARTITION_KW,
        "window" => WINDOW_KW,
        "rows" => ROWS_KW,
        "range" => RANGE_KW,
        "unbounded" => UNBOUNDED_KW,
        "preceding" => PRECEDING_KW,
        "following" => FOLLOWING_KW,
        "current" => CURRENT_KW,
        "row" => ROW_KW,
        "asc" => ASC_KW,
        "desc" => DESC_KW,
        "nulls" => NULLS_KW,
        "first" => FIRST_KW,
        "last" => LAST_KW,
        "true" => TRUE_KW,
        "false" => FALSE_KW,
        "cast" => CAST_KW,
        "try_cast" => TRY_CAST_KW,
        "exists" => EXISTS_KW,
        "values" => VALUES_KW,
        "pivot" => PIVOT_KW,
        "unpivot" => UNPIVOT_KW,
        "sample" => SAMPLE_KW,
        "tablesample" => TABLESAMPLE_KW,
        "create" => CREATE_KW,
        "replace" => REPLACE_KW,
        "if" => IF_KW,
        "table" => TABLE_KW,
        "view" => VIEW_KW,
        "temporary" => TEMPORARY_KW,
        "temp" => TEMP_KW,
        "transient" => TRANSIENT_KW,
        "volatile" => VOLATILE_KW,
        "secure" => SECURE_KW,
        "insert" => INSERT_KW,
        "into" => INTO_KW,
        "update" => UPDATE_KW,
        "delete" => DELETE_KW,
        "merge" => MERGE_KW,
        "set" => SET_KW,
        "flatten" => FLATTEN_KW,
        "connect" => CONNECT_KW,
        "start" => START_KW,
        "prior" => PRIOR_KW,
        "language" => LANGUAGE_KW,
        "javascript" => JAVASCRIPT_KW,
        "python" => PYTHON_KW,
        "java" => JAVA_KW,
        "scala" => SCALA_KW,
        "sql" => SQL_KW,
        "begin" => BEGIN_KW,
        "declare" => DECLARE_KW,
        "let" => LET_KW,
        "return" => RETURN_KW,
        "call" => CALL_KW,
        "procedure" => PROCEDURE_KW,
        "function" => FUNCTION_KW,
        "returns" => RETURNS_KW,
        "task" => TASK_KW,
        "warehouse" => WAREHOUSE_KW,
        "schedule" => SCHEDULE_KW,
        "after" => AFTER_KW,
        "copy" => COPY_KW,
        "grants" => GRANTS_KW,
        "handler" => HANDLER_KW,
        "packages" => PACKAGES_KW,
        "imports" => IMPORTS_KW,
        "runtime_version" => RUNTIME_VERSION_KW,
        "execute" => EXECUTE_KW,
        "owner" => OWNER_KW,
        "caller" => CALLER_KW,
        "strict" => STRICT_KW,
        "called" => CALLED_KW,
        "input" => INPUT_KW,
        "output" => OUTPUT_KW,
        "out" => OUT_KW,
        "matched" => MATCHED_KW,
        "drop" => DROP_KW,
        "alter" => ALTER_KW,
        "within" => WITHIN_KW,
        "for" => FOR_KW,
        "immediate" => IMMEDIATE_KW,
        "overwrite" => OVERWRITE_KW,
        "grant" => GRANT_KW,
        "revoke" => REVOKE_KW,
        "use" => USE_KW,
        "show" => SHOW_KW,
        "describe" => DESCRIBE_KW,
        "truncate" => TRUNCATE_KW,
        "commit" => COMMIT_KW,
        "rollback" => ROLLBACK_KW,
        "undrop" => UNDROP_KW,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::keyword_kind;
    use crate::SyntaxKind;

    /// Mirror of the `keyword_kind` match arms. The test below asserts this stays in sync with
    /// the `SyntaxKind` keyword block, so a new keyword cannot be added to one without the other.
    const KEYWORDS: &[(&str, SyntaxKind)] = &[
        ("select", SyntaxKind::SELECT_KW),
        ("from", SyntaxKind::FROM_KW),
        ("where", SyntaxKind::WHERE_KW),
        ("group", SyntaxKind::GROUP_KW),
        ("by", SyntaxKind::BY_KW),
        ("having", SyntaxKind::HAVING_KW),
        ("order", SyntaxKind::ORDER_KW),
        ("limit", SyntaxKind::LIMIT_KW),
        ("offset", SyntaxKind::OFFSET_KW),
        ("fetch", SyntaxKind::FETCH_KW),
        ("top", SyntaxKind::TOP_KW),
        ("as", SyntaxKind::AS_KW),
        ("and", SyntaxKind::AND_KW),
        ("or", SyntaxKind::OR_KW),
        ("not", SyntaxKind::NOT_KW),
        ("null", SyntaxKind::NULL_KW),
        ("is", SyntaxKind::IS_KW),
        ("in", SyntaxKind::IN_KW),
        ("like", SyntaxKind::LIKE_KW),
        ("ilike", SyntaxKind::ILIKE_KW),
        ("rlike", SyntaxKind::RLIKE_KW),
        ("regexp", SyntaxKind::REGEXP_KW),
        ("between", SyntaxKind::BETWEEN_KW),
        ("case", SyntaxKind::CASE_KW),
        ("when", SyntaxKind::WHEN_KW),
        ("then", SyntaxKind::THEN_KW),
        ("else", SyntaxKind::ELSE_KW),
        ("end", SyntaxKind::END_KW),
        ("join", SyntaxKind::JOIN_KW),
        ("inner", SyntaxKind::INNER_KW),
        ("left", SyntaxKind::LEFT_KW),
        ("right", SyntaxKind::RIGHT_KW),
        ("full", SyntaxKind::FULL_KW),
        ("outer", SyntaxKind::OUTER_KW),
        ("cross", SyntaxKind::CROSS_KW),
        ("lateral", SyntaxKind::LATERAL_KW),
        ("natural", SyntaxKind::NATURAL_KW),
        ("on", SyntaxKind::ON_KW),
        ("using", SyntaxKind::USING_KW),
        ("with", SyntaxKind::WITH_KW),
        ("recursive", SyntaxKind::RECURSIVE_KW),
        ("union", SyntaxKind::UNION_KW),
        ("all", SyntaxKind::ALL_KW),
        ("any", SyntaxKind::ANY_KW),
        ("except", SyntaxKind::EXCEPT_KW),
        ("intersect", SyntaxKind::INTERSECT_KW),
        ("minus", SyntaxKind::MINUS_KW),
        ("distinct", SyntaxKind::DISTINCT_KW),
        ("qualify", SyntaxKind::QUALIFY_KW),
        ("over", SyntaxKind::OVER_KW),
        ("partition", SyntaxKind::PARTITION_KW),
        ("window", SyntaxKind::WINDOW_KW),
        ("rows", SyntaxKind::ROWS_KW),
        ("range", SyntaxKind::RANGE_KW),
        ("unbounded", SyntaxKind::UNBOUNDED_KW),
        ("preceding", SyntaxKind::PRECEDING_KW),
        ("following", SyntaxKind::FOLLOWING_KW),
        ("current", SyntaxKind::CURRENT_KW),
        ("row", SyntaxKind::ROW_KW),
        ("asc", SyntaxKind::ASC_KW),
        ("desc", SyntaxKind::DESC_KW),
        ("nulls", SyntaxKind::NULLS_KW),
        ("first", SyntaxKind::FIRST_KW),
        ("last", SyntaxKind::LAST_KW),
        ("true", SyntaxKind::TRUE_KW),
        ("false", SyntaxKind::FALSE_KW),
        ("cast", SyntaxKind::CAST_KW),
        ("try_cast", SyntaxKind::TRY_CAST_KW),
        ("exists", SyntaxKind::EXISTS_KW),
        ("values", SyntaxKind::VALUES_KW),
        ("pivot", SyntaxKind::PIVOT_KW),
        ("unpivot", SyntaxKind::UNPIVOT_KW),
        ("sample", SyntaxKind::SAMPLE_KW),
        ("tablesample", SyntaxKind::TABLESAMPLE_KW),
        ("create", SyntaxKind::CREATE_KW),
        ("replace", SyntaxKind::REPLACE_KW),
        ("if", SyntaxKind::IF_KW),
        ("table", SyntaxKind::TABLE_KW),
        ("view", SyntaxKind::VIEW_KW),
        ("temporary", SyntaxKind::TEMPORARY_KW),
        ("temp", SyntaxKind::TEMP_KW),
        ("transient", SyntaxKind::TRANSIENT_KW),
        ("volatile", SyntaxKind::VOLATILE_KW),
        ("secure", SyntaxKind::SECURE_KW),
        ("insert", SyntaxKind::INSERT_KW),
        ("into", SyntaxKind::INTO_KW),
        ("update", SyntaxKind::UPDATE_KW),
        ("delete", SyntaxKind::DELETE_KW),
        ("merge", SyntaxKind::MERGE_KW),
        ("set", SyntaxKind::SET_KW),
        ("flatten", SyntaxKind::FLATTEN_KW),
        ("connect", SyntaxKind::CONNECT_KW),
        ("start", SyntaxKind::START_KW),
        ("prior", SyntaxKind::PRIOR_KW),
        ("language", SyntaxKind::LANGUAGE_KW),
        ("javascript", SyntaxKind::JAVASCRIPT_KW),
        ("python", SyntaxKind::PYTHON_KW),
        ("java", SyntaxKind::JAVA_KW),
        ("scala", SyntaxKind::SCALA_KW),
        ("sql", SyntaxKind::SQL_KW),
        ("begin", SyntaxKind::BEGIN_KW),
        ("declare", SyntaxKind::DECLARE_KW),
        ("let", SyntaxKind::LET_KW),
        ("return", SyntaxKind::RETURN_KW),
        ("call", SyntaxKind::CALL_KW),
        ("procedure", SyntaxKind::PROCEDURE_KW),
        ("function", SyntaxKind::FUNCTION_KW),
        ("returns", SyntaxKind::RETURNS_KW),
        ("task", SyntaxKind::TASK_KW),
        ("warehouse", SyntaxKind::WAREHOUSE_KW),
        ("schedule", SyntaxKind::SCHEDULE_KW),
        ("after", SyntaxKind::AFTER_KW),
        ("copy", SyntaxKind::COPY_KW),
        ("grants", SyntaxKind::GRANTS_KW),
        ("handler", SyntaxKind::HANDLER_KW),
        ("packages", SyntaxKind::PACKAGES_KW),
        ("imports", SyntaxKind::IMPORTS_KW),
        ("runtime_version", SyntaxKind::RUNTIME_VERSION_KW),
        ("execute", SyntaxKind::EXECUTE_KW),
        ("owner", SyntaxKind::OWNER_KW),
        ("caller", SyntaxKind::CALLER_KW),
        ("strict", SyntaxKind::STRICT_KW),
        ("called", SyntaxKind::CALLED_KW),
        ("input", SyntaxKind::INPUT_KW),
        ("output", SyntaxKind::OUTPUT_KW),
        ("out", SyntaxKind::OUT_KW),
        ("matched", SyntaxKind::MATCHED_KW),
        ("drop", SyntaxKind::DROP_KW),
        ("alter", SyntaxKind::ALTER_KW),
        ("within", SyntaxKind::WITHIN_KW),
        ("for", SyntaxKind::FOR_KW),
        ("immediate", SyntaxKind::IMMEDIATE_KW),
        ("overwrite", SyntaxKind::OVERWRITE_KW),
        ("grant", SyntaxKind::GRANT_KW),
        ("revoke", SyntaxKind::REVOKE_KW),
        ("use", SyntaxKind::USE_KW),
        ("show", SyntaxKind::SHOW_KW),
        ("describe", SyntaxKind::DESCRIBE_KW),
        ("truncate", SyntaxKind::TRUNCATE_KW),
        ("commit", SyntaxKind::COMMIT_KW),
        ("rollback", SyntaxKind::ROLLBACK_KW),
        ("undrop", SyntaxKind::UNDROP_KW),
    ];

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
        // Completeness: the test list must have exactly one entry per keyword enum variant.
        let range_count = SyntaxKind::__KW_END as u16 - SyntaxKind::__KW_START as u16 - 1;
        assert_eq!(
            KEYWORDS.len() as u16,
            range_count,
            "KEYWORDS test list is out of sync with the SyntaxKind keyword block"
        );
        let mut seen = std::collections::HashSet::new();
        for (text, kind) in KEYWORDS {
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
        }
    }
}
