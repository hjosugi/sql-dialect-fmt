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
    OVERWRITE_KW,
    GRANT_KW,
    REVOKE_KW,
    USE_KW,
    SHOW_KW,
    DESCRIBE_KW,
    TRUNCATE_KW,
    COMMIT_KW,
    ROLLBACK_KW,
    UNDROP_KW,
    ELSEIF_KW,
    WHILE_KW,
    LOOP_KW,
    REPEAT_KW,
    UNTIL_KW,
    DO_KW,
    EXCEPTION_KW,
    CURSOR_KW,
    RESULTSET_KW,
    #[doc(hidden)]
    __KW_END,

    // ---- Node kinds ----
    SOURCE_FILE,
    ERROR,
    EOF,
    /// A *soft* (contextual) keyword token: a word that the grammar recognized as a keyword in a
    /// specific position (e.g. `ASOF`, `MATCH_RECOGNIZE`, `AT`/`BEFORE`, `GROUPING SETS`) but that
    /// is **not** reserved — elsewhere it is an ordinary identifier. Tagged via `bump_as`, it sits
    /// outside the keyword range (so it never reserves the word) yet lets the formatter upper-case
    /// it and the highlighter colour it like a keyword. See `parser::ContextualKeyword`.
    CONTEXTUAL_KEYWORD,
    // statements
    SELECT_STMT,
    EXPR_STMT,
    // clauses & fragments
    SELECT_LIST,
    SELECT_ITEM,
    FROM_CLAUSE,
    WHERE_CLAUSE,
    TABLE_REF,
    LATERAL_VIEW,
    AS_OF_TRAVEL,
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
    LAMBDA_EXPR,
    LAMBDA_PARAMS,
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
    GRANT_STMT,
    REVOKE_STMT,
    CALL_STMT,
    USE_STMT,
    SHOW_STMT,
    DESCRIBE_STMT,
    TRUNCATE_STMT,
    COMMENT_STMT,
    TRANSACTION_STMT,
    UNDROP_STMT,
    // Phase 8: Snowflake Scripting blocks
    BLOCK_STMT,
    DECLARE_SECTION,
    DECLARE_ITEM,
    STMT_LIST,
    EXCEPTION_SECTION,
    EXCEPTION_WHEN,
    LET_STMT,
    ASSIGN_STMT,
    RETURN_STMT,
    IF_STMT,
    LOOP_STMT,
    /// A procedural `CASE … END [CASE]` statement inside a block, distinct from `CASE_EXPR`.
    CASE_STMT,
    /// One `WHEN <test> THEN <stmts>` arm of a procedural `CASE_STMT`.
    CASE_STMT_WHEN,
    SCRIPT_STMT,
    COLUMN_DEF_LIST,
    COLUMN_DEF,
    // Phase 4: Snowflake query extensions
    WITHIN_GROUP,
    PIVOT_CLAUSE,
    NAMED_ARG,
    MATCH_RECOGNIZE,
    // MATCH_RECOGNIZE body sub-clauses
    MEASURES_CLAUSE,
    ROW_MATCH_CLAUSE,
    AFTER_MATCH_CLAUSE,
    PATTERN_CLAUSE,
    PATTERN_BODY,
    SUBSET_CLAUSE,
    DEFINE_CLAUSE,
    DEFINE_ITEM,
    // Hierarchical queries
    START_WITH_CLAUSE,
    CONNECT_BY_CLAUSE,
    // Flow / pipe operator: a chain of statements joined by `->>`
    FLOW_STMT,
    // Phase 8 / scripting-adjacent statements
    SET_STMT,
    EXECUTE_STMT,
    GROUPING_SETS,
    // Phase 6: COPY INTO
    COPY_STMT,
    COPY_LOCATION,
    COPY_OPTION,
    // A `@stage[/path]` reference used as a table/source (e.g. `FROM @s/p`, `COPY ... FROM @s`).
    STAGE_REF,
    // Phase 6: multi-table INSERT
    INTO_CLAUSE,
    INSERT_WHEN,
    // Phase 7: structural object DDL (CREATE SCHEMA/DATABASE/WAREHOUSE/STAGE/FILE FORMAT/SEQUENCE/
    // STREAM/TASK/DYNAMIC TABLE) and access control (GRANT/REVOKE).
    OBJECT_PROPERTY, // one `KEY = value`, `KEY = ( ... )`, or bare flag word property
    STREAM_SOURCE,   // a stream's `ON { TABLE | VIEW | STAGE } <name> [AT|BEFORE ( ... )]` source
    TASK_AFTER,      // a task's `AFTER <pred> [, <pred>]*` predecessor list
    SEMANTIC_VIEW_CLAUSE, // a top-level `CREATE SEMANTIC VIEW` clause (`TABLES (...)`, `METRICS (...)`, ...)
    SEMANTIC_VIEW_ITEM,   // one top-level item inside a semantic-view parenthesized clause
    PRIV_LIST, // the privilege list of a GRANT/REVOKE (`SELECT, INSERT` / `ALL PRIVILEGES`)
    GRANT_TARGET, // the `ON <object_type> <object_name>` securable of a GRANT/REVOKE
    GRANTEE,   // the `[ROLE|USER] <name>` recipient of a GRANT/REVOKE

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

    /// A human-readable name for this kind, for diagnostics.
    ///
    /// Where the `Debug` representation reads `SyntaxKind::INTO_KW` (useless to a SQL author),
    /// this reads `INTO`; punctuation is quoted (`'('`), and the literal/name kinds get a phrase
    /// (`an identifier`, `a string literal`). Keyword kinds derive their text from the variant
    /// name by stripping the trailing `_KW`, so every keyword is covered without a per-variant arm.
    pub fn describe(self) -> &'static str {
        use SyntaxKind::*;
        match self {
            // Literals & names.
            IDENT => "an identifier",
            QUOTED_IDENT => "a quoted identifier",
            STRING => "a string literal",
            DOLLAR_STRING => "a dollar-quoted string",
            INT_NUMBER => "an integer literal",
            FLOAT_NUMBER => "a number literal",
            VARIABLE => "a variable",
            // Punctuation & operators (quoted so the symbol is unambiguous in a sentence).
            L_PAREN => "'('",
            R_PAREN => "')'",
            L_BRACKET => "'['",
            R_BRACKET => "']'",
            L_BRACE => "'{'",
            R_BRACE => "'}'",
            COMMA => "','",
            DOT => "'.'",
            SEMICOLON => "';'",
            COLON => "':'",
            COLON2 => "'::'",
            ASSIGN => "':='",
            EQ => "'='",
            NEQ => "'<>'",
            LT => "'<'",
            LTE => "'<='",
            GT => "'>'",
            GTE => "'>='",
            PLUS => "'+'",
            MINUS => "'-'",
            STAR => "'*'",
            SLASH => "'/'",
            PERCENT => "'%'",
            CONCAT => "'||'",
            PIPE => "'|'",
            PIPE_GT => "'|>'",
            FLOW_PIPE => "'->>'",
            ARROW => "'->'",
            FAT_ARROW => "'=>'",
            AMP => "'&'",
            CARET => "'^'",
            TILDE => "'~'",
            AT => "'@'",
            DOLLAR => "'$'",
            QUESTION => "'?'",
            BANG => "'!'",
            // Trivia / structural.
            WHITESPACE => "whitespace",
            NEWLINE => "a newline",
            COMMENT => "a comment",
            BLOCK_COMMENT => "a block comment",
            EOF => "end of input",
            CONTEXTUAL_KEYWORD => "a keyword",
            // Keywords: derive the bare spelling (`INTO_KW` -> `INTO`) from the variant name.
            kind if kind.is_keyword() => kind.keyword_word(),
            // Any node kind reached here (should not happen on an error path).
            _ => "input",
        }
    }

    /// The bare keyword spelling for a keyword kind (`INTO_KW` -> `INTO`), derived from the
    /// variant's `Debug` name with the trailing `_KW` removed. Returns `""` for non-keywords.
    fn keyword_word(self) -> &'static str {
        macro_rules! kw {
            ($($variant:ident => $word:literal,)*) => {
                match self {
                    $(SyntaxKind::$variant => $word,)*
                    _ => "",
                }
            };
        }
        kw! {
            SELECT_KW => "SELECT", FROM_KW => "FROM", WHERE_KW => "WHERE", GROUP_KW => "GROUP",
            BY_KW => "BY", HAVING_KW => "HAVING", ORDER_KW => "ORDER", LIMIT_KW => "LIMIT",
            OFFSET_KW => "OFFSET", FETCH_KW => "FETCH", TOP_KW => "TOP", AS_KW => "AS",
            AND_KW => "AND", OR_KW => "OR", NOT_KW => "NOT", NULL_KW => "NULL", IS_KW => "IS",
            IN_KW => "IN", LIKE_KW => "LIKE", ILIKE_KW => "ILIKE", RLIKE_KW => "RLIKE",
            REGEXP_KW => "REGEXP", BETWEEN_KW => "BETWEEN", CASE_KW => "CASE", WHEN_KW => "WHEN",
            THEN_KW => "THEN", ELSE_KW => "ELSE", END_KW => "END", JOIN_KW => "JOIN",
            INNER_KW => "INNER", LEFT_KW => "LEFT", RIGHT_KW => "RIGHT", FULL_KW => "FULL",
            OUTER_KW => "OUTER", CROSS_KW => "CROSS", LATERAL_KW => "LATERAL",
            NATURAL_KW => "NATURAL", ON_KW => "ON", USING_KW => "USING", WITH_KW => "WITH",
            RECURSIVE_KW => "RECURSIVE", UNION_KW => "UNION", ALL_KW => "ALL", ANY_KW => "ANY",
            EXCEPT_KW => "EXCEPT", INTERSECT_KW => "INTERSECT", MINUS_KW => "MINUS",
            DISTINCT_KW => "DISTINCT", QUALIFY_KW => "QUALIFY", OVER_KW => "OVER",
            PARTITION_KW => "PARTITION", WINDOW_KW => "WINDOW", ROWS_KW => "ROWS",
            RANGE_KW => "RANGE", UNBOUNDED_KW => "UNBOUNDED", PRECEDING_KW => "PRECEDING",
            FOLLOWING_KW => "FOLLOWING", CURRENT_KW => "CURRENT", ROW_KW => "ROW", ASC_KW => "ASC",
            DESC_KW => "DESC", NULLS_KW => "NULLS", FIRST_KW => "FIRST", LAST_KW => "LAST",
            TRUE_KW => "TRUE", FALSE_KW => "FALSE", CAST_KW => "CAST", TRY_CAST_KW => "TRY_CAST",
            EXISTS_KW => "EXISTS", VALUES_KW => "VALUES", PIVOT_KW => "PIVOT",
            UNPIVOT_KW => "UNPIVOT", SAMPLE_KW => "SAMPLE", TABLESAMPLE_KW => "TABLESAMPLE",
            CREATE_KW => "CREATE", REPLACE_KW => "REPLACE", IF_KW => "IF", TABLE_KW => "TABLE",
            VIEW_KW => "VIEW", TEMPORARY_KW => "TEMPORARY", TEMP_KW => "TEMP",
            TRANSIENT_KW => "TRANSIENT", VOLATILE_KW => "VOLATILE", SECURE_KW => "SECURE",
            INSERT_KW => "INSERT", INTO_KW => "INTO", UPDATE_KW => "UPDATE", DELETE_KW => "DELETE",
            MERGE_KW => "MERGE", SET_KW => "SET", FLATTEN_KW => "FLATTEN", CONNECT_KW => "CONNECT",
            START_KW => "START", PRIOR_KW => "PRIOR", LANGUAGE_KW => "LANGUAGE",
            JAVASCRIPT_KW => "JAVASCRIPT", PYTHON_KW => "PYTHON", JAVA_KW => "JAVA",
            SCALA_KW => "SCALA", SQL_KW => "SQL", BEGIN_KW => "BEGIN", DECLARE_KW => "DECLARE",
            LET_KW => "LET", RETURN_KW => "RETURN", CALL_KW => "CALL", PROCEDURE_KW => "PROCEDURE",
            FUNCTION_KW => "FUNCTION", RETURNS_KW => "RETURNS", TASK_KW => "TASK",
            WAREHOUSE_KW => "WAREHOUSE", SCHEDULE_KW => "SCHEDULE", AFTER_KW => "AFTER",
            COPY_KW => "COPY", GRANTS_KW => "GRANTS", HANDLER_KW => "HANDLER",
            PACKAGES_KW => "PACKAGES", IMPORTS_KW => "IMPORTS",
            RUNTIME_VERSION_KW => "RUNTIME_VERSION", EXECUTE_KW => "EXECUTE", OWNER_KW => "OWNER",
            CALLER_KW => "CALLER", STRICT_KW => "STRICT", CALLED_KW => "CALLED",
            INPUT_KW => "INPUT", OUTPUT_KW => "OUTPUT", OUT_KW => "OUT", MATCHED_KW => "MATCHED",
            DROP_KW => "DROP", ALTER_KW => "ALTER", WITHIN_KW => "WITHIN", FOR_KW => "FOR",
            IMMEDIATE_KW => "IMMEDIATE", OVERWRITE_KW => "OVERWRITE", GRANT_KW => "GRANT",
            REVOKE_KW => "REVOKE", USE_KW => "USE", SHOW_KW => "SHOW", DESCRIBE_KW => "DESCRIBE",
            TRUNCATE_KW => "TRUNCATE", COMMIT_KW => "COMMIT", ROLLBACK_KW => "ROLLBACK",
            UNDROP_KW => "UNDROP", ELSEIF_KW => "ELSEIF", WHILE_KW => "WHILE", LOOP_KW => "LOOP",
            REPEAT_KW => "REPEAT", UNTIL_KW => "UNTIL", DO_KW => "DO", EXCEPTION_KW => "EXCEPTION",
            CURSOR_KW => "CURSOR", RESULTSET_KW => "RESULTSET",
        }
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

    #[test]
    fn describe_is_human_readable() {
        assert_eq!(SyntaxKind::INTO_KW.describe(), "INTO");
        assert_eq!(SyntaxKind::SELECT_KW.describe(), "SELECT");
        assert_eq!(SyntaxKind::L_PAREN.describe(), "'('");
        assert_eq!(SyntaxKind::FAT_ARROW.describe(), "'=>'");
        assert_eq!(SyntaxKind::IDENT.describe(), "an identifier");
        assert_eq!(SyntaxKind::STRING.describe(), "a string literal");
        assert_eq!(SyntaxKind::EOF.describe(), "end of input");
    }

    #[test]
    fn describe_covers_every_keyword() {
        // Every keyword kind must yield a non-empty spelling; a new keyword added to the enum
        // without a `describe` arm would fall through to "" and fail here.
        for raw in (SyntaxKind::__KW_START as u16 + 1)..(SyntaxKind::__KW_END as u16) {
            let kind = SyntaxKind::from_u16(raw);
            assert!(kind.is_keyword());
            assert!(
                !kind.describe().is_empty(),
                "{kind:?} has no describe() text"
            );
        }
    }
}
