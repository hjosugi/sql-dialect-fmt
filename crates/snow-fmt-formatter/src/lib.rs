//! **snow-fmt-formatter** — the formatting half of the toolchain.
//!
//! Two layers:
//! * [`doc`] — a grammar-agnostic pretty-printing engine (a `Doc` IR + width-aware printer) in the
//!   Wadler → Prettier → biome/ruff lineage. Depends on nothing SQL-specific.
//! * [`sql`] — the Snowflake SQL rules that lower the parser's lossless CST into `Doc`s.
//!
//! [`format`] ties them together: parse → lower → print. Like the parser, it never panics; input
//! it cannot model structurally is preserved verbatim, so formatting is always content-preserving.
//!
//! ```
//! use snow_fmt_formatter::{format, FormatOptions};
//! let out = format("select a,b from t", &FormatOptions::default());
//! assert_eq!(out, "SELECT a, b\nFROM t;\n");
//! ```

pub mod doc;
mod sql;

pub use doc::{print, Doc, PrintOptions};

use sql::Ctx;

/// Options controlling formatting. Opinionated and intentionally small.
#[derive(Clone, Copy, Debug)]
pub struct FormatOptions {
    /// Target line width the printer keeps within where it can.
    pub line_width: usize,
    /// Spaces per indentation level.
    pub indent_width: usize,
    /// Upper-case SQL keywords.
    pub uppercase_keywords: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            line_width: 100,
            indent_width: 4,
            uppercase_keywords: true,
        }
    }
}

impl FormatOptions {
    fn print_options(&self) -> PrintOptions {
        PrintOptions {
            line_width: self.line_width,
            indent_width: self.indent_width,
        }
    }

    fn ctx(&self) -> Ctx {
        Ctx {
            uppercase_keywords: self.uppercase_keywords,
        }
    }
}

/// Format Snowflake SQL source text. Never panics; never drops content.
///
/// Phase 3 scope: we only reflow input the parser fully accepts. If parsing reports any error
/// (i.e. the grammar does not yet model some construct), the source is returned **unchanged** —
/// trivially lossless and idempotent — rather than risking a mangled reflow of a fragmented tree.
pub fn format(source: &str, options: &FormatOptions) -> String {
    let parse = snow_fmt_parser::parse(source);
    if !parse.errors().is_empty() {
        return source.to_string();
    }
    let root = parse.syntax();
    let doc = sql::lower_source(&root, options.ctx());
    print(&doc, &options.print_options())
}
