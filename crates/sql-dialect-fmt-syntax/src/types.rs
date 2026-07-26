//! Canonical built-in SQL type words shared by highlighters and editor-facing completions.

/// Built-in type words recognized by sql-dialect-fmt.
///
/// Keep entries uppercase. Consumers that need case-insensitive matching should use
/// [`is_builtin_type`], and consumers that serialize editor regexes can lowercase these words.
pub const BUILTIN_TYPE_WORDS: &[&str] = &[
    "ARRAY",
    "BIGINT",
    "BINARY",
    "BOOLEAN",
    "BYTEINT",
    "CHAR",
    "CHARACTER",
    "DATE",
    "DATETIME",
    "DEC",
    "DECFLOAT",
    "DECIMAL",
    "DOUBLE",
    "FILE",
    "FLOAT",
    "FLOAT4",
    "FLOAT8",
    "GEOGRAPHY",
    "GEOMETRY",
    "INT",
    "INTEGER",
    "MAP",
    "NCHAR",
    "NUMBER",
    "NUMERIC",
    "NVARCHAR",
    "NVARCHAR2",
    "OBJECT",
    "REAL",
    "SMALLINT",
    "STRING",
    "TEXT",
    "TIME",
    "TIMESTAMP",
    "TIMESTAMP_LTZ",
    "TIMESTAMP_NTZ",
    "TIMESTAMP_TZ",
    "TINYINT",
    "UUID",
    "VARBINARY",
    "VARCHAR",
    "VARIANT",
    "VECTOR",
];

/// Case-insensitive built-in type lookup.
#[must_use]
pub fn is_builtin_type(text: &str) -> bool {
    BUILTIN_TYPE_WORDS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(text))
}

#[cfg(test)]
mod tests {
    use super::{is_builtin_type, BUILTIN_TYPE_WORDS};

    #[test]
    fn type_words_are_uppercase_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for word in BUILTIN_TYPE_WORDS {
            assert_eq!(*word, word.to_ascii_uppercase(), "{word}");
            assert!(seen.insert(*word), "duplicate built-in type {word}");
        }
    }

    #[test]
    fn type_lookup_is_case_insensitive() {
        assert!(is_builtin_type("NUMBER"));
        assert!(is_builtin_type("timestamp_ntz"));
        assert!(is_builtin_type("Variant"));
        assert!(!is_builtin_type("definitely_not_a_type"));
    }
}
