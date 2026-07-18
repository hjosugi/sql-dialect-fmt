//! Keyword-run lowering for lenient statement tails, with the contextual-keyword word lists that
//! govern their casing.

use sql_dialect_fmt_syntax::SyntaxKind::*;
use sql_dialect_fmt_syntax::SyntaxNode;

use crate::doc::{concat, Doc};

use super::Lowerer;

impl Lowerer {
    fn lower_keyword_run(&mut self, node: &SyntaxNode, force_word: fn(&str) -> bool) -> Doc {
        let mut parts = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                let force =
                    matches!(token.kind(), IDENT | CONTEXTUAL_KEYWORD) && force_word(token.text());
                parts.push(self.token_cased(token, force));
            } else if let Some(node) = child.as_node() {
                let node_text = node.text().to_string();
                if matches!(node.kind(), NAME | NAME_REF) && force_word(node_text.trim()) {
                    parts.push(self.lower_keyword_run(node, force_word));
                } else {
                    parts.push(self.lower_node(node));
                }
            }
        }
        concat(parts)
    }

    pub(super) fn lower_lenient_stmt(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_keyword_run(node, is_lenient_contextual_keyword)
    }

    pub(super) fn lower_time_travel(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_keyword_run(node, is_time_travel_contextual_keyword)
    }

    pub(super) fn lower_sample_clause(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_keyword_run(node, is_sample_contextual_keyword)
    }
}

fn word_in(word: &str, set: &[&str]) -> bool {
    set.iter()
        .any(|candidate| word.eq_ignore_ascii_case(candidate))
}

fn is_lenient_contextual_keyword(word: &str) -> bool {
    word_in(
        word,
        &[
            "abort",
            "add",
            "after",
            "all",
            "alter",
            "before",
            "bernoulli",
            "cascade",
            "column",
            "columns",
            "comment",
            "condition",
            "continue",
            "copy",
            "csv",
            "data",
            "database",
            "decimal",
            "default",
            "dynamic",
            "execute",
            "exists",
            "file",
            "format",
            "handler",
            "history",
            "if",
            "int",
            "integer",
            "json",
            "location",
            "not",
            "null",
            "offset",
            "option",
            "parquet",
            "policy",
            "rename",
            "restrict",
            "return",
            "role",
            "row",
            "schema",
            "seed",
            "set",
            "share",
            "stage",
            "statement",
            "stream",
            "string",
            "system",
            "table",
            "tables",
            "task",
            "timestamp",
            "to",
            "type",
            "unset",
            "user",
            "using",
            "warehouse",
            "work",
        ],
    )
}

fn is_time_travel_contextual_keyword(word: &str) -> bool {
    word_in(word, &["at", "before", "offset", "statement", "timestamp"])
}

fn is_sample_contextual_keyword(word: &str) -> bool {
    word_in(
        word,
        &["bernoulli", "block", "repeatable", "seed", "system"],
    )
}
