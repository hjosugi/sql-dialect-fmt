#![no_main]

use libfuzzer_sys::fuzz_target;
use sql_dialect_fmt_lexer::tokenize_for_dialect;

mod support;

fuzz_target!(|data: &[u8]| {
    let Some((dialect, source)) = support::sql_input(data) else {
        return;
    };

    let lexed = tokenize_for_dialect(source, dialect);
    let mut reassembled = String::new();
    for token in &lexed.tokens {
        assert!(!token.text.is_empty(), "lexer emitted an empty token");
        reassembled.push_str(token.text);
    }
    assert_eq!(reassembled, source, "lexer token stream was not lossless");

    for error in &lexed.errors {
        support::assert_span_in_source(error.offset, error.len, source.len());
    }
});
