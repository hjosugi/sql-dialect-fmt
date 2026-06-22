//! The [`SyntaxKind`] enumeration and its `u16` conversions / predicates.

/// Every lexical token kind and syntax node kind understood by snow-fmt.
///
/// Ordering is significant: variants are contiguous from `0`, which makes
/// [`SyntaxKind::from_u16`] a checked `transmute`. The `__KW_START` / `__KW_END`
/// sentinels bracket the keyword block so [`SyntaxKind::is_keyword`] is a range check.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u16)]
pub enum SyntaxKind {
    // ---- Trivia (preserved verbatim in the lossless tree) ----
    WHITESPACE = 0,
    NEWLINE,
    COMMENT,       // -- line  or  // line
    BLOCK_COMMENT, // /* ... */

    // ---- Literals & names ----
    IDENT,         // unquoted identifier (also covers un-resolved keywords; see keyword_kind)
    QUOTED_IDENT,  // "quoted identifier"
    STRING,        // 'string literal'
    DOLLAR_STRING, // delimited body token; current Snowflake delimiter is $$ ... $$
    INT_NUMBER,
    FLOAT_NUMBER,
    VARIABLE, // $1, $42 (positional)  or  $name (session/binding)

    // ---- Punctuation & operators ----
    L_PAREN,   // (
    R_PAREN,   // )
    L_BRACKET, // [
    R_BRACKET, // ]
    L_BRACE,   // {
    R_BRACE,   // }
    COMMA,     // ,
    DOT,       // .
    SEMICOLON, // ;
    COLON,     // :   (semi-structured path access, named args in some dialects)
    COLON2,    // ::  (cast)
    ASSIGN,    // :=  (Snowflake Scripting assignment)
    EQ,        // =
    NEQ,       // <> or !=
    LT,        // <
    LTE,       // <=
    GT,        // >
    GTE,       // >=
    PLUS,      // +
    MINUS,     // -
    STAR,      // *
    SLASH,     // /
    PERCENT,   // %
    CONCAT,    // ||
    PIPE,      // |
    PIPE_GT,   // |>  (GoogleSQL-style pipe; kept for compatibility and corpus coverage)
    FLOW_PIPE, // ->> (Snowflake flow / pipe operator)
    ARROW,     // ->  (lambda)
    FAT_ARROW, // =>  (named argument)
    AMP,       // &
    CARET,     // ^
    TILDE,     // ~
    AT,        // @   (stage reference)
    DOLLAR,    // $   (lone dollar, not a variable or $$ )
    QUESTION,  // ?   (bind marker)
    BANG,      // !   (only valid as part of !=; standalone is an error)

    // ---- Keywords (case-insensitive; recognized via keyword_kind) ----
    // NOTE: this is an intentionally partial but representative set covering the SELECT
    // pipeline, common DDL/DML, Snowflake Scripting, and embedded-language declarations.
    // It will grow phase-by-phase as parser coverage expands.
    #[doc(hidden)]
    __KW_START,
    SELECT_KW,
    FROM_KW,
    WHERE_KW,
    GROUP_KW,
    BY_KW,
    HAVING_KW,
    ORDER_KW,
    LIMIT_KW,
    OFFSET_KW,
    FETCH_KW,
    TOP_KW,
    AS_KW,
    AND_KW,
    OR_KW,
    NOT_KW,
    NULL_KW,
    IS_KW,
    IN_KW,
    LIKE_KW,
    ILIKE_KW,
    RLIKE_KW,
    REGEXP_KW,
    BETWEEN_KW,
    CASE_KW,
    WHEN_KW,
    THEN_KW,
    ELSE_KW,
    END_KW,
    JOIN_KW,
    INNER_KW,
    LEFT_KW,
    RIGHT_KW,
    FULL_KW,
    OUTER_KW,
    CROSS_KW,
    LATERAL_KW,
    NATURAL_KW,
    ON_KW,
    USING_KW,
    WITH_KW,
    RECURSIVE_KW,
    UNION_KW,
    ALL_KW,
    ANY_KW,
    EXCEPT_KW,
    INTERSECT_KW,
    MINUS_KW,
    DISTINCT_KW,
    QUALIFY_KW,
    OVER_KW,
    PARTITION_KW,
    WINDOW_KW,
    ROWS_KW,
    RANGE_KW,
    UNBOUNDED_KW,
    PRECEDING_KW,
    FOLLOWING_KW,
    CURRENT_KW,
    ROW_KW,
    ASC_KW,
    DESC_KW,
    NULLS_KW,
    FIRST_KW,
    LAST_KW,
    TRUE_KW,
    FALSE_KW,
    CAST_KW,
    TRY_CAST_KW,
    EXISTS_KW,
    VALUES_KW,
    PIVOT_KW,
    UNPIVOT_KW,
    SAMPLE_KW,
    TABLESAMPLE_KW,
    CREATE_KW,
    REPLACE_KW,
    IF_KW,
    TABLE_KW,
    VIEW_KW,
    TEMPORARY_KW,
    TEMP_KW,
    TRANSIENT_KW,
    VOLATILE_KW,
    SECURE_KW,
    INSERT_KW,
    INTO_KW,
    UPDATE_KW,
    DELETE_KW,
    MERGE_KW,
    SET_KW,
    FLATTEN_KW,
    CONNECT_KW,
    START_KW,
    PRIOR_KW,
    LANGUAGE_KW,
    JAVASCRIPT_KW,
    PYTHON_KW,
    JAVA_KW,
    SCALA_KW,
    SQL_KW,
    BEGIN_KW,
    DECLARE_KW,
    LET_KW,
    RETURN_KW,
    CALL_KW,
    PROCEDURE_KW,
    FUNCTION_KW,
    RETURNS_KW,
    TASK_KW,
    WAREHOUSE_KW,
    SCHEDULE_KW,
    AFTER_KW,
    COPY_KW,
    GRANTS_KW,
    HANDLER_KW,
    PACKAGES_KW,
    IMPORTS_KW,
    RUNTIME_VERSION_KW,
    EXECUTE_KW,
    OWNER_KW,
    CALLER_KW,
    STRICT_KW,
    CALLED_KW,
    INPUT_KW,
    OUTPUT_KW,
    OUT_KW,
    MATCHED_KW,
    DROP_KW,
    ALTER_KW,
    WITHIN_KW,
    FOR_KW,
    IMMEDIATE_KW,
    #[doc(hidden)]
    __KW_END,

    // ---- Node kinds ----
    SOURCE_FILE,
    ERROR,
    EOF,
    // statements
    SELECT_STMT,
    EXPR_STMT,
    // clauses & fragments
    SELECT_LIST,
    SELECT_ITEM,
    FROM_CLAUSE,
    WHERE_CLAUSE,
    TABLE_REF,
    ARG_LIST,
    TYPE_NAME,
    NAME,
    NAME_REF,
    // expressions
    LITERAL,
    STAR_EXPR,
    PAREN_EXPR,
    PREFIX_EXPR,
    BIN_EXPR,
    CALL_EXPR,
    INDEX_EXPR,
    CAST_EXPR,
    // queries & set operations (Phase 2)
    WITH_QUERY,
    WITH_CLAUSE,
    CTE,
    COLUMN_LIST,
    SET_OP,
    SUBQUERY,
    // clauses (Phase 2)
    GROUP_BY_CLAUSE,
    HAVING_CLAUSE,
    QUALIFY_CLAUSE,
    ORDER_BY_CLAUSE,
    ORDER_BY_ITEM,
    LIMIT_CLAUSE,
    OFFSET_CLAUSE,
    JOIN,
    // predicates (Phase 2)
    IS_EXPR,
    IN_EXPR,
    BETWEEN_EXPR,
    EXISTS_EXPR,
    EXPR_LIST,
    // window functions (Phase 2)
    WINDOW_EXPR,
    WINDOW_SPEC,
    PARTITION_BY_CLAUSE,
    WINDOW_FRAME,
    // Phase 2b: CASE / CAST(...) / semi-structured path / VALUES
    CASE_EXPR,
    CASE_WHEN,
    JSON_ACCESS,
    VALUES_CLAUSE,
    VALUES_ROW,
    // Phase 6: DML statements
    INSERT_STMT,
    UPDATE_STMT,
    DELETE_STMT,
    MERGE_STMT,
    SET_CLAUSE,
    ASSIGNMENT,
    MERGE_WHEN,
    // Phase 7: DDL statements
    CREATE_STMT,
    DROP_STMT,
    ALTER_STMT,
    COLUMN_DEF_LIST,
    COLUMN_DEF,
    // Phase 4: Snowflake query extensions
    WITHIN_GROUP,
    PIVOT_CLAUSE,
    NAMED_ARG,
    MATCH_RECOGNIZE,
    // Phase 8 / scripting-adjacent statements
    SET_STMT,
    EXECUTE_STMT,
    GROUPING_SETS,
    // Phase 6: COPY INTO
    COPY_STMT,
    COPY_LOCATION,
    COPY_OPTION,

    #[doc(hidden)]
    __LAST,
}

impl SyntaxKind {
    /// Raw `u16` discriminant (as stored by the rowan green tree).
    #[inline]
    pub const fn to_u16(self) -> u16 {
        self as u16
    }

    /// Reconstruct a [`SyntaxKind`] from its raw discriminant.
    ///
    /// # Panics
    /// Panics if `raw` is out of range. Because the enum is contiguous and `#[repr(u16)]`,
    /// any in-range value corresponds to a real variant, so the `transmute` is sound.
    #[inline]
    pub fn from_u16(raw: u16) -> SyntaxKind {
        assert!(
            raw <= SyntaxKind::__LAST as u16,
            "SyntaxKind out of range: {raw}"
        );
        // SAFETY: variants are contiguous `0..=__LAST` with `#[repr(u16)]`, and we just
        // bounds-checked `raw`, so it names a valid discriminant.
        unsafe { std::mem::transmute::<u16, SyntaxKind>(raw) }
    }

    /// Whitespace, newlines, and comments — preserved but ignored by the grammar.
    #[inline]
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::WHITESPACE
                | SyntaxKind::NEWLINE
                | SyntaxKind::COMMENT
                | SyntaxKind::BLOCK_COMMENT
        )
    }

    /// `--` / `//` line comments and `/* */` block comments.
    #[inline]
    pub const fn is_comment(self) -> bool {
        matches!(self, SyntaxKind::COMMENT | SyntaxKind::BLOCK_COMMENT)
    }

    /// True for any reserved/keyword kind (the block between the sentinels).
    #[inline]
    pub fn is_keyword(self) -> bool {
        let v = self as u16;
        v > SyntaxKind::__KW_START as u16 && v < SyntaxKind::__KW_END as u16
    }
}

#[cfg(test)]
mod tests {
    use super::SyntaxKind;

    #[test]
    fn keyword_and_trivia_predicates() {
        assert!(SyntaxKind::SELECT_KW.is_keyword());
        assert!(SyntaxKind::QUALIFY_KW.is_keyword());
        assert!(!SyntaxKind::IDENT.is_keyword());
        assert!(!SyntaxKind::PIPE_GT.is_keyword());
        assert!(SyntaxKind::WHITESPACE.is_trivia());
        assert!(SyntaxKind::BLOCK_COMMENT.is_trivia());
        assert!(SyntaxKind::COMMENT.is_comment());
        assert!(!SyntaxKind::IDENT.is_trivia());
    }

    #[test]
    fn u16_roundtrip_is_total() {
        // Confirms the enum is contiguous and from_u16/to_u16 are inverses for every kind.
        for raw in 0..=SyntaxKind::__LAST.to_u16() {
            let kind = SyntaxKind::from_u16(raw);
            assert_eq!(kind.to_u16(), raw);
        }
    }

    #[test]
    #[should_panic]
    fn from_u16_out_of_range_panics() {
        let _ = SyntaxKind::from_u16(u16::MAX);
    }
}
