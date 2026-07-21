//! Adjacent-token spacing rules for the generic token walker.

use sql_dialect_fmt_syntax::SyntaxKind;
use SyntaxKind::*;

/// Whether a single space belongs between adjacent tokens `prev` and `cur`.
pub(super) fn needs_space(prev: SyntaxKind, cur: SyntaxKind) -> bool {
    // Keep a numeric literal followed by a standalone dot from merging into a different token
    // (`0 .` must not become the float literal `0.` in lenient statement tails).
    if matches!(prev, INT_NUMBER | FLOAT_NUMBER) && cur == DOT {
        return true;
    }
    if prev == DOT && matches!(cur, INT_NUMBER | FLOAT_NUMBER) {
        return true;
    }
    // Tokens that hug what precedes them.
    if matches!(
        cur,
        COMMA | SEMICOLON | R_PAREN | R_BRACKET | R_BRACE | DOT | COLON | COLON2
    ) {
        return false;
    }
    // Tokens that the following token hugs.
    if matches!(
        prev,
        DOT | COLON | COLON2 | L_PAREN | L_BRACKET | L_BRACE | AT
    ) {
        return false;
    }
    // `(` opens a call/grouping with no space after a callee or another close bracket; `CAST(`
    // and `TRY_CAST(` are spelled tight too.
    if cur == L_PAREN
        && matches!(
            prev,
            IDENT
                | QUOTED_IDENT
                | R_PAREN
                | R_BRACKET
                | CAST_KW
                | TRY_CAST_KW
                | FLATTEN_KW
                | TABLE_KW
        )
    {
        return false;
    }
    // `[` indexes a value with no leading space: `col[0]`.
    if cur == L_BRACKET && is_value_end(prev) {
        return false;
    }
    true
}

pub(super) fn must_separate_to_preserve_tokens(prev: SyntaxKind, cur: SyntaxKind) -> bool {
    matches!(
        (prev, cur),
        (MINUS, GT)
            | (MINUS, GTE)
            | (MINUS, MINUS)
            | (MINUS, ARROW)
            | (MINUS, FLOW_PIPE)
            | (EQ, GT)
            | (EQ, GTE)
            | (LT, EQ)
            | (LT, GT)
            | (LT, GTE)
            | (LT, FAT_ARROW)
            | (GT, EQ)
            | (GT, FAT_ARROW)
            | (ARROW, GT)
            | (ARROW, GTE)
            | (COLON, EQ)
            | (COLON, COLON)
            | (COLON, ASSIGN)
            | (COLON, COLON2)
            | (COLON, FAT_ARROW)
            | (PIPE, GT)
            | (PIPE, PIPE)
            | (PIPE, CONCAT)
            | (BANG, EQ)
            | (BANG, FAT_ARROW)
            | (SLASH, SLASH)
            | (SLASH, STAR)
    )
}

/// Token kinds that end a value expression: used to tell binary `-`/`+` from unary and to
/// recognize an indexable expression before `[`.
pub(super) fn is_value_end(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        IDENT
            | QUOTED_IDENT
            | STRING
            | INT_NUMBER
            | FLOAT_NUMBER
            | VARIABLE
            | PLACEHOLDER
            | QUESTION
            | R_PAREN
            | R_BRACKET
            | R_BRACE
            | NULL_KW
            | TRUE_KW
            | FALSE_KW
            | END_KW
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sql_dialect_fmt_lexer::tokenize;

    #[test]
    fn compound_operator_boundaries_that_retokenize_are_explicitly_separated() {
        let cases = [
            (EQ, "=", GTE, ">="),
            (LT, "<", GTE, ">="),
            (LT, "<", FAT_ARROW, "=>"),
            (GT, ">", FAT_ARROW, "=>"),
            (ARROW, "->", GT, ">"),
            (ARROW, "->", GTE, ">="),
            (PIPE, "|", CONCAT, "||"),
            (BANG, "!", FAT_ARROW, "=>"),
        ];

        for (prev, prev_text, cur, cur_text) in cases {
            assert!(
                must_separate_to_preserve_tokens(prev, cur),
                "missing boundary protection for {prev:?} followed by {cur:?}"
            );

            let joined = format!("{prev_text}{cur_text}");
            let joined_kinds: Vec<_> = tokenize(&joined)
                .tokens
                .into_iter()
                .filter(|token| !token.kind.is_trivia())
                .map(|token| token.kind)
                .collect();
            assert_ne!(
                joined_kinds,
                vec![prev, cur],
                "the test case is not a lexically ambiguous boundary: {joined:?}"
            );
        }
    }
}
