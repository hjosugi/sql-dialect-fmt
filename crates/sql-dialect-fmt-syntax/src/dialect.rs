//! The SQL [`Dialect`] selector — the runtime seam for multi-dialect support.
//!
//! The engine (lossless lexer, never-fail parser, Doc-IR formatter) is dialect-agnostic; only the
//! ~20% that differs between dialects — reserved keywords, lexer quoting/special tokens, which
//! statements/operators the grammar accepts, and a few formatter rules — is gated on a [`Dialect`].
//! Modeled on `sqlparser-rs`'s `Dialect` trait: rather than a trait object, a small `Copy` enum
//! threads through the lexer, parser, and formatter, and the divergence points are expressed as
//! `#[must_use]` predicate methods on it.
//!
//! Today every predicate returns the **Snowflake-correct** answer ([`Dialect::Snowflake`] is the
//! [`Default`]), so adding the seam changes no behavior. The Databricks arms encode where the two
//! dialects diverge and are refined in later phases.

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
    /// Databricks SQL / Spark SQL. Behavior is being filled in across later phases; today its
    /// predicate answers describe the *intended* divergence but no Databricks-specific lexing,
    /// parsing, or formatting is wired up yet.
    Databricks,
}

impl Dialect {
    /// Dollar quoting: `$$ ... $$` dollar-quoted bodies and `$1` / `$name` positional/variable
    /// references. Snowflake only — Databricks has no `$$` body or `$n` reference.
    #[must_use]
    pub fn supports_dollar_quoting(self) -> bool {
        matches!(self, Dialect::Snowflake)
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

    /// Backtick-quoted identifiers: `` `col` ``. Databricks only (Snowflake quotes with `"`).
    #[must_use]
    pub fn supports_backtick_identifiers(self) -> bool {
        matches!(self, Dialect::Databricks)
    }

    /// `LATERAL VIEW explode(...)` table-generating clause. Databricks only.
    #[must_use]
    pub fn supports_lateral_view(self) -> bool {
        matches!(self, Dialect::Databricks)
    }

    /// Delta/Spark table DDL options: `USING`, `LOCATION`, `TBLPROPERTIES`, `OPTIONS`, and
    /// `PARTITIONED BY`. Databricks only.
    #[must_use]
    pub fn supports_delta_table_options(self) -> bool {
        matches!(self, Dialect::Databricks)
    }

    /// Higher-order-function lambdas: `x -> expr` and `(x, y) -> expr`. Databricks only for now.
    #[must_use]
    pub fn supports_lambda_expr(self) -> bool {
        matches!(self, Dialect::Databricks)
    }

    /// Time travel via `VERSION AS OF` / `TIMESTAMP AS OF`. Databricks only (Snowflake uses
    /// `AT` / `BEFORE`).
    #[must_use]
    pub fn supports_as_of_travel(self) -> bool {
        matches!(self, Dialect::Databricks)
    }

    /// Delta/Spark maintenance + cache statements — `VACUUM`, `OPTIMIZE … ZORDER BY`,
    /// `INSERT OVERWRITE`, `CACHE`/`UNCACHE`/`REFRESH`, `DESCRIBE HISTORY`, and the
    /// `WHEN NOT MATCHED BY SOURCE`/`INSERT *` MERGE extensions. Databricks only. The leading words
    /// (`VACUUM`, `OPTIMIZE`, `CACHE`, …) are recognized **contextually** at statement start, so they
    /// stay ordinary identifiers under Snowflake (and elsewhere under Databricks), and Snowflake
    /// output is byte-identical.
    #[must_use]
    pub fn supports_delta_commands(self) -> bool {
        matches!(self, Dialect::Databricks)
    }
}

#[cfg(test)]
mod tests {
    use super::Dialect;

    #[test]
    fn default_is_snowflake() {
        assert_eq!(Dialect::default(), Dialect::Snowflake);
    }

    #[test]
    fn snowflake_only_predicates() {
        let s = Dialect::Snowflake;
        assert!(s.supports_dollar_quoting());
        assert!(s.supports_flow_operator());
        assert!(s.supports_copy_into());
        assert!(s.supports_semantic_view());
        assert!(s.supports_scripting_blocks());
        assert!(s.supports_stage_refs());
        assert!(!s.supports_backtick_identifiers());
        assert!(!s.supports_lateral_view());
        assert!(!s.supports_delta_table_options());
        assert!(!s.supports_lambda_expr());
        assert!(!s.supports_as_of_travel());
        assert!(!s.supports_delta_commands());
    }

    #[test]
    fn databricks_only_predicates() {
        let d = Dialect::Databricks;
        assert!(!d.supports_dollar_quoting());
        assert!(!d.supports_flow_operator());
        assert!(!d.supports_copy_into());
        assert!(!d.supports_semantic_view());
        assert!(d.supports_scripting_blocks());
        assert!(!d.supports_stage_refs());
        assert!(d.supports_backtick_identifiers());
        assert!(d.supports_lateral_view());
        assert!(d.supports_delta_table_options());
        assert!(d.supports_lambda_expr());
        assert!(d.supports_as_of_travel());
        assert!(d.supports_delta_commands());
    }
}
