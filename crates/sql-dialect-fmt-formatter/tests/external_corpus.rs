//! Corpus regression harness for the formatter's hard invariants.
//!
//! This file hosts two layers of the same machinery:
//!
//! * [`sample_corpus_is_clean`] runs unconditionally over the small curated corpus committed under
//!   `tests/corpus_sample/`, so `cargo test --workspace` always exercises end-to-end coverage of the
//!   major statement families.
//! * [`external_corpus_preserves_formatter_invariants`] is `#[ignore]`d by default. Point
//!   `SQL_DIALECT_FMT_EXTERNAL_CORPUS` at one or more files/directories (path-list separated) to run the
//!   identical checks over a large, out-of-repo corpus:
//!
//!   ```sh
//!   SQL_DIALECT_FMT_EXTERNAL_CORPUS=/path/to/sqls \
//!     cargo test -p sql-dialect-fmt-formatter --test external_corpus -- --ignored
//!   ```
//!
//! Both layers assert the same per-file invariants (see [`CorpusFailure`] / [`check_file`]):
//! parse never panics, significant tokens are preserved, formatting is idempotent, and formatted
//! clean input reparses cleanly. See `docs/CORPUS.md` for the full workflow and triage guide.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::tokenize;
use sql_dialect_fmt_parser::parse;
use sql_dialect_fmt_syntax::SyntaxKind;

const EXTERNAL_CORPUS_ENV: &str = "SQL_DIALECT_FMT_EXTERNAL_CORPUS";
const EXTERNAL_CORPUS_LIMIT_ENV: &str = "SQL_DIALECT_FMT_EXTERNAL_CORPUS_LIMIT";

/// A single offending file and the invariant it broke, formatted for a clear test failure.
#[derive(Debug)]
struct CorpusFailure {
    file: PathBuf,
    reason: String,
}

impl std::fmt::Display for CorpusFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "  {}: {}", self.file.display(), self.reason)
    }
}

/// Run every corpus invariant over one file's source text.
fn check_file(file: &Path, source: &str, options: &FormatOptions) -> Result<(), CorpusFailure> {
    let fail = |reason: String| CorpusFailure {
        file: file.to_path_buf(),
        reason,
    };

    // (1) parse must never panic. Running it is the assertion.
    let _ = parse(source);

    // (3) idempotency holds for all input, including verbatim fallback.
    let formatted = format(source, options);
    let reformatted = format(&formatted, options);
    if formatted != reformatted {
        return Err(fail(format!(
            "not idempotent: format(format(x)) != format(x); first diff at byte {}",
            first_diff(&formatted, &reformatted)
        )));
    }

    // The stronger invariants only apply when the toolchain accepts the input cleanly. Input the
    // grammar does not yet model is returned verbatim by `format`, which is trivially lossless and
    // idempotent but should not be held to reparse/round-trip claims.
    let lex_clean = tokenize(source).errors.is_empty();
    let parse_clean = parse(source).errors().is_empty();
    if lex_clean && parse_clean {
        // (4) reparse-clean: formatted output of clean input must itself parse without errors.
        let reparse = parse(&formatted);
        if !reparse.errors().is_empty() {
            return Err(fail(format!(
                "formatted output does not reparse cleanly: {:?}",
                reparse.errors()
            )));
        }

        // (2) significant-token round-trip: formatting may move whitespace and re-case keywords,
        // but the case-folded significant-token stream must be identical.
        let before = significant_tokens(source);
        let after = significant_tokens(&formatted);
        if before != after {
            return Err(fail(format!(
                "significant tokens changed across formatting ({} -> {} tokens)",
                before.len(),
                after.len()
            )));
        }
    }

    Ok(())
}

/// Run [`check_file`] over a set of files, collecting every failure in one report.
fn run_corpus(files: &[PathBuf], label: &str) {
    assert!(!files.is_empty(), "{label}: no .sql files found");

    let options = FormatOptions::default();
    let mut failures = Vec::new();
    for file in files {
        let Ok(bytes) = fs::read(file) else {
            failures.push(CorpusFailure {
                file: file.clone(),
                reason: "could not read file".to_string(),
            });
            continue;
        };
        let Ok(source) = String::from_utf8(bytes) else {
            // Non-UTF-8 corpora are out of scope for the structural checks.
            continue;
        };
        if let Err(failure) = check_file(file, &source, &options) {
            failures.push(failure);
        }
    }

    assert!(
        failures.is_empty(),
        "{label}: {} file(s) violated a formatter invariant:\n{}",
        failures.len(),
        failures
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn sample_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus_sample")
}

#[test]
fn sample_corpus_is_clean() {
    let mut files = Vec::new();
    collect_sql_files(&sample_corpus_dir(), &mut files);
    files.sort();

    assert!(
        files.len() >= 6,
        "expected the curated sample corpus to cover every statement family (>= 6 files), found {}",
        files.len()
    );

    run_corpus(&files, "sample_corpus");

    // The committed samples are stored in formatter-canonical form: `format(x) == x`.
    let options = FormatOptions::default();
    let mut drifted = Vec::new();
    for file in &files {
        let source = fs::read_to_string(file).expect("read sample file");
        let formatted = format(&source, &options);
        if formatted != source {
            drifted.push(CorpusFailure {
                file: file.clone(),
                reason: format!(
                    "sample is not in canonical form; first diff at byte {}",
                    first_diff(&source, &formatted)
                ),
            });
        }
    }
    assert!(
        drifted.is_empty(),
        "sample_corpus: {} sample(s) drifted from canonical form (re-run the formatter and commit \
         the output):\n{}",
        drifted.len(),
        drifted
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
#[ignore = "set SQL_DIALECT_FMT_EXTERNAL_CORPUS to one or more SQL files/directories"]
fn external_corpus_preserves_formatter_invariants() {
    let roots = external_corpus_roots();
    let mut files = Vec::new();
    for root in env::split_paths(&roots) {
        let root = resolve_corpus_root(root);
        collect_sql_files(&root, &mut files);
    }
    files.sort();
    files.dedup();

    let limit = external_corpus_limit();
    files.truncate(limit);

    run_corpus(&files, "external_corpus");
}

fn external_corpus_roots() -> std::ffi::OsString {
    env::var_os(EXTERNAL_CORPUS_ENV)
        .unwrap_or_else(|| panic!("{EXTERNAL_CORPUS_ENV} must point at SQL files/directories"))
}

fn external_corpus_limit() -> usize {
    env::var(EXTERNAL_CORPUS_LIMIT_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(usize::MAX)
}

fn collect_sql_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if is_sql_file(path) {
            out.push(path.to_path_buf());
        }
        return;
    }
    if !path.is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_sql_files(&entry.path(), out);
    }
}

fn resolve_corpus_root(root: PathBuf) -> PathBuf {
    if root.is_absolute() || root.exists() {
        return root;
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .unwrap_or(manifest_dir);
    let workspace_relative = workspace_root.join(&root);
    if workspace_relative.exists() {
        workspace_relative
    } else {
        root
    }
}

fn is_sql_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sql"))
}

fn significant_tokens(sql: &str) -> Vec<String> {
    tokenize(sql)
        .tokens
        .into_iter()
        .filter(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::SEMICOLON)
        .map(|token| token.text.to_ascii_uppercase())
        .collect()
}

fn first_diff(a: &str, b: &str) -> usize {
    a.bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()))
}
