//! Delimiter definitions for embedded procedure/function bodies.
//!
//! Snowflake currently documents string literal delimiters `'` and `$$` for
//! Snowflake Scripting procedure bodies. The lexer models `$$...$$` as a single
//! body token because embedded JavaScript/Python/SQL can contain arbitrary SQL
//! punctuation and semicolons. Keeping the delimiter as data makes the lexer
//! resilient if Snowflake adds another body delimiter later.

use sql_dialect_fmt_syntax::Dialect;

use crate::{ParamTypes, PlaceholderMatcher};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyDelimiter {
    pub name: &'static str,
    pub opener: &'static str,
    pub closer: &'static str,
}

impl BodyDelimiter {
    pub const fn symmetric(name: &'static str, delimiter: &'static str) -> Self {
        BodyDelimiter {
            name,
            opener: delimiter,
            closer: delimiter,
        }
    }

    pub const fn paired(name: &'static str, opener: &'static str, closer: &'static str) -> Self {
        BodyDelimiter {
            name,
            opener,
            closer,
        }
    }
}

pub const DOLLAR_QUOTED_BODY: BodyDelimiter = BodyDelimiter::symmetric("dollar-quoted body", "$$");

pub const DEFAULT_BODY_DELIMITERS: &[BodyDelimiter] = &[DOLLAR_QUOTED_BODY];

#[derive(Clone, Copy, Debug)]
pub struct LexOptions<'cfg> {
    pub body_delimiters: &'cfg [BodyDelimiter],
    /// The SQL dialect being lexed. Drives quoting and special-token behavior (`$$`/`$n`, `@stage`)
    /// so the same lexer can serve multiple dialects. Defaults to [`Dialect::Snowflake`].
    pub dialect: Dialect,
    /// Overrides which placeholder spellings are recognized. `None` uses
    /// [`ParamTypes::for_dialect`] for [`LexOptions::dialect`].
    pub param_types: Option<ParamTypes>,
    /// Custom placeholder matchers tried at each token start (see [`PlaceholderMatcher`]).
    pub custom_placeholders: &'cfg [&'cfg dyn PlaceholderMatcher],
}

impl Default for LexOptions<'static> {
    fn default() -> Self {
        LexOptions {
            body_delimiters: DEFAULT_BODY_DELIMITERS,
            dialect: Dialect::default(),
            param_types: None,
            custom_placeholders: &[],
        }
    }
}

impl<'cfg> LexOptions<'cfg> {
    /// [`LexOptions`] for `dialect` with the default body delimiters, returning the updated options.
    #[must_use]
    pub fn with_dialect(mut self, dialect: Dialect) -> Self {
        self.dialect = dialect;
        self
    }

    /// Override the recognized placeholder spellings, returning the updated options.
    #[must_use]
    pub fn with_param_types(mut self, param_types: Option<ParamTypes>) -> Self {
        self.param_types = param_types;
        self
    }

    /// Set custom placeholder matchers, returning the updated options.
    #[must_use]
    pub fn with_custom_placeholders(
        mut self,
        custom_placeholders: &'cfg [&'cfg dyn PlaceholderMatcher],
    ) -> Self {
        self.custom_placeholders = custom_placeholders;
        self
    }

    /// The effective placeholder types (the override, or the dialect default).
    #[must_use]
    pub fn effective_param_types(&self) -> ParamTypes {
        self.param_types
            .unwrap_or_else(|| ParamTypes::for_dialect(self.dialect))
    }
}
