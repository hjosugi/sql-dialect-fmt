//! Delimiter definitions for embedded procedure/function bodies.
//!
//! Snowflake currently documents string literal delimiters `'` and `$$` for
//! Snowflake Scripting procedure bodies. The lexer models `$$...$$` as a single
//! body token because embedded JavaScript/Python/SQL can contain arbitrary SQL
//! punctuation and semicolons. Keeping the delimiter as data makes the lexer
//! resilient if Snowflake adds another body delimiter later.

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
}

impl Default for LexOptions<'static> {
    fn default() -> Self {
        LexOptions {
            body_delimiters: DEFAULT_BODY_DELIMITERS,
        }
    }
}
