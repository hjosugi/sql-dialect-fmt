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
        COMMA | SEMICOLON | R_PAREN | R_BRACKET | DOT | COLON | COLON2
    ) {
        return false;
    }
    // Tokens that the following token hugs.
    if matches!(prev, DOT | COLON | COLON2 | L_PAREN | L_BRACKET | AT) {
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
            | (MINUS, MINUS)
            | (EQ, GT)
            | (LT, EQ)
            | (LT, GT)
            | (GT, EQ)
            | (COLON, EQ)
            | (COLON, COLON)
            | (PIPE, GT)
            | (PIPE, PIPE)
            | (BANG, EQ)
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
            | R_PAREN
            | R_BRACKET
            | NULL_KW
            | TRUE_KW
            | FALSE_KW
            | END_KW
    )
}
