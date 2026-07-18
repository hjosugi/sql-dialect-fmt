//! Query-expression lowering: the `SELECT` pipeline and the query expressions that nest it —
//! subqueries, set operations, `WITH` queries, `MATCH_RECOGNIZE`, and flow pipelines.

use sql_dialect_fmt_syntax::{SyntaxKind, SyntaxNode};
use SyntaxKind::*;

use crate::doc::{
    concat, empty, group, group_expanded, hard_line, indent, join, line, soft_line, space, text,
    Doc,
};

use super::expr::{has_trailing_comma, item_sep};
use super::{trimmed_text, Lowerer};

impl Lowerer {
    /// Lower a `SELECT_STMT`: a `SELECT <list>` header group followed by one clause per line.
    pub(super) fn lower_select(&mut self, select: &SyntaxNode) -> Doc {
        // `SELECT` and any `DISTINCT`/`ALL` quantifier are the statement's leading tokens.
        let mut head = Vec::new();
        let mut list = None;
        let mut clauses = Vec::new();
        for child in select.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                head.push(self.token(token));
            } else if let Some(node) = child.into_node() {
                if node.kind() == SELECT_LIST {
                    list = Some(node);
                } else if is_select_clause(node.kind()) {
                    clauses.push(node);
                }
            }
        }

        let magic_comma = list.as_ref().is_some_and(has_trailing_comma);
        let list_doc = list
            .as_ref()
            .map(|list| self.lower_select_list(list, magic_comma))
            .unwrap_or_else(empty);

        let inner = concat(vec![concat(head), indent(concat(vec![line(), list_doc]))]);
        // A normal header is one group (flat when it fits, else one item per line); a magic comma
        // forces that group to break.
        let header = if magic_comma {
            group_expanded(inner)
        } else {
            group(inner)
        };

        let mut parts = vec![header];
        for clause in clauses {
            parts.push(hard_line());
            self.reset(); // a clause keyword starts its own line with no leading space
            parts.push(self.lower_clause(&clause));
        }
        concat(parts)
    }

    fn lower_select_list(&mut self, list: &SyntaxNode, trailing_comma: bool) -> Doc {
        let items = self.lower_items(list.children().filter(|n| n.kind() == SELECT_ITEM));
        let mut doc = join(item_sep(), items);
        if trailing_comma {
            // Re-emit the author's trailing comma (a token of the list, not of any item).
            doc = concat(vec![doc, text(",")]);
        }
        doc
    }

    /// Lower a single top-level `SELECT` clause. Most are inline; a few get structural layout.
    fn lower_clause(&mut self, clause: &SyntaxNode) -> Doc {
        match clause.kind() {
            FROM_CLAUSE => self.lower_from(clause),
            ORDER_BY_CLAUSE | GROUP_BY_CLAUSE | DISTRIBUTE_BY_CLAUSE | SORT_BY_CLAUSE
            | CLUSTER_BY_CLAUSE => self.lower_keyword_item_list(clause),
            _ => self.lower_node(clause),
        }
    }

    /// `FROM` with each `JOIN` on its own line (aligned under `FROM`); comma-separated tables stay
    /// inline. Layout only — the token stream is untouched.
    fn lower_from(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                parts.push(self.token(token));
            } else if let Some(node) = child.as_node() {
                if matches!(node.kind(), JOIN | LATERAL_VIEW) {
                    parts.push(hard_line());
                    self.reset();
                }
                parts.push(self.lower_node(node));
            }
        }
        concat(parts)
    }

    /// A `KEYWORD item, item` clause (`ORDER BY`, `GROUP BY`) whose items wrap one-per-line when
    /// they do not fit. The leading keywords are the tokens before the first item node; valueless
    /// forms like `GROUP BY ALL` (no item nodes) are emitted as-is.
    pub(super) fn lower_keyword_item_list(&mut self, node: &SyntaxNode) -> Doc {
        let mut head = Vec::new();
        let mut items = Vec::new();
        let mut seen_item = false;
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                if !seen_item {
                    head.push(self.token(token));
                }
                // Commas between items are dropped here; the join re-synthesizes them.
            } else if let Some(node) = child.as_node() {
                seen_item = true;
                self.reset();
                items.push(self.lower_node(node));
            }
        }
        if items.is_empty() {
            return concat(head);
        }
        let body = indent(concat(vec![line(), join(item_sep(), items)]));
        group(concat(vec![concat(head), body]))
    }

    /// `MATCH_RECOGNIZE ( ... )` with each body clause (PARTITION BY / ORDER BY / MEASURES /
    /// PER MATCH / AFTER MATCH SKIP / PATTERN / SUBSET / DEFINE) on its own indented line.
    pub(super) fn lower_match_recognize(&mut self, node: &SyntaxNode) -> Doc {
        let mut head = Vec::new();
        let mut clauses = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                // The header is the `MATCH_RECOGNIZE` word; the parens are synthesized below.
                if token.kind().is_trivia() || matches!(token.kind(), L_PAREN | R_PAREN) {
                    continue;
                }
                head.push(self.token(token));
            } else if let Some(node) = child.as_node() {
                self.reset();
                clauses.push(concat(vec![hard_line(), self.lower_node(node)]));
            }
        }
        self.resume_after(R_PAREN);
        concat(vec![
            concat(head),
            space(),
            text("("),
            indent(concat(clauses)),
            hard_line(),
            text(")"),
        ])
    }

    /// `PATTERN ( <row pattern> )`: the keyword up-cased, the pattern body emitted verbatim so its
    /// regex-like quantifiers (`A+`, `B*`, `(C | D){1,3}`) are never re-spaced.
    pub(super) fn lower_pattern_clause(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                parts.push(self.token(token)); // PATTERN
            } else if let Some(node) = child.as_node() {
                // `node.text()` can carry the leading inter-token space on a reparse; trim it so the
                // single explicit space stays idempotent.
                parts.push(concat(vec![space(), trimmed_text(node)]));
                self.resume_after(R_PAREN);
            }
        }
        concat(parts)
    }

    /// A flow-operator pipeline `<stmt> ->> <stmt> ->> ...`: each statement formatted normally, the
    /// `->>` operator leading each continuation line. No semicolons are inserted between steps.
    pub(super) fn lower_flow(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind() == FLOW_PIPE {
                    parts.push(hard_line());
                    parts.push(text("->>"));
                    parts.push(space());
                }
            } else if let Some(node) = child.as_node() {
                self.reset();
                parts.push(self.lower_query(node));
            }
        }
        concat(parts)
    }

    /// Dispatch a query expression to its structural rule (a bare `SELECT`) or the generic walker.
    pub(super) fn lower_query(&mut self, node: &SyntaxNode) -> Doc {
        match node.kind() {
            SELECT_STMT => self.lower_select(node),
            _ => self.lower_node(node),
        }
    }

    /// A parenthesized subquery `( query )`: inline when it fits, otherwise the body is indented on
    /// its own lines. A multi-clause inner `SELECT` carries hard lines, which force the break.
    pub(super) fn lower_subquery(&mut self, node: &SyntaxNode) -> Doc {
        let open_sep = self.sep_before(L_PAREN);
        let inner = node.children().next();
        self.reset();
        let body = inner.map(|n| self.lower_query(&n)).unwrap_or_else(empty);
        self.resume_after(R_PAREN);
        let content = concat(vec![
            text("("),
            indent(concat(vec![soft_line(), body])),
            soft_line(),
            text(")"),
        ]);
        concat(vec![open_sep, group(content)])
    }

    /// A set operation (`UNION [ALL] / EXCEPT / INTERSECT / MINUS`): each operand on its own line(s)
    /// with the operator keyword(s) between them. Chained operations flatten because the left
    /// operand is itself a `SET_OP`.
    pub(super) fn lower_set_op(&mut self, node: &SyntaxNode) -> Doc {
        let mut operands = Vec::new();
        let mut ops = Vec::new();
        let mut op_started = false;
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                // Reset only before the first operator keyword; the rest (`ALL`/`DISTINCT`) keep
                // their normal spacing so we get `UNION ALL`, not `UNIONALL`.
                if !op_started {
                    self.reset();
                    op_started = true;
                }
                ops.push(self.token(token));
            } else if let Some(node) = child.as_node() {
                self.reset();
                op_started = false;
                operands.push(self.lower_query(node));
            }
        }
        // Consume the operands by value (the `Vec` is discarded afterwards) so the operand subtrees
        // move into the result instead of being cloned.
        let mut operands = operands.into_iter();
        let mut parts = Vec::new();
        if let Some(lhs) = operands.next() {
            parts.push(lhs);
        }
        // Operator keywords (e.g. `UNION ALL`) share one line between the operands.
        parts.push(hard_line());
        parts.push(concat(ops));
        if let Some(rhs) = operands.next() {
            parts.push(hard_line());
            parts.push(rhs);
        }
        concat(parts)
    }

    /// A `WITH` query: the CTE clause, then the main query on its own line.
    pub(super) fn lower_with_query(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for child in node.children() {
            if child.kind() == WITH_CLAUSE {
                self.reset();
                parts.push(self.lower_with_clause(&child));
            } else {
                parts.push(hard_line());
                self.reset();
                parts.push(self.lower_query(&child));
            }
        }
        concat(parts)
    }

    /// `WITH [RECURSIVE] cte AS (...), other AS (...)` — one CTE per line.
    fn lower_with_clause(&mut self, node: &SyntaxNode) -> Doc {
        let mut head = Vec::new(); // `WITH` and an optional `RECURSIVE`
        let mut ctes = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() || token.kind() == COMMA {
                    continue;
                }
                if ctes.is_empty() {
                    head.push(self.token(token));
                }
            } else if let Some(node) = child.as_node() {
                self.reset();
                ctes.push(self.lower_node(node));
            }
        }
        concat(vec![
            concat(head),
            space(),
            join(concat(vec![text(","), hard_line()]), ctes),
        ])
    }
}

fn is_select_clause(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        FROM_CLAUSE
            | WHERE_CLAUSE
            | DISTRIBUTE_BY_CLAUSE
            | SORT_BY_CLAUSE
            | CLUSTER_BY_CLAUSE
            | GROUP_BY_CLAUSE
            | HAVING_CLAUSE
            | QUALIFY_CLAUSE
            | ORDER_BY_CLAUSE
            | LIMIT_CLAUSE
            | OFFSET_CLAUSE
            | START_WITH_CLAUSE
            | CONNECT_BY_CLAUSE
    )
}
