//! The `T!` shorthand macro: punctuation/operator token → [`crate::SyntaxKind`].
//!
//! `T![->>]` reads better than `SyntaxKind::FLOW_PIPE` in parser code (this is rust-analyzer's
//! `T!` convention). Keyword kinds are referenced directly (e.g. `SyntaxKind::SELECT_KW`) for
//! now; they can be folded into `T!` via codegen once the grammar stabilizes.
//!
//! Delimiters use the quoted-char trick (`T!['(']`) so the macro arguments stay balanced.

/// Map a punctuation/operator token to its [`crate::SyntaxKind`].
#[macro_export]
macro_rules! T {
    [,] => { $crate::SyntaxKind::COMMA };
    [;] => { $crate::SyntaxKind::SEMICOLON };
    ['('] => { $crate::SyntaxKind::L_PAREN };
    [')'] => { $crate::SyntaxKind::R_PAREN };
    ['['] => { $crate::SyntaxKind::L_BRACKET };
    [']'] => { $crate::SyntaxKind::R_BRACKET };
    ['{'] => { $crate::SyntaxKind::L_BRACE };
    ['}'] => { $crate::SyntaxKind::R_BRACE };
    [.] => { $crate::SyntaxKind::DOT };
    [:] => { $crate::SyntaxKind::COLON };
    [::] => { $crate::SyntaxKind::COLON2 };
    [:=] => { $crate::SyntaxKind::ASSIGN };
    [=] => { $crate::SyntaxKind::EQ };
    [<>] => { $crate::SyntaxKind::NEQ };
    [<] => { $crate::SyntaxKind::LT };
    [<=] => { $crate::SyntaxKind::LTE };
    [>] => { $crate::SyntaxKind::GT };
    [>=] => { $crate::SyntaxKind::GTE };
    [+] => { $crate::SyntaxKind::PLUS };
    [-] => { $crate::SyntaxKind::MINUS };
    [*] => { $crate::SyntaxKind::STAR };
    [/] => { $crate::SyntaxKind::SLASH };
    [%] => { $crate::SyntaxKind::PERCENT };
    [||] => { $crate::SyntaxKind::CONCAT };
    [|] => { $crate::SyntaxKind::PIPE };
    [|>] => { $crate::SyntaxKind::PIPE_GT };
    [->>] => { $crate::SyntaxKind::FLOW_PIPE };
    [->] => { $crate::SyntaxKind::ARROW };
    [=>] => { $crate::SyntaxKind::FAT_ARROW };
    [&] => { $crate::SyntaxKind::AMP };
    [^] => { $crate::SyntaxKind::CARET };
    [~] => { $crate::SyntaxKind::TILDE };
    [@] => { $crate::SyntaxKind::AT };
    [?] => { $crate::SyntaxKind::QUESTION };
}

#[cfg(test)]
mod tests {
    // `T!` is already in scope crate-wide via `#[macro_use] mod macros;`.
    use crate::SyntaxKind;

    #[test]
    fn t_macro_maps_punctuation() {
        assert_eq!(T![,], SyntaxKind::COMMA);
        assert_eq!(T![;], SyntaxKind::SEMICOLON);
        assert_eq!(T!['('], SyntaxKind::L_PAREN);
        assert_eq!(T![')'], SyntaxKind::R_PAREN);
        assert_eq!(T!['['], SyntaxKind::L_BRACKET);
        assert_eq!(T![']'], SyntaxKind::R_BRACKET);
        assert_eq!(T!['{'], SyntaxKind::L_BRACE);
        assert_eq!(T!['}'], SyntaxKind::R_BRACE);
        assert_eq!(T![.], SyntaxKind::DOT);
        assert_eq!(T![::], SyntaxKind::COLON2);
        assert_eq!(T![:=], SyntaxKind::ASSIGN);
        assert_eq!(T![<>], SyntaxKind::NEQ);
        assert_eq!(T![<=], SyntaxKind::LTE);
        assert_eq!(T![>=], SyntaxKind::GTE);
        assert_eq!(T![|>], SyntaxKind::PIPE_GT);
        assert_eq!(T![->>], SyntaxKind::FLOW_PIPE);
        assert_eq!(T![||], SyntaxKind::CONCAT);
        assert_eq!(T![|], SyntaxKind::PIPE);
        assert_eq!(T![=>], SyntaxKind::FAT_ARROW);
        assert_eq!(T![->], SyntaxKind::ARROW);
        assert_eq!(T![@], SyntaxKind::AT);
        assert_eq!(T![*], SyntaxKind::STAR);
        assert_eq!(T![?], SyntaxKind::QUESTION);
    }
}
