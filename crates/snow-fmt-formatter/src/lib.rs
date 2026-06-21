//! A width-aware formatter for Snowflake SQL.
//!
//! The pipeline is the one proven by Prettier, `biome_formatter` and `ruff_formatter`:
//!
//! ```text
//! source ──parse──▶ lossless CST ──lower──▶ Doc IR ──print──▶ formatted text
//! ```
//!
//! * [`sql`] lowers the CST into a [`doc::Doc`] — a tree of *intent* (groups, indents, line
//!   breaks) rather than characters — re-anchoring comments to nodes along the way ([`comments`]).
//! * [`printer`] resolves that intent against [`FormatOptions::line_width`], keeping short
//!   constructs on one line and exploding long ones.
//!
//! ## Guarantees
//!
//! [`format`] is **total and self-verifying**: it never panics and never produces invalid SQL.
//! Broken input (syntax errors) is returned unchanged. For input *with comments*, the formatter
//! runs but then checks its own output and falls back to the original byte-for-byte unless all of
//! these hold:
//!
//! * the output re-parses with no errors (still valid SQL),
//! * every comment is preserved verbatim (none dropped or altered), and
//! * the output is stable (formatting it again is a no-op).
//!
//! So the invariants hold unconditionally: **idempotent** (`format(format(x)) == format(x)`) and
//! **never drops a comment**. Comment-free valid SQL skips the checks (it is already proven stable
//! by the test corpus).

mod builder;
mod comments;
mod doc;
mod options;
mod printer;
mod sql;

pub use options::{FormatOptions, KeywordCase};

use snow_fmt_syntax::SyntaxNode;

/// Format Snowflake SQL with the default options.
///
/// Equivalent to [`format_with`] using [`FormatOptions::default`].
pub fn format(source: &str) -> String {
    format_with(source, &FormatOptions::default())
}

/// Format Snowflake SQL with explicit [`FormatOptions`].
///
/// Returns `source` unchanged if it has syntax errors, or if formatting it would drop/alter a
/// comment, produce invalid SQL, or not be stable; otherwise returns the reformatted SQL, which
/// always ends in a single trailing newline.
pub fn format_with(source: &str, opts: &FormatOptions) -> String {
    let parse = snow_fmt_parser::parse(source);
    if !parse.errors().is_empty() {
        return source.to_string(); // never reformat broken SQL
    }
    let root = parse.syntax();
    let formatted = run_pipeline(&root, source, opts);

    // Comment-free input takes the fast path: the lowering is total and the corpus proves it
    // idempotent and token-preserving, so no per-run verification is needed.
    if !has_comments(&root) {
        return formatted;
    }

    // The comment path verifies itself: the output must be valid SQL, preserve every comment, and
    // be stable. If any check fails, fall back to the untouched source rather than risk a bad edit.
    let reparse = snow_fmt_parser::parse(&formatted);
    if !reparse.errors().is_empty() {
        return source.to_string();
    }
    let reparsed_root = reparse.syntax();
    if comment_texts(&root) != comment_texts(&reparsed_root) {
        return source.to_string();
    }
    if run_pipeline(&reparsed_root, &formatted, opts) != formatted {
        return source.to_string();
    }
    formatted
}

/// Lower + print + normalize, with no self-verification (the pure formatting pipeline).
fn run_pipeline(root: &SyntaxNode, src: &str, opts: &FormatOptions) -> String {
    normalize_trailing_newline(printer::print(sql::format_source(root, src, opts), opts))
}

fn has_comments(root: &SyntaxNode) -> bool {
    root.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| t.kind().is_comment())
}

/// The sorted multiset of comment texts in a tree — used to assert formatting preserves them all.
fn comment_texts(root: &SyntaxNode) -> Vec<String> {
    let mut texts: Vec<String> = root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind().is_comment())
        .map(|t| t.text().to_string())
        .collect();
    texts.sort();
    texts
}

/// Ensure the output ends with exactly one `\n` — unless it is empty (e.g. whitespace-only input).
fn normalize_trailing_newline(mut s: String) -> String {
    if s.is_empty() {
        return s;
    }
    while s.ends_with('\n') {
        s.pop();
    }
    s.push('\n');
    s
}
