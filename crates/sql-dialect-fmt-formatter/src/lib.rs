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
pub mod range;
mod sql;

#[doc(inline)]
pub use doc::{print, Doc, PrintOptions};
pub use range::{format_range, RangeEdit};
use sql_dialect_fmt_parser::ParseError;
pub use sql_dialect_fmt_syntax::Dialect;

use sql::Ctx;

/// How SQL keywords should be cased in formatted output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeywordCase {
    /// Upper-case recognized keywords (`SELECT`, `FROM`).
    Upper,
    /// Lower-case recognized keywords (`select`, `from`).
    Lower,
    /// Preserve the source spelling of recognized keywords.
    Preserve,
}

/// Output line-ending policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LineEnding {
    /// Use the first line ending found in the input, falling back to LF when there is none.
    Auto,
    /// Always write LF (`\n`).
    Lf,
    /// Always write CRLF (`\r\n`).
    Crlf,
}

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
///     .with_keyword_case(sql_dialect_fmt_formatter::KeywordCase::Preserve);
/// assert_eq!(options.line_width, 80);
/// assert_eq!(options.indent_width, 2);
/// assert_eq!(options.keyword_case, sql_dialect_fmt_formatter::KeywordCase::Preserve);
/// ```
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct FormatOptions {
    /// Target line width the printer keeps within where it can.
    pub line_width: usize,
    /// Spaces per indentation level.
    pub indent_width: usize,
    /// Upper-case SQL keywords.
    ///
    /// Kept for 1.x source compatibility. New code should use [`FormatOptions::keyword_case`] or
    /// [`FormatOptions::with_keyword_case`]. When this is set to `false` and `keyword_case` is still
    /// [`KeywordCase::Upper`], the formatter treats it as [`KeywordCase::Preserve`].
    pub uppercase_keywords: bool,
    /// Keyword casing policy.
    pub keyword_case: KeywordCase,
    /// Output line-ending policy.
    pub line_ending: LineEnding,
    /// The SQL dialect to parse and format. Defaults to [`Dialect::Snowflake`].
    pub dialect: Dialect,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            line_width: 100,
            indent_width: 4,
            uppercase_keywords: true,
            keyword_case: KeywordCase::Upper,
            line_ending: LineEnding::Lf,
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
        self.keyword_case = if uppercase_keywords {
            KeywordCase::Upper
        } else {
            KeywordCase::Preserve
        };
        self
    }

    /// Choose keyword casing, returning the updated options so calls can be chained.
    #[must_use]
    pub fn with_keyword_case(mut self, keyword_case: KeywordCase) -> Self {
        self.keyword_case = keyword_case;
        self.uppercase_keywords = matches!(keyword_case, KeywordCase::Upper);
        self
    }

    /// Choose output line endings, returning the updated options so calls can be chained.
    #[must_use]
    pub fn with_line_ending(mut self, line_ending: LineEnding) -> Self {
        self.line_ending = line_ending;
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
            keyword_case: self.effective_keyword_case(),
            line_width: self.line_width,
            indent_width: self.indent_width,
            dialect: self.dialect,
        }
    }

    fn effective_keyword_case(&self) -> KeywordCase {
        if !self.uppercase_keywords && self.keyword_case == KeywordCase::Upper {
            KeywordCase::Preserve
        } else {
            self.keyword_case
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
    if let Some(result) = format_directive_regions(source, options) {
        return result;
    }
    format_plain_with_diagnostics(source, options, 0)
}

fn format_plain_with_diagnostics(
    source: &str,
    options: &FormatOptions,
    base_offset: usize,
) -> FormatResult {
    let ctx = options.ctx();
    let lexed = sql_dialect_fmt_lexer::tokenize_for_dialect(source, ctx.dialect);
    let has_lex_errors = !lexed.errors.is_empty();
    let has_multiline_trailing_space = lexed.tokens.iter().any(|token| {
        !token.kind.is_trivia() && multiline_token_has_line_trailing_space(token.text)
    });
    let parse = sql_dialect_fmt_parser::parse_lexed(source, ctx.dialect, lexed);
    let mut parse_errors = parse.errors().to_vec();
    if base_offset > 0 {
        adjust_parse_errors(&mut parse_errors, base_offset);
    }
    if has_lex_errors || has_multiline_trailing_space || !parse_errors.is_empty() {
        return FormatResult {
            formatted: source.to_string(),
            parse_errors,
        };
    }
    let root = parse.syntax();
    let doc = sql::lower_source(&root, ctx);
    let printed = print(&doc, &options.print_options());
    FormatResult {
        formatted: apply_line_ending(&printed, source, options.line_ending),
        parse_errors,
    }
}

fn format_directive_regions(source: &str, options: &FormatOptions) -> Option<FormatResult> {
    let regions = directive_regions(source)?;
    let mut formatted = String::new();
    let mut parse_errors = Vec::new();
    for region in regions {
        let text = &source[region.start..region.end];
        if region.enabled {
            let result = format_plain_with_diagnostics(text, options, region.start);
            formatted.push_str(&result.formatted);
            parse_errors.extend(result.parse_errors);
        } else {
            formatted.push_str(text);
        }
    }
    let index = sql_dialect_fmt_text::LineIndex::new(source);
    for error in &mut parse_errors {
        error.line_column = Some(index.line_column(error.offset));
    }
    let formatted = apply_line_ending(&formatted, source, options.line_ending);
    Some(FormatResult {
        formatted,
        parse_errors,
    })
}

#[derive(Clone, Copy)]
struct FormatRegion {
    start: usize,
    end: usize,
    enabled: bool,
}

fn directive_regions(source: &str) -> Option<Vec<FormatRegion>> {
    let mut regions = Vec::new();
    let mut disabled = false;
    let mut region_start = 0usize;
    let mut line_start = 0usize;
    let mut saw_directive = false;

    for line in source.split_inclusive('\n') {
        let line_end = line_start + line.len();
        match format_directive(line) {
            Some(FormatDirective::Off) if !disabled => {
                saw_directive = true;
                if region_start < line_start {
                    regions.push(FormatRegion {
                        start: region_start,
                        end: line_start,
                        enabled: true,
                    });
                }
                disabled = true;
                region_start = line_start;
            }
            Some(FormatDirective::On) if disabled => {
                saw_directive = true;
                regions.push(FormatRegion {
                    start: region_start,
                    end: line_end,
                    enabled: false,
                });
                disabled = false;
                region_start = line_end;
            }
            _ => {}
        }
        line_start = line_end;
    }

    if line_start < source.len() {
        let line = &source[line_start..];
        match format_directive(line) {
            Some(FormatDirective::Off) if !disabled => {
                saw_directive = true;
                if region_start < line_start {
                    regions.push(FormatRegion {
                        start: region_start,
                        end: line_start,
                        enabled: true,
                    });
                }
                disabled = true;
                region_start = line_start;
            }
            Some(FormatDirective::On) if disabled => {
                saw_directive = true;
                regions.push(FormatRegion {
                    start: region_start,
                    end: source.len(),
                    enabled: false,
                });
                disabled = false;
                region_start = source.len();
            }
            _ => {}
        }
    }

    if region_start < source.len() {
        regions.push(FormatRegion {
            start: region_start,
            end: source.len(),
            enabled: !disabled,
        });
    }
    saw_directive.then_some(regions)
}

#[derive(Clone, Copy)]
enum FormatDirective {
    Off,
    On,
}

fn format_directive(line: &str) -> Option<FormatDirective> {
    let trimmed = line.trim_start();
    let body = trimmed.strip_prefix("--")?.trim_start();
    for prefix in ["sql-dialect-fmt:", "snowfmt:", "fmt:"] {
        if let Some(rest) = body.strip_prefix(prefix) {
            let word = rest.split_whitespace().next()?;
            if word.eq_ignore_ascii_case("off") {
                return Some(FormatDirective::Off);
            }
            if word.eq_ignore_ascii_case("on") {
                return Some(FormatDirective::On);
            }
        }
    }
    None
}

fn adjust_parse_errors(errors: &mut [ParseError], base_offset: usize) {
    for error in errors {
        error.offset += base_offset;
    }
}

fn apply_line_ending(text: &str, source: &str, line_ending: LineEnding) -> String {
    let target = match line_ending {
        LineEnding::Lf => "\n",
        LineEnding::Crlf => "\r\n",
        LineEnding::Auto => first_line_ending(source).unwrap_or("\n"),
    };
    if target == "\n" {
        text.replace("\r\n", "\n")
    } else {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    }
}

fn first_line_ending(source: &str) -> Option<&'static str> {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => return Some("\n"),
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => return Some("\r\n"),
            _ => i += 1,
        }
    }
    None
}

pub(crate) fn multiline_token_has_line_trailing_space(text: &str) -> bool {
    text.lines()
        .any(|line| line.ends_with(' ') || line.ends_with('\t'))
}
