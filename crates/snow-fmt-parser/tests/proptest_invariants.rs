//! Property-based panic-safety / invariant harness for the **parser**.
//!
//! Audit item (panic-safety, MEDIUM): "no fuzzing or property-testing for panic-safety
//! invariants". This file fuzzes [`snow_fmt_parser::parse`] with thousands of generated inputs and
//! asserts the parser's load-bearing guarantees on every one of them:
//!
//! * `parse(s)` **never panics** — the calls below run inside proptest, which would surface any
//!   panic as a failing (and shrunk) case.
//! * the green tree **round-trips byte-for-byte**: `parse(s).syntax().to_string() == s`. This is the
//!   losslessness invariant the whole toolchain rests on (formatting, highlighting, recovery).
//! * the tree is **total over the bytes**: every leaf token's text, concatenated in order, equals
//!   the input, and no leaf is empty.
//!
//! Inputs come from three complementary strategies (see [`mod gen`]):
//!   1. arbitrary Unicode/`String` (stress the lexer's UTF-8 boundary handling),
//!   2. arbitrary ASCII (denser coverage of the punctuation/operator space),
//!   3. a structured "SQL token salad" (keywords, identifiers, literals, `$$` bodies, parens,
//!      operators) glued with varied whitespace — far likelier to drive deep grammar paths than
//!      random noise.
//!
//! Case counts are capped (see `PROPTEST_CASES`) so `cargo test` stays fast.

use proptest::prelude::*;
use snow_fmt_parser::parse;

/// Keep the suite fast and deterministic-ish: a few hundred cases per property is plenty to flush
/// out panics / lossiness without making `cargo test` drag.
const PROPTEST_CASES: u32 = 512;

/// Generators shared by the parser and (conceptually) formatter harnesses.
///
/// Kept inline here because integration-test files cannot share a private module and we must not
/// touch the shared `snow-fmt-test-support` crate; the formatter harness carries its own copy.
mod gen {
    use proptest::prelude::*;

    /// A representative slice of the keyword / contextual-keyword vocabulary plus a handful of
    /// multi-token constructs, so the salad can reach real grammar productions.
    pub const WORDS: &[&str] = &[
        "select", "SELECT", "from", "FROM", "where", "group", "by", "having", "order", "limit",
        "offset", "as", "and", "or", "not", "null", "is", "in", "like", "between", "case", "when",
        "then", "else", "end", "join", "inner", "left", "right", "full", "outer", "cross", "on",
        "using", "with", "recursive", "union", "all", "distinct", "exists", "qualify", "over",
        "partition", "rows", "range", "create", "or", "replace", "table", "view", "drop", "if",
        "exists", "cluster", "clone", "primary", "key", "foreign", "references", "unique", "check",
        "constraint", "default", "comment", "pivot", "unpivot", "lateral", "flatten", "values",
        "grouping", "sets", "rollup", "cube", "asc", "desc", "nulls", "first", "last",
        // plain identifiers (the lexer emits IDENT for these; some are contextual keywords):
        "t", "u", "a", "b", "c", "id", "name", "mydb", "sch", "col1", "x", "y", "z",
    ];

    /// Punctuation and operator lexemes the lexer recognizes (single- and multi-char).
    pub const PUNCT: &[&str] = &[
        "(", ")", "[", "]", "{", "}", ",", ".", ";", ":", "::", ":=", "=", "<>", "!=", "<", "<=",
        ">", ">=", "+", "-", "*", "/", "%", "||", "|", "|>", "->>", "->", "=>", "&", "^", "~", "@",
        "$", "?",
    ];

    /// String / number / variable literal forms.
    pub const LITERALS: &[&str] = &[
        "'abc'", "'it''s'", "''", "42", "3.14", "0", "1e10", "1.5e-3", "$1", "$42", "$name",
        "\"quoted id\"", "\"weird \"\"x\"\"\"",
    ];

    /// Whitespace / trivia glue between tokens (including comments and newlines).
    pub const GLUE: &[&str] = &[
        " ", "  ", "\t", "\n", " \n ", "", "-- c\n", "/* b */", " /*x*/ ", "\n-- y\n",
    ];

    /// One token chosen from the vocabularies above, including `$$`-delimited bodies.
    fn token() -> impl Strategy<Value = String> {
        prop_oneof![
            6 => prop::sample::select(WORDS).prop_map(str::to_owned),
            4 => prop::sample::select(PUNCT).prop_map(str::to_owned),
            3 => prop::sample::select(LITERALS).prop_map(str::to_owned),
            // $$-delimited bodies, including ones whose contents look like SQL or contain
            // a near-terminator (`$` then non-`$`) to exercise the scanner's end detection.
            2 => "[a-z0-9 ;()$\n]{0,12}".prop_map(|body| format!("$${body}$$")),
        ]
    }

    /// A "SQL token salad": 1..=24 tokens separated by random trivia glue. Optionally wrapped so the
    /// generator also reaches statement-shaped inputs.
    pub fn token_salad() -> impl Strategy<Value = String> {
        prop::collection::vec((token(), prop::sample::select(GLUE)), 1..24).prop_map(|parts| {
            let mut s = String::new();
            for (tok, glue) in parts {
                s.push_str(&tok);
                s.push_str(glue);
            }
            s
        })
    }

    /// Arbitrary ASCII bytes rendered as text (dense coverage of the operator/error space).
    pub fn ascii_blob() -> impl Strategy<Value = String> {
        prop::collection::vec(any::<u8>(), 0..64)
            .prop_map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// The losslessness + totality checks every parse must satisfy.
///
/// Returns a [`TestCaseResult`] so the `prop_assert*!` macros can short-circuit; the calling
/// `proptest!` body forwards the result with `?`.
fn assert_parser_invariants(input: &str) -> Result<(), TestCaseError> {
    let parse = parse(input);
    let root = parse.syntax();

    // (1) byte-for-byte round-trip of the green tree.
    prop_assert_eq!(
        root.to_string(),
        input,
        "CST did not round-trip for {:?}",
        input
    );

    // (2) totality: concatenating every leaf token reconstructs the input, no empty leaves.
    let mut reassembled = String::new();
    for tok in root
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
    {
        prop_assert!(
            !tok.text().is_empty(),
            "empty leaf token in tree for {:?}",
            input
        );
        reassembled.push_str(tok.text());
    }
    prop_assert_eq!(
        reassembled,
        input,
        "leaf concatenation did not reconstruct input for {:?}",
        input
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: PROPTEST_CASES,
        // A lossy/panicking parser bug should fail loudly; allow ample shrinking.
        max_shrink_iters: 4096,
        ..ProptestConfig::default()
    })]

    /// Arbitrary Unicode strings: stress UTF-8 boundary handling in the lexer/builder.
    #[test]
    fn parse_arbitrary_unicode_is_lossless(s in ".{0,64}") {
        assert_parser_invariants(&s)?;
    }

    /// Arbitrary ASCII byte blobs.
    #[test]
    fn parse_arbitrary_ascii_is_lossless(s in gen::ascii_blob()) {
        assert_parser_invariants(&s)?;
    }

    /// Structured SQL token salad — reaches real grammar productions and recovery paths.
    #[test]
    fn parse_token_salad_is_lossless(s in gen::token_salad()) {
        assert_parser_invariants(&s)?;
    }
}
