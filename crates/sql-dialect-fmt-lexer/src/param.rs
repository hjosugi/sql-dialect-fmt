//! Placeholder syntax recognition (the `param_types` option).
//!
//! Which spellings count as a prepared-statement placeholder differs by dialect and by project
//! (some use `?`, some `:name`, some `$1`). [`ParamTypes`] is the effective set the lexer consults;
//! it defaults to the dialect's native forms and can be overridden per format call (from
//! `sql-dialect-fmt.toml`). A `true` flag makes the corresponding spelling lex as a single
//! `VARIABLE`/`QUESTION` token instead of operators plus identifiers.

use sql_dialect_fmt_syntax::Dialect;

/// Which placeholder spellings the lexer recognizes as parameter tokens.
///
/// The `Default` is `ParamTypes::for_dialect(Dialect::Snowflake)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamTypes {
    /// `?` (the token is always `QUESTION`; this flag is informational for substitution).
    pub positional: bool,
    /// `?1` numbered positional.
    pub numbered_question: bool,
    /// `:1` numbered.
    pub numbered_colon: bool,
    /// `$1` numbered.
    pub numbered_dollar: bool,
    /// `:name`.
    pub named_colon: bool,
    /// `@name`.
    pub named_at: bool,
    /// `$name`.
    pub named_dollar: bool,
    /// `:"name"`.
    pub quoted_colon: bool,
    /// `@"name"`.
    pub quoted_at: bool,
    /// `$"name"`.
    pub quoted_dollar: bool,
}

impl ParamTypes {
    /// The placeholder spellings a dialect recognizes natively (the pre-`param_types` behavior).
    #[must_use]
    pub const fn for_dialect(dialect: Dialect) -> ParamTypes {
        let dollar = matches!(
            dialect,
            Dialect::Snowflake | Dialect::PostgreSql | Dialect::DuckDb
        );
        let colon = matches!(dialect, Dialect::PlSql);
        let at = matches!(
            dialect,
            Dialect::TransactSql
                | Dialect::MySql
                | Dialect::MariaDb
                | Dialect::TiDb
                | Dialect::SingleStoreDb
        );
        ParamTypes {
            positional: true,
            numbered_question: false,
            numbered_colon: false,
            numbered_dollar: dollar,
            named_colon: colon,
            named_at: at,
            named_dollar: dollar,
            quoted_colon: false,
            quoted_at: false,
            quoted_dollar: false,
        }
    }

    /// Whether `spelling` is recognized, by its user-facing flag name (`positional`, `named_colon`,
    /// …). Used by config parsing and tests.
    #[must_use]
    pub fn get(self, flag: &str) -> Option<bool> {
        Some(match flag {
            "positional" => self.positional,
            "numbered_question" => self.numbered_question,
            "numbered_colon" => self.numbered_colon,
            "numbered_dollar" => self.numbered_dollar,
            "named_colon" => self.named_colon,
            "named_at" => self.named_at,
            "named_dollar" => self.named_dollar,
            "quoted_colon" => self.quoted_colon,
            "quoted_at" => self.quoted_at,
            "quoted_dollar" => self.quoted_dollar,
            _ => return None,
        })
    }

    /// Set a flag by its user-facing name, returning `None` for an unknown name.
    #[must_use]
    pub fn with_flag(mut self, flag: &str, value: bool) -> Option<ParamTypes> {
        match flag {
            "positional" => self.positional = value,
            "numbered_question" => self.numbered_question = value,
            "numbered_colon" => self.numbered_colon = value,
            "numbered_dollar" => self.numbered_dollar = value,
            "named_colon" => self.named_colon = value,
            "named_at" => self.named_at = value,
            "named_dollar" => self.named_dollar = value,
            "quoted_colon" => self.quoted_colon = value,
            "quoted_at" => self.quoted_at = value,
            "quoted_dollar" => self.quoted_dollar = value,
            _ => return None,
        }
        Some(self)
    }

    /// The names of every flag, for error messages and docs.
    pub const FLAGS: &'static [&'static str] = &[
        "positional",
        "numbered_question",
        "numbered_colon",
        "numbered_dollar",
        "named_colon",
        "named_at",
        "named_dollar",
        "quoted_colon",
        "quoted_at",
        "quoted_dollar",
    ];
}

impl Default for ParamTypes {
    fn default() -> Self {
        ParamTypes::for_dialect(Dialect::Snowflake)
    }
}
