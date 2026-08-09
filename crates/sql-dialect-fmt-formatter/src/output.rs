//! Candidate-output finalization and semantic safety checks.
//!
//! SQL lowering produces a `Doc`, but printing is not the last step: generated line endings and
//! comment boundaries must be normalized, then the candidate must be checked against the source's
//! meaningful token stream. Keeping that pipeline here prevents formatter orchestration from
//! accumulating lexer-specific postconditions.

use sql_dialect_fmt_lexer::{tokenize_for_dialect, Lexed, Token};
use sql_dialect_fmt_syntax::{Dialect, SyntaxKind, SyntaxNode, SyntaxToken};

use crate::LineEnding;

#[derive(Clone, Debug, PartialEq, Eq)]
struct MeaningfulToken {
    kind: SyntaxKind,
    normalized_text: String,
}

/// Source-side fingerprint used to validate one printed candidate.
///
/// Token kinds must always match. Case-normalized token text must also match unless the exact CST
/// token is an embedded body that lowering intentionally reformats.
pub(crate) struct OutputGuard {
    source: Vec<MeaningfulToken>,
    may_rewrite_text: Vec<bool>,
}

impl OutputGuard {
    pub(crate) fn from_lexed(lexed: &Lexed<'_>) -> Self {
        let source = meaningful_signature(lexed);
        let may_rewrite_text = vec![false; source.len()];
        Self {
            source,
            may_rewrite_text,
        }
    }

    /// Align the source signature with permissions derived from the lossless CST.
    pub(crate) fn record_text_rewrite_permissions(
        &mut self,
        root: &SyntaxNode,
        may_rewrite: impl Fn(&SyntaxToken) -> bool,
    ) {
        self.may_rewrite_text = root
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| is_meaningful(token.kind()))
            .map(|token| may_rewrite(&token))
            .collect();
    }

    /// Whether `candidate` is lexically valid and preserves every meaningful source token.
    pub(crate) fn accepts(&self, candidate: &str, dialect: Dialect) -> bool {
        let lexed = tokenize_for_dialect(candidate, dialect);
        if !lexed.errors.is_empty() || self.source.len() != self.may_rewrite_text.len() {
            return false;
        }
        let mut candidate = meaningful_tokens(&lexed);
        let all_source_tokens_match =
            self.source
                .iter()
                .zip(&self.may_rewrite_text)
                .all(|(source, may_rewrite_text)| {
                    candidate.next().is_some_and(|candidate| {
                        source.kind == candidate.kind
                            && (*may_rewrite_text
                                || source.normalized_text.eq_ignore_ascii_case(candidate.text))
                    })
                });
        all_source_tokens_match && candidate.next().is_none()
    }
}

/// Finish a plain formatted region: apply its requested line endings and normalize comment
/// boundaries before [`OutputGuard`] validates it.
pub(crate) fn finalize_candidate(
    printed: &str,
    source: &str,
    line_ending: LineEnding,
    dialect: Dialect,
) -> String {
    separate_adjacent_comments(apply_line_ending(printed, source, line_ending), dialect)
}

/// Check only token kinds. Used after enabled and verbatim directive regions are concatenated: each
/// enabled region has already passed the stronger [`OutputGuard`] text check, while disabled
/// regions must remain byte-verbatim.
pub(crate) fn preserves_meaningful_token_kinds(
    source: &str,
    candidate: &str,
    dialect: Dialect,
) -> bool {
    if source == candidate {
        return true;
    }
    let source = tokenize_for_dialect(source, dialect);
    let candidate = tokenize_for_dialect(candidate, dialect);
    source.errors.is_empty()
        && candidate.errors.is_empty()
        && meaningful_tokens(&source)
            .map(|token| token.kind)
            .eq(meaningful_tokens(&candidate).map(|token| token.kind))
}

pub(crate) fn apply_line_ending(text: &str, source: &str, line_ending: LineEnding) -> String {
    let target = match line_ending {
        LineEnding::Lf => "\n",
        LineEnding::Crlf => "\r\n",
        LineEnding::Auto => first_line_ending(source).unwrap_or("\n"),
    };
    if target == "\n" {
        text.replace("\r\n", "\n")
    } else {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    }
}

fn meaningful_signature(lexed: &Lexed<'_>) -> Vec<MeaningfulToken> {
    meaningful_tokens(lexed)
        .map(|token| MeaningfulToken {
            kind: token.kind,
            normalized_text: token.text.to_ascii_uppercase(),
        })
        .collect()
}

fn meaningful_tokens<'lexed, 'source: 'lexed>(
    lexed: &'lexed Lexed<'source>,
) -> impl Iterator<Item = &'lexed Token<'source>> {
    lexed
        .tokens
        .iter()
        .filter(|token| is_meaningful(token.kind))
}

fn is_meaningful(kind: SyntaxKind) -> bool {
    !kind.is_trivia() && kind != SyntaxKind::SEMICOLON
}

/// Ensure an emitted comment cannot touch the preceding significant token. Such adjacency is
/// lexically valid (`+-- comment` is still PLUS + COMMENT) but the next formatting pass inserts a
/// space, violating the fixed-point contract. Reconstructing from the lossless token stream keeps
/// quoted text untouched and handles both line and block comments without scanning their spelling.
fn separate_adjacent_comments(formatted: String, dialect: Dialect) -> String {
    let lexed = tokenize_for_dialect(&formatted, dialect);
    if !lexed.errors.is_empty() {
        return formatted;
    }
    let mut separated = String::with_capacity(formatted.len());
    for token in lexed.tokens {
        if token.kind.is_comment()
            && separated
                .chars()
                .next_back()
                .is_some_and(|previous| !previous.is_whitespace())
        {
            separated.push(' ');
        }
        separated.push_str(token.text);
    }
    separated
}

fn first_line_ending(source: &str) -> Option<&'static str> {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => return Some("\n"),
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => return Some("\r\n"),
            _ => index += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_rejects_kind_and_text_changes_but_allows_one_explicit_rewrite() {
        let source = "select 'original'";
        let lexed = tokenize_for_dialect(source, Dialect::Snowflake);
        let mut guard = OutputGuard::from_lexed(&lexed);

        assert!(guard.accepts("SELECT 'original';\n", Dialect::Snowflake));
        assert!(!guard.accepts("SELECT 'changed';\n", Dialect::Snowflake));
        assert!(!guard.accepts("SELECT 1;\n", Dialect::Snowflake));

        let parse = sql_dialect_fmt_parser::parse(source);
        guard.record_text_rewrite_permissions(&parse.syntax(), |token| {
            token.kind() == SyntaxKind::STRING
        });
        assert!(guard.accepts("SELECT 'changed';\n", Dialect::Snowflake));
    }

    #[test]
    fn finalization_separates_adjacent_comments_on_the_first_pass() {
        assert_eq!(
            finalize_candidate(
                "+-- note\n",
                "+\n-- note",
                LineEnding::Auto,
                Dialect::Databricks
            ),
            "+ -- note\n"
        );
    }

    #[test]
    fn directive_composite_check_ignores_only_semicolons_and_trivia() {
        assert!(preserves_meaningful_token_kinds(
            "select a",
            "SELECT a;\n",
            Dialect::Snowflake
        ));
        assert!(!preserves_meaningful_token_kinds(
            "select a",
            "SELECT b + a;\n",
            Dialect::Snowflake
        ));
    }

    #[test]
    fn line_ending_policy_remains_explicit() {
        assert_eq!(apply_line_ending("a\r\nb\n", "", LineEnding::Lf), "a\nb\n");
        assert_eq!(
            apply_line_ending("a\nb\n", "", LineEnding::Crlf),
            "a\r\nb\r\n"
        );
        assert_eq!(
            apply_line_ending("a\nb\n", "source\r\n", LineEnding::Auto),
            "a\r\nb\r\n"
        );
    }
}
