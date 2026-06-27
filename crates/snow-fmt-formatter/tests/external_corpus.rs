//! Optional large-corpus formatter harness.
//!
//! This test is ignored by default so the workspace remains self-contained. Point
//! `SNOW_FMT_EXTERNAL_CORPUS` at one or more files/directories (path-list separated) and run:
//!
//! ```sh
//! SNOW_FMT_EXTERNAL_CORPUS=/path/to/sqls \
//!   cargo test -p snow-fmt-formatter --test external_corpus -- --ignored
//! ```

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use snow_fmt_formatter::{format, FormatOptions};
use snow_fmt_lexer::tokenize;
use snow_fmt_parser::parse;
use snow_fmt_syntax::SyntaxKind;

#[test]
#[ignore = "set SNOW_FMT_EXTERNAL_CORPUS to one or more SQL files/directories"]
fn external_corpus_preserves_formatter_invariants() {
    let roots = env::var_os("SNOW_FMT_EXTERNAL_CORPUS")
        .expect("SNOW_FMT_EXTERNAL_CORPUS must point at SQL files/directories");
    let mut files = Vec::new();
    for root in env::split_paths(&roots) {
        collect_sql_files(&root, &mut files);
    }
    files.sort();
    files.dedup();

    let limit = env::var("SNOW_FMT_EXTERNAL_CORPUS_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(usize::MAX);

    let options = FormatOptions::default();
    for file in files.into_iter().take(limit) {
        let bytes = fs::read(&file).unwrap_or_else(|err| panic!("read {}: {err}", file.display()));
        let Ok(source) = String::from_utf8(bytes) else {
            continue;
        };

        let formatted = format(&source, &options);
        assert_eq!(
            formatted,
            format(&formatted, &options),
            "{} is not idempotent",
            file.display()
        );

        let clean_input = tokenize(&source).errors.is_empty() && parse(&source).errors().is_empty();
        if clean_input {
            assert!(
                parse(&formatted).errors().is_empty(),
                "{} formatted output does not parse cleanly",
                file.display()
            );
            assert_eq!(
                signature(&source),
                signature(&formatted),
                "{} changed significant tokens",
                file.display()
            );
        }
    }
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

fn is_sql_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sql"))
}

fn signature(sql: &str) -> Vec<String> {
    tokenize(sql)
        .tokens
        .into_iter()
        .filter(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::SEMICOLON)
        .map(|token| token.text.to_ascii_uppercase())
        .collect()
}
