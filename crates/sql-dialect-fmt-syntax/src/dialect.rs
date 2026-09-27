//! The SQL [`Dialect`] selector — the runtime seam for multi-dialect support.
//!
//! The engine (lossless lexer, never-fail parser, Doc-IR formatter) is dialect-agnostic; only the
//! ~20% that differs between dialects — reserved keywords, lexer quoting/special tokens, which
//! statements/operators the grammar accepts, and a few formatter rules — is gated on a [`Dialect`].
//! Modeled on `sqlparser-rs`'s `Dialect` trait: rather than a trait object, a small `Copy` enum
//! threads through the lexer, parser, and formatter, and the divergence points are expressed as
//! `#[must_use]` predicate methods on it.
//!
//! The vocabulary mirrors [`sql-formatter`](https://github.com/sql-formatter-org/sql-formatter):
//! the standard `sql` default plus BigQuery, ClickHouse, DB2 (and DB2 for i), DuckDB, Hive, MariaDB,
//! MySQL, TiDB, N1QL, Oracle PL/SQL, PostgreSQL, Redshift, SingleStoreDB, Snowflake, Spark,
//! SQLite, Transact-SQL, and Trino, with Databricks as this toolchain's Delta-flavored Spark.
//!
//! [`Dialect::Snowflake`] stays the [`Default`]. Snowflake and Databricks predicate answers are
//! unchanged from the original two-dialect seam; the other dialects describe their *intended*
//! lexical profile so the lexer/highlighter can diverge per dialect.

/// The SQL dialect a lex/parse/format request targets.
///
/// `#[non_exhaustive]` so further dialects can be added without it being a breaking change. Default
/// is [`Dialect::Snowflake`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Dialect {
    /// Snowflake SQL — the original and default dialect of this toolchain.
    #[default]
    Snowflake,
    /// Databricks SQL / Delta-flavored Spark SQL.
    Databricks,
    /// Apache Spark SQL.
    Spark,
    /// Google Cloud BigQuery.
    BigQuery,
    /// ClickHouse.
    ClickHouse,
    /// IBM DB2.
    Db2,
    /// IBM DB2 for i.
    Db2i,
    /// DuckDB.
    DuckDb,
    /// Apache Hive.
    Hive,
    /// MariaDB.
    MariaDb,
    /// MySQL.
    MySql,
    /// TiDB (MySQL-compatible).
    TiDb,
    /// Couchbase N1QL.
    N1ql,
    /// Oracle PL/SQL.
    PlSql,
    /// PostgreSQL.
    PostgreSql,
    /// Amazon Redshift.
    Redshift,
    /// SingleStoreDB.
    SingleStoreDb,
    /// SQLite.
    Sqlite,
    /// Standard / generic SQL (the `sql` default of `sql-formatter`).
    Sql,
    /// Microsoft SQL Server Transact-SQL (the `transactsql` / `tsql` dialect).
    TransactSql,
    /// Trino (and, in practice, Presto).
    Trino,
}

impl Dialect {
    /// Every dialect, in the order used by [`Dialect::ALL`]-driven tooling (help text, CLI
    /// completion, docs).
    pub const ALL: &'static [Dialect] = &[
        Dialect::Snowflake,
        Dialect::Databricks,
        Dialect::Spark,
        Dialect::BigQuery,
        Dialect::ClickHouse,
        Dialect::Db2,
        Dialect::Db2i,
        Dialect::DuckDb,
        Dialect::Hive,
        Dialect::MariaDb,
        Dialect::MySql,
        Dialect::TiDb,
        Dialect::N1ql,
        Dialect::PlSql,
        Dialect::PostgreSql,
        Dialect::Redshift,
        Dialect::SingleStoreDb,
        Dialect::Sqlite,
        Dialect::Sql,
        Dialect::TransactSql,
        Dialect::Trino,
    ];

    /// Number of dialects in [`Dialect::ALL`] (`DialectSet` sizes its bitmask to this).
    pub const COUNT: usize = Dialect::ALL.len();

    /// This dialect's bit index, matching its position in [`Dialect::ALL`].
    #[must_use]
    pub const fn bit_index(self) -> u32 {
        match self {
            Dialect::Snowflake => 0,
            Dialect::Databricks => 1,
            Dialect::Spark => 2,
            Dialect::BigQuery => 3,
            Dialect::ClickHouse => 4,
            Dialect::Db2 => 5,
            Dialect::Db2i => 6,
            Dialect::DuckDb => 7,
            Dialect::Hive => 8,
            Dialect::MariaDb => 9,
            Dialect::MySql => 10,
            Dialect::TiDb => 11,
            Dialect::N1ql => 12,
            Dialect::PlSql => 13,
            Dialect::PostgreSql => 14,
            Dialect::Redshift => 15,
            Dialect::SingleStoreDb => 16,
            Dialect::Sqlite => 17,
            Dialect::Sql => 18,
            Dialect::TransactSql => 19,
            Dialect::Trino => 20,
        }
    }

    /// This dialect's one-bit [`DialectSet`].
    #[must_use]
    pub const fn bit(self) -> u64 {
        1u64 << self.bit_index()
    }

    /// The canonical lowercase name used in config files and CLI flags.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Dialect::Snowflake => "snowflake",
            Dialect::Databricks => "databricks",
            Dialect::Spark => "spark",
            Dialect::BigQuery => "bigquery",
            Dialect::ClickHouse => "clickhouse",
            Dialect::Db2 => "db2",
            Dialect::Db2i => "db2i",
            Dialect::DuckDb => "duckdb",
            Dialect::Hive => "hive",
            Dialect::MariaDb => "mariadb",
            Dialect::MySql => "mysql",
            Dialect::TiDb => "tidb",
            Dialect::N1ql => "n1ql",
            Dialect::PlSql => "plsql",
            Dialect::PostgreSql => "postgresql",
            Dialect::Redshift => "redshift",
            Dialect::SingleStoreDb => "singlestoredb",
            Dialect::Sqlite => "sqlite",
            Dialect::Sql => "sql",
            Dialect::TransactSql => "transactsql",
            Dialect::Trino => "trino",
        }
    }

    /// Resolve a dialect from a (case-insensitive) canonical name or common alias.
    ///
    /// Aliases mirror the names people actually type — `oracle` for [`Dialect::PlSql`], `tsql` for
    /// [`Dialect::TransactSql`], `postgres` for [`Dialect::PostgreSql`], `presto` for
    /// [`Dialect::Trino`], and so on. Returns `None` for an unknown name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Dialect> {
        let normalized = name.trim().to_ascii_lowercase();
        Some(match normalized.as_str() {
            "snowflake" => Dialect::Snowflake,
            "databricks" => Dialect::Databricks,
            "spark" | "sparksql" | "spark-sql" => Dialect::Spark,
            "bigquery" | "bq" => Dialect::BigQuery,
            "clickhouse" => Dialect::ClickHouse,
            "db2" => Dialect::Db2,
            "db2i" | "db2-for-i" | "as400" => Dialect::Db2i,
            "duckdb" => Dialect::DuckDb,
            "hive" => Dialect::Hive,
            "mariadb" | "maria" => Dialect::MariaDb,
            "mysql" => Dialect::MySql,
            "tidb" => Dialect::TiDb,
            "n1ql" | "couchbase" => Dialect::N1ql,
            "plsql" | "pl/sql" | "pl-sql" | "oracle" => Dialect::PlSql,
            "postgresql" | "postgres" | "pg" => Dialect::PostgreSql,
            "redshift" => Dialect::Redshift,
            "singlestoredb" | "singlestore" | "memsql" => Dialect::SingleStoreDb,
            "sqlite" | "sqlite3" => Dialect::Sqlite,
            "sql" | "standard" | "ansi" | "generic" => Dialect::Sql,
            "transactsql" | "transact-sql" | "tsql" | "mssql" | "sqlserver" | "sql-server" => {
                Dialect::TransactSql
            }
            "trino" | "presto" => Dialect::Trino,
            _ => return None,
        })
    }

    /// `true` when the dialect uses `"double quoted"` spellings for identifiers.
    ///
    /// The MySQL family, BigQuery, Hive, Spark and N1QL treat `"..."` as a *string* literal and
    /// quote identifiers with backticks (or brackets) instead. Databricks keeps the historical
    /// `"..."`-as-identifier behavior of this toolchain.
    #[must_use]
    pub fn supports_double_quoted_identifiers(self) -> bool {
        !matches!(
            self,
            Dialect::MySql
                | Dialect::MariaDb
                | Dialect::TiDb
                | Dialect::SingleStoreDb
                | Dialect::BigQuery
                | Dialect::Hive
                | Dialect::Spark
                | Dialect::N1ql
        )
    }

    /// `true` when `#` begins a line comment (MySQL family, BigQuery, ClickHouse).
    #[must_use]
    pub fn supports_hash_line_comments(self) -> bool {
        matches!(
            self,
            Dialect::MySql
                | Dialect::MariaDb
                | Dialect::TiDb
                | Dialect::SingleStoreDb
                | Dialect::BigQuery
                | Dialect::ClickHouse
        )
    }

    /// `true` when `#name` / `##name` is a (temp) identifier rather than a comment — Transact-SQL
    /// local/global temporary tables.
    #[must_use]
    pub fn supports_hash_identifiers(self) -> bool {
        matches!(self, Dialect::TransactSql)
    }

    /// `[bracketed]` identifiers. Transact-SQL and SQLite.
    #[must_use]
    pub fn supports_bracket_identifiers(self) -> bool {
        matches!(self, Dialect::TransactSql | Dialect::Sqlite)
    }

    /// `/* ... /* ... */ ... */` block comments nest. PostgreSQL and Transact-SQL; every other
    /// dialect stops at the first `*/`.
    #[must_use]
    pub fn supports_nested_block_comments(self) -> bool {
        matches!(self, Dialect::PostgreSql | Dialect::TransactSql)
    }

    /// `@name` / `@@name` variables. Transact-SQL and the MySQL family.
    #[must_use]
    pub fn supports_at_variables(self) -> bool {
        matches!(
            self,
            Dialect::TransactSql
                | Dialect::MySql
                | Dialect::MariaDb
                | Dialect::TiDb
                | Dialect::SingleStoreDb
        )
    }

    /// `:name` bind variables (as distinct from the `:` semi-structured path operator).
    #[must_use]
    pub fn supports_colon_variables(self) -> bool {
        matches!(self, Dialect::PlSql)
    }

    /// Lowercase letters that introduce prefixed string literals (`e'…'`, `n'…'`, `x'…'`,
    /// `b'…'`, `r'…'`). Empty when the dialect has no prefixed string forms.
    #[must_use]
    pub const fn prefixed_string_letters(self) -> &'static [u8] {
        match self {
            Dialect::Snowflake
            | Dialect::ClickHouse
            | Dialect::Hive
            | Dialect::N1ql
            | Dialect::Sql
            | Dialect::DuckDb => &[],
            Dialect::Databricks | Dialect::Spark => b"rx",
            Dialect::BigQuery => b"rb",
            Dialect::Db2 | Dialect::Db2i | Dialect::TransactSql => b"nx",
            Dialect::MariaDb | Dialect::MySql | Dialect::TiDb | Dialect::SingleStoreDb => b"nxb",
            Dialect::PlSql => b"n",
            Dialect::PostgreSql => b"exb",
            Dialect::Redshift => b"e",
            Dialect::Sqlite => b"xb",
            Dialect::Trino => b"x",
        }
    }

    /// `true` when `\` is an escape character inside a *prefixed* string literal (e.g. PostgreSQL
    /// `e'…'`). Plain `'…'` strings keep the toolchain's universal backslash handling.
    #[must_use]
    pub fn prefixed_string_uses_backslash_escapes(self, letter: u8) -> bool {
        letter.eq_ignore_ascii_case(&b'e')
    }

    /// Dollar quoting: `$$ ... $$` dollar-quoted bodies and `$1` / `$name` positional/variable
    /// references. Snowflake, PostgreSQL and DuckDB.
    #[must_use]
    pub fn supports_dollar_quoting(self) -> bool {
        matches!(
            self,
            Dialect::Snowflake | Dialect::PostgreSql | Dialect::DuckDb
        )
    }

    /// The flow operator `->>` chaining statements into a pipeline. Snowflake only.
    #[must_use]
    pub fn supports_flow_operator(self) -> bool {
        matches!(self, Dialect::Snowflake)
    }

    /// `COPY INTO <target> FROM <source>` bulk load/unload. Snowflake only.
    #[must_use]
    pub fn supports_copy_into(self) -> bool {
        matches!(self, Dialect::Snowflake)
    }

    /// `// ...` line comments. Snowflake accepts them; e.g. Databricks/Spark uses `/` as an operator
    /// and must not have a doubled slash consume the rest of the physical line.
    #[must_use]
    pub fn supports_double_slash_comments(self) -> bool {
        matches!(self, Dialect::Snowflake)
    }

    /// Snowpipe DDL: `CREATE [OR REPLACE] PIPE <name> <properties> AS COPY INTO ...` and
    /// `ALTER PIPE`. Snowflake only — the object does not exist in the other dialects, where `pipe`
    /// stays an ordinary identifier.
    #[must_use]
    pub fn supports_pipe_ddl(self) -> bool {
        matches!(self, Dialect::Snowflake)
    }

    /// `CREATE SEMANTIC VIEW ...` semantic-layer DDL. Snowflake only.
    #[must_use]
    pub fn supports_semantic_view(self) -> bool {
        matches!(self, Dialect::Snowflake)
    }

    /// SQL scripting blocks: `BEGIN ... END`, declarations, control-flow statements, and the `:=`
    /// assignment operator. Snowflake and Databricks both support compound SQL blocks.
    #[must_use]
    pub fn supports_scripting_blocks(self) -> bool {
        matches!(self, Dialect::Snowflake | Dialect::Databricks)
    }

    /// Stage references: `@stage` / `@~` / `@%table` paths in `FROM`, `COPY`, and `PUT`/`GET`.
    /// Snowflake only.
    #[must_use]
    pub fn supports_stage_refs(self) -> bool {
        matches!(self, Dialect::Snowflake)
    }

    /// Backtick-quoted identifiers: `` `col` ``. The MySQL family, the Spark family, BigQuery,
    /// ClickHouse, Hive, N1QL, SingleStoreDB and SQLite. Snowflake quotes with `"`.
    #[must_use]
    pub fn supports_backtick_identifiers(self) -> bool {
        matches!(
            self,
            Dialect::Databricks
                | Dialect::Spark
                | Dialect::BigQuery
                | Dialect::ClickHouse
                | Dialect::Hive
                | Dialect::MariaDb
                | Dialect::MySql
                | Dialect::TiDb
                | Dialect::N1ql
                | Dialect::SingleStoreDb
                | Dialect::Sqlite
        )
    }

    /// Prefixed string literals such as raw strings (`r'...'`) and hex binary literals (`X'1A'`).
    /// See [`Dialect::prefixed_string_letters`].
    #[must_use]
    pub fn supports_prefixed_strings(self) -> bool {
        !self.prefixed_string_letters().is_empty()
    }

    /// Null-safe equality operator `<=>`. The MySQL family and the Spark family.
    #[must_use]
    pub fn supports_null_safe_eq(self) -> bool {
        matches!(
            self,
            Dialect::Databricks
                | Dialect::Spark
                | Dialect::MySql
                | Dialect::MariaDb
                | Dialect::TiDb
                | Dialect::SingleStoreDb
        )
    }

    /// `LATERAL VIEW explode(...)` table-generating clause. Spark, Databricks and Hive.
    #[must_use]
    pub fn supports_lateral_view(self) -> bool {
        matches!(self, Dialect::Databricks | Dialect::Spark | Dialect::Hive)
    }

    /// Delta/Spark table DDL options: `USING`, `LOCATION`, `TBLPROPERTIES`, `OPTIONS`, and
    /// `PARTITIONED BY`. Databricks and Spark.
    #[must_use]
    pub fn supports_delta_table_options(self) -> bool {
        matches!(self, Dialect::Databricks | Dialect::Spark)
    }

    /// Higher-order-function lambdas: `x -> expr` and `(x, y) -> expr`. Spark family, ClickHouse,
    /// DuckDB, and Trino.
    #[must_use]
    pub fn supports_lambda_expr(self) -> bool {
        matches!(
            self,
            Dialect::Databricks
                | Dialect::Spark
                | Dialect::ClickHouse
                | Dialect::DuckDb
                | Dialect::Trino
        )
    }

    /// Time travel via `VERSION AS OF` / `TIMESTAMP AS OF`. Databricks and Spark (Snowflake uses
    /// `AT` / `BEFORE`).
    #[must_use]
    pub fn supports_as_of_travel(self) -> bool {
        matches!(self, Dialect::Databricks | Dialect::Spark)
    }

    /// Spark-family query distribution clauses: `DISTRIBUTE BY`, `SORT BY`, and `CLUSTER BY`.
    #[must_use]
    pub fn supports_databricks_query_clauses(self) -> bool {
        matches!(self, Dialect::Databricks | Dialect::Spark | Dialect::Hive)
    }

    /// `RETURNING <expr> [, ...]` after `INSERT`/`UPDATE`/`DELETE`. PostgreSQL, SQLite, DuckDB,
    /// MariaDB and N1QL.
    #[must_use]
    pub fn supports_returning_clause(self) -> bool {
        matches!(
            self,
            Dialect::PostgreSql
                | Dialect::Sqlite
                | Dialect::DuckDb
                | Dialect::MariaDb
                | Dialect::N1ql
        )
    }

    /// `INSERT ... ON CONFLICT [(cols)] DO NOTHING | DO UPDATE SET ...`. PostgreSQL, SQLite, DuckDB.
    #[must_use]
    pub fn supports_insert_conflict(self) -> bool {
        matches!(
            self,
            Dialect::PostgreSql | Dialect::Sqlite | Dialect::DuckDb
        )
    }

    /// `INSERT ... ON DUPLICATE KEY UPDATE ...`. The MySQL family.
    #[must_use]
    pub fn supports_on_duplicate_key(self) -> bool {
        matches!(
            self,
            Dialect::MySql | Dialect::MariaDb | Dialect::TiDb | Dialect::SingleStoreDb
        )
    }

    /// SQLite `INSERT OR REPLACE|IGNORE|ABORT|FAIL|ROLLBACK INTO ...`.
    #[must_use]
    pub fn supports_insert_or_clause(self) -> bool {
        matches!(self, Dialect::Sqlite)
    }

    /// ClickHouse `PREWHERE <expr>` (a filter applied before `WHERE`).
    #[must_use]
    pub fn supports_prewhere(self) -> bool {
        matches!(self, Dialect::ClickHouse)
    }

    /// Transact-SQL `OUTPUT ... [INTO ...]` DML clause.
    #[must_use]
    pub fn supports_output_clause(self) -> bool {
        matches!(self, Dialect::TransactSql)
    }

    /// A trailing `FOR ...` clause on a `SELECT`: Transact-SQL `FOR JSON|XML ...` output, and
    /// the row-locking `FOR UPDATE` / `FOR SHARE` of PostgreSQL, the MySQL family, DuckDB and
    /// Redshift.
    #[must_use]
    pub fn supports_select_for_clause(self) -> bool {
        matches!(
            self,
            Dialect::TransactSql
                | Dialect::PostgreSql
                | Dialect::MySql
                | Dialect::MariaDb
                | Dialect::TiDb
                | Dialect::SingleStoreDb
                | Dialect::DuckDb
                | Dialect::Redshift
        )
    }

    /// PostgreSQL/DuckDB `SELECT DISTINCT ON (<expr>, ...)`.
    #[must_use]
    pub fn supports_distinct_on(self) -> bool {
        matches!(self, Dialect::PostgreSql | Dialect::DuckDb)
    }

    /// MySQL-family `INSERT INTO t SET col = expr, ...`.
    #[must_use]
    pub fn supports_insert_set(self) -> bool {
        matches!(
            self,
            Dialect::MySql | Dialect::MariaDb | Dialect::TiDb | Dialect::SingleStoreDb
        )
    }

    /// MySQL-family `REPLACE [INTO] t ...` (delete-then-insert upsert).
    #[must_use]
    pub fn supports_replace_into(self) -> bool {
        matches!(
            self,
            Dialect::MySql | Dialect::MariaDb | Dialect::TiDb | Dialect::SingleStoreDb
        )
    }

    /// Transact-SQL `SELECT ... INTO <target> FROM ...`.
    #[must_use]
    pub fn supports_select_into(self) -> bool {
        matches!(self, Dialect::TransactSql)
    }

    /// `<table> WITH (NOLOCK, ...)` Transact-SQL table hints.
    #[must_use]
    pub fn supports_table_hints(self) -> bool {
        matches!(self, Dialect::TransactSql)
    }

    /// JSON member-access arrows `->` / `->>`. PostgreSQL, DuckDB and the MySQL family. (Snowflake
    /// uses `:`; `->>` there is the statement-level flow operator.)
    #[must_use]
    pub fn supports_json_arrow(self) -> bool {
        matches!(
            self,
            Dialect::PostgreSql
                | Dialect::DuckDb
                | Dialect::MySql
                | Dialect::MariaDb
                | Dialect::TiDb
                | Dialect::SingleStoreDb
        )
    }

    /// `f(expr AS alias, ...)` argument aliases. BigQuery `STRUCT(...)`.
    #[must_use]
    pub fn supports_argument_aliases(self) -> bool {
        matches!(self, Dialect::BigQuery)
    }

    /// `agg(expr ORDER BY ...)` order inside an aggregate argument list. PostgreSQL, Redshift,
    /// BigQuery, DuckDB, ClickHouse, the MySQL family and Trino.
    #[must_use]
    pub fn supports_argument_order_by(self) -> bool {
        matches!(
            self,
            Dialect::PostgreSql
                | Dialect::Redshift
                | Dialect::BigQuery
                | Dialect::DuckDb
                | Dialect::ClickHouse
                | Dialect::MySql
                | Dialect::MariaDb
                | Dialect::TiDb
                | Dialect::SingleStoreDb
                | Dialect::Trino
        )
    }

    /// Whether the parser is allowed to start a new statement without a `;` separator. Snowflake and
    /// Databricks do (their grammar covers the common statement tails); every other dialect is
    /// strict so an unrecognized trailing clause becomes a diagnostic (and the formatter falls back
    /// to verbatim) instead of a silently invented statement boundary.
    #[must_use]
    pub fn tolerates_implicit_statement_boundaries(self) -> bool {
        matches!(self, Dialect::Snowflake | Dialect::Databricks)
    }

    /// SQLite `GLOB` comparison operator.
    #[must_use]
    pub fn supports_glob(self) -> bool {
        matches!(self, Dialect::Sqlite)
    }

    /// Transact-SQL `EXEC <proc> ...` statement.
    #[must_use]
    pub fn supports_exec_statement(self) -> bool {
        matches!(self, Dialect::TransactSql)
    }

    /// ClickHouse `... SETTINGS k = v [, ...]` query tail.
    #[must_use]
    pub fn supports_settings_clause(self) -> bool {
        matches!(self, Dialect::ClickHouse)
    }

    /// ClickHouse `... FORMAT <name>` query tail.
    #[must_use]
    pub fn supports_format_clause(self) -> bool {
        matches!(self, Dialect::ClickHouse)
    }

    /// BigQuery `SELECT * EXCEPT (...)` / `SELECT * REPLACE (...)` star modifiers.
    #[must_use]
    pub fn supports_select_star_modifier(self) -> bool {
        matches!(self, Dialect::BigQuery)
    }

    /// Delta/Spark maintenance + cache statements — `VACUUM`, `OPTIMIZE … ZORDER BY`,
    /// `INSERT OVERWRITE`, `CACHE`/`UNCACHE`/`REFRESH`, `DESCRIBE HISTORY`, and the
    /// `WHEN NOT MATCHED BY SOURCE`/`INSERT *` MERGE extensions. Databricks and Spark. The leading
    /// words (`VACUUM`, `OPTIMIZE`, `CACHE`, …) are recognized **contextually** at statement start,
    /// so they stay ordinary identifiers under other dialects.
    #[must_use]
    pub fn supports_delta_commands(self) -> bool {
        matches!(self, Dialect::Databricks | Dialect::Spark)
    }
}

/// A compact bitset of [`Dialect`] variants, used to describe which dialects reserve a keyword (or
/// share any other dialect-scoped property). Iteration order follows [`Dialect::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DialectSet(u64);

impl DialectSet {
    /// No dialects.
    pub const EMPTY: DialectSet = DialectSet(0);
    /// Every dialect in [`Dialect::ALL`].
    pub const ALL: DialectSet = DialectSet((1u64 << Dialect::COUNT) - 1);
    /// Snowflake only.
    pub const SNOWFLAKE_ONLY: DialectSet = DialectSet::of(&[Dialect::Snowflake]);
    /// Databricks only.
    pub const DATABRICKS_ONLY: DialectSet = DialectSet::of(&[Dialect::Databricks]);

    /// Build a set from a slice of dialects (usable in `const` context).
    #[must_use]
    pub const fn of(dialects: &[Dialect]) -> DialectSet {
        let mut bits = 0u64;
        let mut i = 0;
        while i < dialects.len() {
            bits |= dialects[i].bit();
            i += 1;
        }
        DialectSet(bits)
    }

    /// Whether the set has no dialects.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether `dialect` is a member.
    #[must_use]
    pub const fn contains(self, dialect: Dialect) -> bool {
        self.0 & dialect.bit() != 0
    }

    /// Whether `dialect` reserves a keyword carrying this set. Alias for [`DialectSet::contains`],
    /// used at the keyword call sites for readability.
    #[must_use]
    pub const fn reserved_in(self, dialect: Dialect) -> bool {
        self.contains(dialect)
    }

    /// The union of two sets.
    #[must_use]
    pub const fn union(self, other: DialectSet) -> DialectSet {
        DialectSet(self.0 | other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Dialect, DialectSet};

    #[test]
    fn default_is_snowflake() {
        assert_eq!(Dialect::default(), Dialect::Snowflake);
    }

    #[test]
    fn all_contains_every_variant_without_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for dialect in Dialect::ALL {
            assert!(
                seen.insert(*dialect),
                "duplicate dialect in ALL: {dialect:?}"
            );
        }
        assert_eq!(seen.len(), Dialect::ALL.len());
        // Every canonical name round-trips.
        for dialect in Dialect::ALL {
            assert_eq!(
                Dialect::from_name(dialect.canonical_name()),
                Some(*dialect),
                "canonical name did not round-trip: {dialect:?}"
            );
        }
    }

    #[test]
    fn aliases_resolve() {
        assert_eq!(Dialect::from_name("oracle"), Some(Dialect::PlSql));
        assert_eq!(Dialect::from_name("PL/SQL"), Some(Dialect::PlSql));
        assert_eq!(Dialect::from_name("tsql"), Some(Dialect::TransactSql));
        assert_eq!(Dialect::from_name("mssql"), Some(Dialect::TransactSql));
        assert_eq!(Dialect::from_name("postgres"), Some(Dialect::PostgreSql));
        assert_eq!(Dialect::from_name("Presto"), Some(Dialect::Trino));
        assert_eq!(Dialect::from_name("couchbase"), Some(Dialect::N1ql));
        assert_eq!(Dialect::from_name("  MySQL  "), Some(Dialect::MySql));
        assert_eq!(Dialect::from_name("nonsense"), None);
    }

    #[test]
    fn snowflake_only_predicates() {
        let s = Dialect::Snowflake;
        assert!(s.supports_dollar_quoting());
        assert!(s.supports_flow_operator());
        assert!(s.supports_copy_into());
        assert!(s.supports_pipe_ddl());
        assert!(s.supports_double_slash_comments());
        assert!(s.supports_semantic_view());
        assert!(s.supports_scripting_blocks());
        assert!(s.supports_stage_refs());
        assert!(!s.supports_backtick_identifiers());
        assert!(!s.supports_prefixed_strings());
        assert!(!s.supports_null_safe_eq());
        assert!(!s.supports_lateral_view());
        assert!(!s.supports_delta_table_options());
        assert!(!s.supports_lambda_expr());
        assert!(!s.supports_as_of_travel());
        assert!(!s.supports_databricks_query_clauses());
        assert!(!s.supports_delta_commands());
    }

    #[test]
    fn databricks_only_predicates() {
        let d = Dialect::Databricks;
        assert!(!d.supports_dollar_quoting());
        assert!(!d.supports_flow_operator());
        assert!(!d.supports_copy_into());
        assert!(!d.supports_pipe_ddl());
        assert!(!d.supports_double_slash_comments());
        assert!(!d.supports_semantic_view());
        assert!(d.supports_scripting_blocks());
        assert!(!d.supports_stage_refs());
        assert!(d.supports_backtick_identifiers());
        assert!(d.supports_prefixed_strings());
        assert!(d.supports_null_safe_eq());
        assert!(d.supports_lateral_view());
        assert!(d.supports_delta_table_options());
        assert!(d.supports_lambda_expr());
        assert!(d.supports_as_of_travel());
        assert!(d.supports_databricks_query_clauses());
        assert!(d.supports_delta_commands());
    }

    #[test]
    fn oracle_and_mysql_and_tsql_lexical_profiles() {
        let oracle = Dialect::PlSql;
        assert!(oracle.supports_double_quoted_identifiers());
        assert!(!oracle.supports_backtick_identifiers());
        assert!(oracle.supports_colon_variables());
        assert_eq!(oracle.prefixed_string_letters(), b"n");

        let mysql = Dialect::MySql;
        assert!(!mysql.supports_double_quoted_identifiers());
        assert!(mysql.supports_backtick_identifiers());
        assert!(mysql.supports_hash_line_comments());
        assert!(mysql.supports_at_variables());

        let tsql = Dialect::TransactSql;
        assert!(tsql.supports_bracket_identifiers());
        assert!(tsql.supports_hash_identifiers());
        assert!(!tsql.supports_hash_line_comments());
        assert!(tsql.supports_at_variables());
        assert!(tsql.supports_nested_block_comments());
    }

    #[test]
    fn bit_index_matches_all_order() {
        for (index, dialect) in Dialect::ALL.iter().enumerate() {
            assert_eq!(dialect.bit_index() as usize, index, "{dialect:?}");
            assert_eq!(dialect.bit(), 1u64 << index, "{dialect:?}");
        }
        assert_eq!(Dialect::COUNT, Dialect::ALL.len());
    }

    #[test]
    fn dialect_set_membership_and_all() {
        let all = DialectSet::ALL;
        for dialect in Dialect::ALL {
            assert!(all.contains(*dialect), "{dialect:?}");
        }
        assert!(DialectSet::SNOWFLAKE_ONLY.contains(Dialect::Snowflake));
        assert!(!DialectSet::SNOWFLAKE_ONLY.contains(Dialect::Databricks));
        assert!(DialectSet::DATABRICKS_ONLY.contains(Dialect::Databricks));
        assert!(!DialectSet::DATABRICKS_ONLY.contains(Dialect::Snowflake));
        assert!(DialectSet::EMPTY.is_empty());
        assert!(!DialectSet::ALL.is_empty());
        // `of` builds exactly the requested members.
        let spark_family = DialectSet::of(&[Dialect::Spark, Dialect::Databricks]);
        assert!(spark_family.contains(Dialect::Spark));
        assert!(spark_family.contains(Dialect::Databricks));
        assert!(!spark_family.contains(Dialect::Snowflake));
        assert!(!DialectSet::ALL.union(DialectSet::EMPTY).is_empty());
    }
}
