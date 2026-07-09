//! Contextual keyword inventory used by the parser.
//!
//! Snowflake and Databricks both have many words that act keyword-like only in narrow syntactic
//! positions. They stay lexed as identifiers, and grammar rules opt into recognizing them with
//! [`crate::parser::Parser::nth_contextual`].

/// Snowflake's *contextual keywords*: words that act as keywords only in a specific syntactic
/// position and otherwise remain ordinary identifiers. They are never lexed as keywords and never
/// reserved, so the grammar recognizes them by text via [`crate::parser::Parser::nth_contextual`].
/// Listing them in one enum keeps the set discoverable and the match texts typo-proof.
macro_rules! contextual_keywords {
    ($(
        $(#[$meta:meta])*
        $variant:ident => $text:literal,
    )*) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ContextualKeyword {
            $(
                $(#[$meta])*
                $variant,
            )*
        }

        impl ContextualKeyword {
            /// The lowercase source text this word matches case-insensitively.
            pub(crate) fn text(self) -> &'static str {
                match self {
                    $(
                        ContextualKeyword::$variant => $text,
                    )*
                }
            }
        }
    };
}

contextual_keywords! {
    /// `<table> AT (...)` — time travel.
    At => "at",
    /// `<table> BEFORE (...)` — time travel.
    Before => "before",
    /// `ASOF JOIN` — the join-type word.
    Asof => "asof",
    /// `ASOF JOIN ... MATCH_CONDITION (...)`.
    MatchCondition => "match_condition",
    /// `<table> MATCH_RECOGNIZE (...)`.
    MatchRecognize => "match_recognize",
    /// `GROUP BY GROUPING SETS (...)` — first word.
    Grouping => "grouping",
    /// `GROUPING SETS (...)` — second word.
    Sets => "sets",
    /// `MEASURES <expr> AS <alias> [, ...]`.
    Measures => "measures",
    /// `PATTERN ( <row pattern> )`.
    Pattern => "pattern",
    /// `DEFINE <symbol> AS <predicate> [, ...]`.
    Define => "define",
    /// `SUBSET <name> = ( <symbol>, ... )`.
    Subset => "subset",
    /// `... PER MATCH`, `AFTER MATCH SKIP`.
    Match => "match",
    /// `ONE ROW PER MATCH`.
    One => "one",
    /// `AFTER MATCH SKIP ...`.
    Skip => "skip",
    /// `AFTER MATCH SKIP PAST LAST ROW`.
    Past => "past",
    /// `AFTER MATCH SKIP TO NEXT ROW`.
    Next => "next",
    /// `AFTER MATCH SKIP TO [FIRST|LAST] <symbol>`.
    To => "to",
    /// `CONNECT BY NOCYCLE ...`.
    NoCycle => "nocycle",
    /// `<table> CHANGES ( INFORMATION => ... )` — change-tracking queries.
    Changes => "changes",
    /// `COMMENT ON <object> IS '...'` — recognized only before `ON` so it never shadows the very
    /// common `comment` column/identifier.
    Comment => "comment",
    /// Databricks table time travel: `VERSION AS OF <expr>`.
    Version => "version",
    /// Databricks table time travel: `TIMESTAMP AS OF <expr>`.
    Timestamp => "timestamp",
    /// The `OF` in Databricks `VERSION AS OF` / `TIMESTAMP AS OF`.
    Of => "of",
    /// Databricks `CREATE TABLE ... LOCATION '<path>'`.
    Location => "location",
    /// Databricks `CREATE TABLE ... TBLPROPERTIES (...)`.
    Tblproperties => "tblproperties",
    /// Databricks `CREATE TABLE ... OPTIONS (...)`.
    Options => "options",
    /// Databricks `CREATE TABLE ... PARTITIONED BY (...)`.
    Partitioned => "partitioned",
    /// `BEGIN TRANSACTION` — distinguishes a transaction start from a Snowflake Scripting block.
    Transaction => "transaction",
    /// `BEGIN WORK` — the SQL-standard spelling of a transaction start.
    Work => "work",
    /// `BEGIN ATOMIC` — Databricks SQL scripting atomic compound statement modifier.
    Atomic => "atomic",
    /// `INTERVAL '1 day'` / `INTERVAL 1 DAY` expression literal.
    Interval => "interval",
    /// Interval unit word.
    Year => "year",
    /// Interval unit word.
    Years => "years",
    /// Interval unit word.
    Month => "month",
    /// Interval unit word.
    Months => "months",
    /// Interval unit word.
    Week => "week",
    /// Interval unit word.
    Weeks => "weeks",
    /// Interval unit word.
    Day => "day",
    /// Interval unit word.
    Days => "days",
    /// Interval unit word.
    Hour => "hour",
    /// Interval unit word.
    Minute => "minute",
    /// Interval unit word.
    Minutes => "minutes",
    /// Interval unit word.
    Second => "second",
    /// Interval unit word.
    Seconds => "seconds",
    /// Interval unit word.
    Millisecond => "millisecond",
    /// Interval unit word.
    Milliseconds => "milliseconds",
    /// Interval unit word.
    Microsecond => "microsecond",
    /// Interval unit word.
    Microseconds => "microseconds",
    /// `FOR i IN REVERSE <start> TO <end>` — counts down.
    Reverse => "reverse",
    /// `<name> [<type>] DEFAULT <expr>` — a declaration's default value (also a DDL column default).
    Default => "default",
    /// `BREAK [<label>]` — exit a loop.
    Break => "break",
    /// `CONTINUE [<label>]` — skip to the next loop iteration.
    Continue => "continue",
    /// `CREATE SCHEMA ...`.
    Schema => "schema",
    /// `CREATE DATABASE ...`.
    Database => "database",
    /// `CREATE STAGE ...` / `... ON STAGE ...`.
    Stage => "stage",
    /// `CREATE SEQUENCE ...`.
    Sequence => "sequence",
    /// `CREATE STREAM ...`.
    Stream => "stream",
    /// `CREATE DYNAMIC TABLE ...`.
    Dynamic => "dynamic",
    /// `CREATE SEMANTIC VIEW ...`.
    Semantic => "semantic",
    /// `CREATE FILE FORMAT ...`.
    File => "file",
    /// `CREATE FILE FORMAT ...` — second word.
    Format => "format",
    /// `CREATE MASKING POLICY ...`.
    Masking => "masking",
    /// `CREATE MASKING POLICY ...` / `CREATE ROW ACCESS POLICY ...`.
    Policy => "policy",
    /// `CREATE ROW ACCESS POLICY ...` — middle word.
    Access => "access",
    /// `CREATE TAG ...`.
    Tag => "tag",
    /// `CREATE TAG ... ALLOWED_VALUES ...`.
    AllowedValues => "allowed_values",
    /// `CREATE TAG ... PROPAGATE = ...`.
    Propagate => "propagate",
    /// `CREATE MASKING POLICY ... EXEMPT_OTHER_POLICIES = ...`.
    ExemptOtherPolicies => "exempt_other_policies",
    /// `CREATE SEMANTIC VIEW ... WITH TABLES`.
    Tables => "tables",
    /// `CREATE SEMANTIC VIEW ... WITH RELATIONSHIPS`.
    Relationships => "relationships",
    /// `CREATE SEMANTIC VIEW ... WITH FACTS`.
    Facts => "facts",
    /// `CREATE SEMANTIC VIEW ... WITH DIMENSIONS`.
    Dimensions => "dimensions",
    /// `CREATE SEMANTIC VIEW ... WITH METRICS`.
    Metrics => "metrics",
    /// `CREATE SEMANTIC VIEW ... PUBLIC`.
    Public => "public",
    /// `CREATE SEMANTIC VIEW ... PRIVATE`.
    Private => "private",
    /// Semantic View `SYNONYMS` attribute.
    Synonyms => "synonyms",
    /// Semantic View `LABELS` attribute.
    Labels => "labels",
    /// Semantic View `AI_SQL_GENERATION` attribute.
    AiSqlGeneration => "ai_sql_generation",
    /// Semantic View `AI_QUESTION_CATEGORIZATION` attribute.
    AiQuestionCategorization => "ai_question_categorization",
    /// Semantic View `AI_VERIFIED_QUERIES` attribute.
    AiVerifiedQueries => "ai_verified_queries",
    /// Semantic View verified-query `QUESTION`.
    Question => "question",
    /// Semantic View verified-query `VERIFIED_AT`.
    VerifiedAt => "verified_at",
    /// Semantic View verified-query `ONBOARDING_QUESTION`.
    OnboardingQuestion => "onboarding_question",
    /// Semantic View verified-query `VERIFIED_BY`.
    VerifiedBy => "verified_by",
    /// `... TO ROLE r` / `... FROM ROLE r`.
    Role => "role",
    /// `... TO USER u`.
    User => "user",
    /// `GRANT <role> TO SHARE s`.
    Share => "share",
    /// `REVOKE ... FROM r RESTRICT|CASCADE` — cascade.
    Cascade => "cascade",
    /// `REVOKE ... FROM r RESTRICT` — restrict.
    Restrict => "restrict",
    /// `REVOKE GRANT OPTION FOR ...` / `... WITH GRANT OPTION` — option.
    Option => "option",
    /// `GRANT ALL PRIVILEGES ...`.
    Privileges => "privileges",
    /// `MATERIALIZED VIEW`.
    Materialized => "materialized",
    /// `LOCAL TEMP[ORARY]` table/view modifier.
    Local => "local",
    /// `GLOBAL TEMP[ORARY]` table/view modifier.
    Global => "global",
    /// `CLUSTER BY ( ... )`.
    Cluster => "cluster",
    /// `CREATE TABLE <name> CLONE <source>`.
    Clone => "clone",
    /// `CREATE TABLE <name> SHALLOW CLONE <source>`.
    Shallow => "shallow",
    /// `CREATE TABLE <name> DEEP CLONE <source>`.
    Deep => "deep",
    /// Databricks/Spark `DISTRIBUTE BY`.
    Distribute => "distribute",
    /// Databricks/Spark `SORT BY`.
    Sort => "sort",
    /// A `PRIMARY KEY` constraint (also the first word of the two).
    Primary => "primary",
    /// The `KEY` of `PRIMARY KEY` / `FOREIGN KEY`.
    Key => "key",
    /// A `UNIQUE` constraint.
    Unique => "unique",
    /// A `FOREIGN KEY` constraint.
    Foreign => "foreign",
    /// `FOREIGN KEY ( ... ) REFERENCES <table> ( ... )`.
    References => "references",
    /// `CONSTRAINT <name> ...` — names an out-of-line constraint.
    Constraint => "constraint",
    /// A `CHECK ( <expr> )` constraint.
    Check => "check",
    /// A column `COLLATE '<spec>'`.
    Collate => "collate",
    // ---- Databricks / Delta maintenance + cache statements (contextual at statement start, so the
    // words stay ordinary identifiers under Snowflake and elsewhere under Databricks) ----
    /// `VACUUM <table|path> ...` — Delta file cleanup statement.
    Vacuum => "vacuum",
    /// `VACUUM <t> RETAIN <n> HOURS ...`.
    Retain => "retain",
    /// `VACUUM <t> RETAIN <n> HOURS` — the `HOURS` unit word.
    Hours => "hours",
    /// `VACUUM <t> ... DRY RUN` — first word.
    Dry => "dry",
    /// `VACUUM <t> ... DRY RUN` — second word.
    Run => "run",
    /// `OPTIMIZE <table> [WHERE p] [ZORDER BY (cols)]` — Delta compaction statement.
    Optimize => "optimize",
    /// `OPTIMIZE <t> ZORDER BY ( col [, ...] )` — the z-order clause keyword.
    Zorder => "zorder",
    /// `CACHE [LAZY] TABLE <t> ...` — Spark cache statement.
    Cache => "cache",
    /// `CACHE LAZY TABLE ...` — defer caching until first use.
    Lazy => "lazy",
    /// `UNCACHE TABLE [IF EXISTS] <t>` — drop a cached table.
    Uncache => "uncache",
    /// `REFRESH [TABLE] <t>` / `REFRESH <path>` — invalidate cached entries.
    Refresh => "refresh",
    /// `RESTORE TABLE <t> ...` — Delta restore statement.
    Restore => "restore",
    /// `ANALYZE TABLE <t> COMPUTE STATISTICS` — Spark statistics statement.
    Analyze => "analyze",
    /// `ANALYZE TABLE <t> COMPUTE STATISTICS` — compute word.
    Compute => "compute",
    /// `ANALYZE TABLE <t> COMPUTE STATISTICS` — statistics word.
    Statistics => "statistics",
    /// `ANALYZE TABLE <t> COMPUTE STATISTICS FOR COLUMNS ...` — columns word.
    Columns => "columns",
    /// `MSCK REPAIR TABLE <t>` — first word.
    Msck => "msck",
    /// `MSCK REPAIR TABLE <t>` — second word.
    Repair => "repair",
    /// `MSCK REPAIR TABLE <t> SYNC PARTITIONS` — sync word.
    Sync => "sync",
    /// `MSCK REPAIR TABLE <t> SYNC PARTITIONS` — partitions word.
    Partitions => "partitions",
    /// `DESCRIBE HISTORY <table>` — Delta change-history statement (the `HISTORY` word).
    History => "history",
    /// `MERGE ... WHEN NOT MATCHED BY SOURCE ...` — the `SOURCE` qualifier word.
    Source => "source",
    /// `MERGE ... WHEN NOT MATCHED BY TARGET ...` — the `TARGET` qualifier word.
    Target => "target",
}
