#![no_main]

use libfuzzer_sys::fuzz_target;
use sql_dialect_fmt_formatter::{format, FormatOptions};
use sql_dialect_fmt_lexer::tokenize_for_dialect;
use sql_dialect_fmt_parser::parse_with_dialect;
use sql_dialect_fmt_syntax::{Dialect, SyntaxKind};

mod support;

fuzz_target!(|data: &[u8]| {
    let Some((dialect, source)) = support::sql_input(data) else {
        return;
    };

    let options = FormatOptions::default().with_dialect(dialect);
    let once = format(source, &options);
    let twice = format(&once, &options);
    assert_eq!(twice, once, "formatter was not idempotent");

    if !well_formed(source, dialect) {
        return;
    }

    let reparsed = parse_with_dialect(&once, dialect);
    assert!(
        reparsed.errors().is_empty(),
        "formatted output did not parse cleanly: {:?}",
        reparsed.errors()
    );
    assert_eq!(
        signature(source, dialect),
        signature(&once, dialect),
        "formatter changed the meaningful token sequence"
    );
});

fn well_formed(source: &str, dialect: Dialect) -> bool {
    tokenize_for_dialect(source, dialect).errors.is_empty()
        && parse_with_dialect(source, dialect).errors().is_empty()
}

fn signature(source: &str, dialect: Dialect) -> Vec<String> {
    tokenize_for_dialect(source, dialect)
        .tokens
        .into_iter()
        .filter(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::SEMICOLON)
        .map(|token| token.text.to_ascii_uppercase())
        .collect()
}
