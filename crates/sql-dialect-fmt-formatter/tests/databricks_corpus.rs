//! Committed Databricks/Spark SQL corpus harness.
//!
//! Runs the formatter's hard invariants over the ~15 representative Databricks `.sql` files under
//! `tests/databricks_corpus/`, all under the **Databricks** dialect:
//!   * parse never panics,
//!   * byte-exact lossless round-trip (`parse(s).syntax().to_string() == s`),
//!   * formatting is idempotent,
//!   * formatted clean input re-parses clean,
//!   * the case-folded significant-token stream is preserved, and
//!   * each committed sample is already in Databricks-canonical form (`format(x) == x`), so drift
//!     surfaces immediately.
//!
//! The fixtures are kept in canonical form; regenerate with the `databricks_canonicalize` example
//! (see the module-level comment in that example) and commit the output.

use std::fs;
use std::path::{Path, PathBuf};

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::tokenize_for_dialect;
use sql_dialect_fmt_parser::parse_with_dialect;
use sql_dialect_fmt_syntax::{Dialect, SyntaxKind};

fn options() -> FormatOptions {
    FormatOptions::default().with_dialect(Dialect::Databricks)
}

fn significant_tokens(sql: &str) -> Vec<String> {
    tokenize_for_dialect(sql, Dialect::Databricks)
        .tokens
        .into_iter()
        .filter(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::SEMICOLON)
        .map(|token| token.text.to_ascii_uppercase())
        .collect()
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("databricks_corpus")
}

fn collect_sql_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read databricks_corpus dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("sql"))
        .collect();
    files.sort();
    files
}

fn first_diff(a: &str, b: &str) -> usize {
    a.bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()))
}

#[test]
fn databricks_corpus_preserves_invariants() {
    let files = collect_sql_files(&corpus_dir());
    assert!(
        files.len() >= 15,
        "expected >= 15 Databricks corpus files, found {}",
        files.len()
    );

    let opts = options();
    let mut failures = Vec::new();
    for file in &files {
        let source = fs::read_to_string(file).expect("read corpus file");

        // (1) parse must never panic — running it is the assertion.
        let parsed = parse_with_dialect(&source, Dialect::Databricks);

        // (2) lossless round-trip.
        if parsed.syntax().to_string() != source {
            failures.push(format!(
                "{}: parse tree does not round-trip (first diff @ byte {})",
                file.display(),
                first_diff(&source, &parsed.syntax().to_string())
            ));
            continue;
        }

        // (3) idempotency.
        let formatted = format(&source, &opts);
        let reformatted = format(&formatted, &opts);
        if formatted != reformatted {
            failures.push(format!(
                "{}: not idempotent (first diff @ byte {})",
                file.display(),
                first_diff(&formatted, &reformatted)
            ));
            continue;
        }

        // The stronger invariants apply only when the toolchain accepts the input cleanly.
        let lex_clean = tokenize_for_dialect(&source, Dialect::Databricks)
            .errors
            .is_empty();
        if lex_clean && parsed.errors().is_empty() {
            // (4) reparse-clean.
            let reparse = parse_with_dialect(&formatted, Dialect::Databricks);
            if !reparse.errors().is_empty() {
                failures.push(format!(
                    "{}: formatted output does not reparse clean: {:?}",
                    file.display(),
                    reparse.errors()
                ));
                continue;
            }
            // (5) significant-token preservation.
            if significant_tokens(&source) != significant_tokens(&formatted) {
                failures.push(format!(
                    "{}: significant tokens changed across formatting",
                    file.display()
                ));
                continue;
            }
        } else {
            failures.push(format!(
                "{}: committed corpus file should parse clean under Databricks but did not: {:?}",
                file.display(),
                parsed.errors()
            ));
            continue;
        }

        // (6) canonical form: committed samples are stored as `format(x) == x`.
        if formatted != source {
            failures.push(format!(
                "{}: not in Databricks-canonical form (first diff @ byte {}); regenerate and commit",
                file.display(),
                first_diff(&source, &formatted)
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "databricks_corpus: {} file(s) violated an invariant:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
