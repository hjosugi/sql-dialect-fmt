//! **sql-dialect-fmt-formatter** — the formatting half of the toolchain.
//!
//! Two layers:
//! * [`doc`] — a grammar-agnostic pretty-printing engine (a `Doc` IR + width-aware printer) in the
//!   Wadler → Prettier → biome/ruff lineage. Depends on nothing SQL-specific.
//! * `sql` — the Snowflake SQL rules that lower the parser's lossless CST into `Doc`s.
//!
//! [`format()`] ties them together: parse → lower → print. Like the parser, it never panics; input
//! it cannot model structurally is preserved verbatim, so formatting is always content-preserving.
//!
//! ```
//! use sql_dialect_fmt_formatter::{format, FormatOptions};
//! let out = format("select a,b from t", &FormatOptions::default());
//! assert_eq!(out, "SELECT a, b\nFROM t;\n");
//! ```
//!
//! ## Public API stability
//!
//! [`FormatOptions`] is the configuration entry point and is `#[non_exhaustive]`: build it from
//! [`FormatOptions::default`] and refine it with the `with_*` methods so adding future knobs stays
//! backwards compatible.

pub mod doc;
mod sql;

#[doc(inline)]
pub use doc::{print, Doc, PrintOptions};
use sql_dialect_fmt_parser::ParseError;
pub use sql_dialect_fmt_syntax::Dialect;

use sql::Ctx;

/// Options controlling formatting. Opinionated and intentionally small.
///
/// This type is `#[non_exhaustive]`: future releases may add knobs without it being a breaking
/// change. Consequently, callers in other crates cannot build it with a struct literal. Start from
/// [`FormatOptions::default`] and adjust it through the `with_*` builder methods instead:
///
/// ```
/// use sql_dialect_fmt_formatter::FormatOptions;
/// let options = FormatOptions::default()
///     .with_line_width(80)
///     .with_indent_width(2)
///     .with_uppercase_keywords(false);
/// assert_eq!(options.line_width, 80);
/// assert_eq!(options.indent_width, 2);
/// assert!(!options.uppercase_keywords);
/// ```
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct FormatOptions {
    /// Target line width the printer keeps within where it can.
    pub line_width: usize,
    /// Spaces per indentation level.
    pub indent_width: usize,
    /// Upper-case SQL keywords.
    pub uppercase_keywords: bool,
    /// The SQL dialect to parse and format. Defaults to [`Dialect::Snowflake`].
    pub dialect: Dialect,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            line_width: 100,
            indent_width: 4,
            uppercase_keywords: true,
            dialect: Dialect::Snowflake,
        }
    }
}

impl FormatOptions {
    /// Set the target line width (the column the printer tries to keep lines within), returning the
    /// updated options so calls can be chained.
    #[must_use]
    pub fn with_line_width(mut self, line_width: usize) -> Self {
        self.line_width = line_width;
        self
    }

    /// Set the number of spaces per indentation level, returning the updated options so calls can be
    /// chained.
    #[must_use]
    pub fn with_indent_width(mut self, indent_width: usize) -> Self {
        self.indent_width = indent_width;
        self
    }

    /// Choose whether SQL keywords are upper-cased, returning the updated options so calls can be
    /// chained.
    #[must_use]
    pub fn with_uppercase_keywords(mut self, uppercase_keywords: bool) -> Self {
        self.uppercase_keywords = uppercase_keywords;
        self
    }

    /// Set the SQL dialect to parse and format, returning the updated options so calls can be
    /// chained.
    #[must_use]
    pub fn with_dialect(mut self, dialect: Dialect) -> Self {
        self.dialect = dialect;
        self
    }

    fn print_options(&self) -> PrintOptions {
        PrintOptions {
            line_width: self.line_width,
            indent_width: self.indent_width,
        }
    }

    fn ctx(&self) -> Ctx {
        Ctx {
            uppercase_keywords: self.uppercase_keywords,
            line_width: self.line_width,
            indent_width: self.indent_width,
            dialect: self.dialect,
        }
    }
}

/// Output plus parse diagnostics from a single formatter pass.
#[derive(Clone, Debug)]
pub struct FormatResult {
    /// The formatted SQL, or the original source when lexing/parsing fails or a protected token
    /// shape requires verbatim preservation.
    pub formatted: String,
    /// Parser diagnostics collected from the same parse used for formatting.
    pub parse_errors: Vec<ParseError>,
}

/// Format Snowflake SQL source text. Never panics; never drops content.
///
/// Phase 3 scope: we only reflow input the lexer and parser fully accept. If lexing or parsing
/// reports any error (i.e. the grammar does not yet model some construct, or a token is
/// unterminated), the source is returned **unchanged** — trivially lossless and idempotent — rather
/// than risking a mangled reflow of a fragmented tree.
pub fn format(source: &str, options: &FormatOptions) -> String {
    format_with_diagnostics(source, options).formatted
}

/// Format SQL and return parse diagnostics from the same lex/parse pass.
pub fn format_with_diagnostics(source: &str, options: &FormatOptions) -> FormatResult {
    let ctx = options.ctx();
    let lexed = sql_dialect_fmt_lexer::tokenize_for_dialect(source, ctx.dialect);
    let has_lex_errors = !lexed.errors.is_empty();
    let has_multiline_trailing_space = lexed.tokens.iter().any(|token| {
        !token.kind.is_trivia() && multiline_token_has_line_trailing_space(token.text)
    });
    let parse = sql_dialect_fmt_parser::parse_lexed(source, ctx.dialect, lexed);
    let parse_errors = parse.errors().to_vec();
    if has_lex_errors || has_multiline_trailing_space || !parse_errors.is_empty() {
        return FormatResult {
            formatted: source.to_string(),
            parse_errors,
        };
    }
    let root = parse.syntax();
    let doc = sql::lower_source(&root, ctx);
    FormatResult {
        formatted: print(&doc, &options.print_options()),
        parse_errors,
    }
}

pub(crate) fn multiline_token_has_line_trailing_space(text: &str) -> bool {
    text.lines()
        .any(|line| line.ends_with(' ') || line.ends_with('\t'))
}
