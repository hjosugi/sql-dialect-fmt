//! Expression and delimited-list lowering: parenthesized comma lists, collection literals,
//! `CASE`/`IN`/logical chains, and the shared bracketed-list builders.

use sql_dialect_fmt_syntax::{SyntaxKind, SyntaxNode};
use SyntaxKind::*;

use crate::doc::{
    concat, empty, group, group_expanded, indent, join, line, soft_line, space, text, Doc,
};

use super::Lowerer;

impl Lowerer {
    pub(super) fn lower_window_spec(&mut self, node: &SyntaxNode) -> Doc {
        let open_sep = self.sep_before(L_PAREN);
        let mut segments = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() || matches!(token.kind(), L_PAREN | R_PAREN) {
                    continue;
                }
                self.reset();
                segments.push(self.token(token));
            } else if let Some(node) = child.as_node() {
                self.reset();
                segments.push(self.lower_node(node));
            }
        }
        self.resume_after(R_PAREN);
        concat(vec![open_sep, paren_grouped_segments(segments)])
    }

    pub(super) fn lower_logical_expr(&mut self, node: &SyntaxNode) -> Doc {
        let Some(op) = logical_chain_operator(node) else {
            return self.lower_children(node);
        };
        let mut operands = Vec::new();
        self.collect_logical_operands(node, op, &mut operands);
        if operands.len() < 2 {
            return self.lower_children(node);
        }

        let mut operands = operands.into_iter();
        let mut parts = vec![operands.next().expect("len checked")];
        let op_doc = self.synth_kw(match op {
            AND_KW => "AND",
            OR_KW => "OR",
            _ => unreachable!("logical_chain_operator only returns AND/OR"),
        });
        let mut tail = Vec::new();
        for operand in operands {
            tail.push(line());
            tail.push(op_doc.clone());
            tail.push(space());
            tail.push(operand);
        }
        parts.push(indent(concat(tail)));
        group(concat(parts))
    }

    fn collect_logical_operands(&mut self, node: &SyntaxNode, op: SyntaxKind, out: &mut Vec<Doc>) {
        if node.kind() == BIN_EXPR && logical_chain_operator(node) == Some(op) {
            let children: Vec<_> = node.children().collect();
            if children.len() == 2 {
                self.collect_logical_operands(&children[0], op, out);
                self.reset();
                self.collect_logical_operands(&children[1], op, out);
                return;
            }
        }
        if !out.is_empty() {
            self.reset();
        }
        out.push(self.lower_node(node));
    }

    /// `( item, item )` with width-driven wrapping and magic-trailing-comma explosion. The items
    /// are the node's child *nodes*; parentheses and commas are its tokens. An aggregate quantifier
    /// (`DISTINCT`/`ALL`) is a leading token of the list and is emitted just inside the `(`.
    pub(super) fn lower_paren_list(&mut self, node: &SyntaxNode) -> Doc {
        // A column-name list (`INSERT INTO t (a, b)`, a derived-table alias `t (c1, c2)`, a `USING
        // (a, b)`) attaches to the preceding name/keyword and always takes a leading space — unlike
        // a call's `ARG_LIST`, which hugs its callee (`coalesce(a, b)`). Match `CREATE TABLE t (…)`.
        let open_sep = if node.kind() == COLUMN_LIST && self.prev.is_some() {
            space()
        } else {
            self.sep_before(L_PAREN)
        };
        let trailing = paren_list_has_trailing_comma(node);
        let quantifier = node
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| matches!(t.kind(), DISTINCT_KW | ALL_KW));
        let prefix = if let Some(token) = quantifier {
            self.reset();
            concat(vec![self.token(&token), space()])
        } else {
            empty()
        };
        let items = self.lower_items(node.children());
        self.resume_after(R_PAREN);
        concat(vec![open_sep, bracketed(prefix, items, trailing)])
    }

    pub(super) fn lower_delimited_list(
        &mut self,
        node: &SyntaxNode,
        open: SyntaxKind,
        close: SyntaxKind,
        open_text: &'static str,
        close_text: &'static str,
    ) -> Doc {
        let open_sep = self.sep_before(open);
        let trailing = delimited_list_has_trailing_comma(node, close);
        let items = self.lower_items(node.children());
        self.resume_after(close);
        concat(vec![
            open_sep,
            delimited(open_text, close_text, empty(), items, trailing),
        ])
    }

    pub(super) fn lower_object_field(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                parts.push(self.token(token));
                if token.kind() == COLON {
                    parts.push(space());
                    self.reset();
                }
            } else if let Some(node) = child.as_node() {
                parts.push(self.lower_node(node));
            }
        }
        concat(parts)
    }

    pub(super) fn lower_bind_marker(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        let mut first_significant = true;
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                if first_significant && token.kind() == COLON {
                    let sep = self.sep_before(IDENT);
                    self.reset();
                    parts.push(sep);
                }
                parts.push(self.token(token));
                first_significant = false;
            } else if let Some(node) = child.as_node() {
                parts.push(self.lower_node(node));
                first_significant = false;
            }
        }
        self.resume_after(R_PAREN);
        concat(parts)
    }

    pub(super) fn lower_value_children(&mut self, node: &SyntaxNode) -> Doc {
        let doc = self.lower_children(node);
        self.resume_after(R_PAREN);
        doc
    }

    /// A `CASE` expression: flat when it fits, otherwise one arm per line with `END` dedented:
    ///
    /// ```text
    /// CASE
    ///     WHEN c THEN r
    ///     ELSE e
    /// END
    /// ```
    pub(super) fn lower_case(&mut self, node: &SyntaxNode) -> Doc {
        let mut head = Vec::new(); // `CASE` and an optional simple-CASE operand
        let mut arms = Vec::new(); // each `WHEN .. THEN ..` and the `ELSE ..`
        let mut end = empty();
        let mut else_kw: Option<Doc> = None;
        let mut seen_arm = false;

        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                match token.kind() {
                    k if k.is_trivia() => {}
                    CASE_KW => head.push(self.token(token)),
                    ELSE_KW => {
                        self.reset();
                        else_kw = Some(self.token(token));
                    }
                    END_KW => {
                        self.reset();
                        end = self.token(token);
                    }
                    _ => head.push(self.token(token)),
                }
            } else if let Some(node) = child.as_node() {
                if node.kind() == CASE_WHEN {
                    seen_arm = true;
                    self.reset();
                    arms.push(self.lower_node(node));
                } else if let Some(kw) = else_kw.take() {
                    self.reset();
                    arms.push(concat(vec![kw, space(), self.lower_node(node)]));
                } else if !seen_arm {
                    // A simple CASE operand: `CASE x WHEN ...`.
                    self.reset();
                    head.push(space());
                    head.push(self.lower_node(node));
                }
            }
        }

        let mut body = Vec::new();
        for arm in arms {
            body.push(line());
            body.push(arm);
        }
        self.prev = Some(END_KW);
        self.prev_unary = false;
        group(concat(vec![
            concat(head),
            indent(concat(body)),
            line(),
            end,
        ]))
    }

    /// `x [NOT] IN ( ... )`. Unlike a call's `ARG_LIST`, the parentheses here are tokens of the
    /// `IN_EXPR` itself and the comma list is a nested `EXPR_LIST`, so we stitch them back into the
    /// same structural bracket (a value subquery is rendered inline).
    pub(super) fn lower_in_expr(&mut self, node: &SyntaxNode) -> Doc {
        let elems: Vec<_> = node
            .children_with_tokens()
            .filter(|el| !el.kind().is_trivia())
            .collect();
        let mut parts = Vec::new();
        let mut i = 0;
        while i < elems.len() {
            if elems[i].kind() == L_PAREN {
                if let Some(inner) = elems.get(i + 1).and_then(|e| e.as_node()) {
                    let open_sep = self.sep_before(L_PAREN);
                    if inner.kind() == EXPR_LIST {
                        let trailing = has_trailing_comma(inner);
                        let items = self.lower_items(inner.children());
                        self.resume_after(R_PAREN);
                        parts.push(concat(vec![open_sep, bracketed(empty(), items, trailing)]));
                    } else {
                        // A subquery or query expression: keep the parentheses, render inline.
                        self.reset();
                        let body = self.lower_node(inner);
                        self.resume_after(R_PAREN);
                        parts.push(concat(vec![open_sep, text("("), body, text(")")]));
                    }
                    i += 2; // `(` and the inner node
                    if elems.get(i).map(|e| e.kind()) == Some(R_PAREN) {
                        i += 1; // the matching `)`
                    }
                    continue;
                }
            }
            if let Some(token) = elems[i].as_token() {
                parts.push(self.token(token));
            } else if let Some(node) = elems[i].as_node() {
                parts.push(self.lower_node(node));
            }
            i += 1;
        }
        concat(parts)
    }

    /// Lower each child node as a list item from a fresh spacing state (the surrounding brackets and
    /// the join own all inter-item spacing).
    pub(super) fn lower_items(&mut self, nodes: impl Iterator<Item = SyntaxNode>) -> Vec<Doc> {
        nodes
            .map(|item| {
                self.reset();
                self.lower_node(&item)
            })
            .collect()
    }
}

fn paren_grouped_segments(segments: Vec<Doc>) -> Doc {
    if segments.is_empty() {
        return text("()");
    }
    group(concat(vec![
        text("("),
        indent(concat(vec![soft_line(), join(line(), segments)])),
        soft_line(),
        text(")"),
    ]))
}

fn logical_chain_operator(node: &SyntaxNode) -> Option<SyntaxKind> {
    node.children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .find(|kind| matches!(kind, AND_KW | OR_KW))
}

/// Build `( items )`: flat when it fits, one-per-line when it does not, and force-exploded (with
/// the preserved trailing comma) when `trailing` is set. An exploded list propagates the break to
/// its ancestors, so a multiline collection never sits inline.
pub(super) fn bracketed(prefix: Doc, items: Vec<Doc>, trailing: bool) -> Doc {
    delimited("(", ")", prefix, items, trailing)
}

fn delimited(
    open: &'static str,
    close: &'static str,
    prefix: Doc,
    items: Vec<Doc>,
    trailing: bool,
) -> Doc {
    if items.is_empty() {
        return concat(vec![text(open), prefix, text(close)]);
    }
    let joined = join(item_sep(), items);
    let body = if trailing {
        concat(vec![soft_line(), joined, text(",")])
    } else {
        concat(vec![soft_line(), joined])
    };
    // `prefix` (e.g. an aggregate `DISTINCT`) hugs the open paren, before the (soft) first break.
    let content = concat(vec![
        text(open),
        prefix,
        indent(body),
        soft_line(),
        text(close),
    ]);
    if trailing {
        group_expanded(content)
    } else {
        group(content)
    }
}

/// Whether `node`'s last significant child token is a comma (a tolerated trailing comma).
pub(super) fn has_trailing_comma(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter(|el| !el.kind().is_trivia())
        .last()
        .is_some_and(|el| el.kind() == COMMA)
}

/// The separator between comma-list items: a comma, then a space (flat) or newline (broken).
pub(super) fn item_sep() -> Doc {
    concat(vec![text(","), line()])
}

/// Does a parenthesized list end with `, )` — a tolerated trailing comma? (The last two significant
/// tokens are `COMMA R_PAREN`.)
pub(super) fn paren_list_has_trailing_comma(node: &SyntaxNode) -> bool {
    delimited_list_has_trailing_comma(node, R_PAREN)
}

fn delimited_list_has_trailing_comma(node: &SyntaxNode, close: SyntaxKind) -> bool {
    let mut prev = None;
    let mut last = None;
    for el in node.children_with_tokens() {
        if !el.kind().is_trivia() {
            prev = last;
            last = Some(el.kind());
        }
    }
    last == Some(close) && prev == Some(COMMA)
}
