use std::ops::Range;

use sql_dialect_fmt_lexer::tokenize;
use sql_dialect_fmt_syntax::SyntaxKind;

#[derive(Clone, Debug)]
pub(crate) struct SpannedToken<'a> {
    pub(crate) kind: SyntaxKind,
    pub(crate) text: &'a str,
    pub(crate) range: Range<usize>,
}

pub(crate) fn spanned_tokens(source: &str) -> Vec<SpannedToken<'_>> {
    let mut offset = 0usize;
    tokenize(source)
        .tokens
        .into_iter()
        .filter_map(|token| {
            let start = offset;
            offset += token.text.len();
            (!token.kind.is_trivia()).then_some(SpannedToken {
                kind: token.kind,
                text: token.text,
                range: start..offset,
            })
        })
        .collect()
}

pub(crate) fn token_at(tokens: &[SpannedToken<'_>], offset: usize) -> Option<usize> {
    tokens
        .iter()
        .position(|token| token.range.start <= offset && offset < token.range.end)
        .or_else(|| {
            offset.checked_sub(1).and_then(|previous| {
                tokens
                    .iter()
                    .position(|token| token.range.start <= previous && previous < token.range.end)
            })
        })
}
