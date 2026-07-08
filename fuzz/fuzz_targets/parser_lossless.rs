#![no_main]

use libfuzzer_sys::fuzz_target;
use sql_dialect_fmt_parser::parse_with_dialect;

mod support;

fuzz_target!(|data: &[u8]| {
    let Some((dialect, source)) = support::sql_input(data) else {
        return;
    };

    let parse = parse_with_dialect(source, dialect);
    let root = parse.syntax();
    assert_eq!(root.to_string(), source, "parser CST did not round-trip");

    let mut reassembled = String::new();
    for token in root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        assert!(
            !token.text().is_empty(),
            "parser CST contains an empty token"
        );
        reassembled.push_str(token.text());
    }
    assert_eq!(
        reassembled, source,
        "parser leaf token stream did not reconstruct input"
    );

    for error in parse.errors() {
        support::assert_span_in_source(error.offset, error.len, source.len());
    }
});
