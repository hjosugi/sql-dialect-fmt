//! Shared helpers for integration tests.
//!
//! Keep domain-specific cases in each test file, but keep mechanical invariants
//! here: losslessness, byte ranges, "clean parse", and table-driven token checks.

pub mod highlight {
    use snow_fmt_highlight::{highlight, HighlightKind, Highlighted};

    pub type HighlightPair<'a> = (&'a str, HighlightKind);

    pub fn interesting_highlights(input: &str) -> Vec<HighlightPair<'_>> {
        highlight(input)
            .tokens
            .into_iter()
            .filter(|token| !matches!(token.kind, HighlightKind::Whitespace))
            .map(|token| (token.text, token.kind))
            .collect()
    }

    pub fn assert_highlight_lossless(input: &str) -> Highlighted<'_> {
        assert_highlight_lossless_with_context(input, input)
    }

    pub fn assert_highlight_lossless_with_context<'a>(
        input: &'a str,
        context: &str,
    ) -> Highlighted<'a> {
        let highlighted = highlight(input);
        assert!(
            highlighted.errors.is_empty(),
            "unexpected highlight/lexer errors for {context}: {:?}",
            highlighted.errors
        );
        let joined: String = highlighted.tokens.iter().map(|token| token.text).collect();
        assert_eq!(
            joined, input,
            "highlight text must round-trip for {context}"
        );
        for token in &highlighted.tokens {
            assert_eq!(
                &input[token.range.clone()],
                token.text,
                "highlight range must point at token text for {context}"
            );
        }
        highlighted
    }

    pub fn assert_interesting_highlights(input: &str, expected: &[HighlightPair<'_>]) {
        assert_eq!(interesting_highlights(input).as_slice(), expected);
        assert_highlight_lossless(input);
    }
}

pub mod lexer {
    use snow_fmt_lexer::{tokenize, Lexed, SyntaxKind};

    pub type LexPair<'a> = (SyntaxKind, &'a str);

    pub fn lex_pairs(input: &str) -> Vec<LexPair<'_>> {
        tokenize(input)
            .tokens
            .into_iter()
            .map(|token| (token.kind, token.text))
            .collect()
    }

    pub fn lex_non_trivia(input: &str) -> Vec<LexPair<'_>> {
        tokenize(input)
            .tokens
            .into_iter()
            .filter(|token| !token.kind.is_trivia())
            .map(|token| (token.kind, token.text))
            .collect()
    }

    pub fn assert_lex_lossless(input: &str) -> Lexed<'_> {
        let lexed = tokenize(input);
        assert_lexed_lossless(input, &lexed);
        lexed
    }

    pub fn assert_lexed_lossless(input: &str, lexed: &Lexed<'_>) {
        let joined: String = lexed.tokens.iter().map(|token| token.text).collect();
        assert_eq!(joined, input, "lexer text must round-trip for {input:?}");
        assert!(
            lexed.tokens.iter().all(|token| !token.text.is_empty()),
            "lexer must not emit empty tokens for {input:?}: {:?}",
            lexed.tokens
        );
    }

    pub fn assert_lexes_to(input: &str, expected: &[LexPair<'_>]) {
        assert_eq!(lex_pairs(input).as_slice(), expected, "for {input:?}");
        assert_lex_lossless(input);
    }

    pub fn assert_lexes_non_trivia_to(input: &str, expected: &[LexPair<'_>]) {
        assert_eq!(lex_non_trivia(input).as_slice(), expected, "for {input:?}");
        assert_lex_lossless(input);
    }
}

pub mod parser {
    use snow_fmt_parser::{parse, Parse, SyntaxKind};

    pub fn assert_parse_roundtrip(input: &str) -> Parse {
        assert_parse_roundtrip_with_context(input, input)
    }

    pub fn assert_parse_roundtrip_with_context(input: &str, context: &str) -> Parse {
        let parsed = parse(input);
        assert_eq!(
            parsed.syntax().to_string(),
            input,
            "parse tree must round-trip for {context}"
        );
        parsed
    }

    pub fn assert_parse_clean(input: &str) -> Parse {
        assert_parse_clean_with_context(input, input)
    }

    pub fn assert_parse_clean_with_context(input: &str, context: &str) -> Parse {
        let parsed = assert_parse_roundtrip_with_context(input, context);
        assert!(
            parsed.errors().is_empty(),
            "unexpected parse errors for {context}: {:?}",
            parsed.errors()
        );
        parsed
    }

    pub fn assert_parse_recovers(input: &str) -> Parse {
        assert_parse_roundtrip_with_context(input, input)
    }

    pub fn assert_parse_recovers_with_context(input: &str, context: &str) -> Parse {
        assert_parse_roundtrip_with_context(input, context)
    }

    pub fn has_node_kind(input: &str, kind: SyntaxKind) -> bool {
        parse(input)
            .syntax()
            .descendants()
            .any(|node| node.kind() == kind)
    }

    pub fn assert_has_node_kind(input: &str, kind: SyntaxKind) {
        assert!(
            has_node_kind(input, kind),
            "expected parse tree for {input:?} to contain {kind:?}"
        );
    }
}
