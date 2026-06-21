//! Lower the Snowflake SQL CST into the formatter [`Doc`] IR.
//!
//! Each `lower_*` rule maps one node kind to a `Doc` describing how it should lay out, deferring
//! the flat-vs-broken decision to the printer. The guiding conventions:
//!
//! * **Clauses stack.** Within a `SELECT`, each clause (`FROM`, `WHERE`, …) starts a new line
//!   ([`hard_line`]). Lists *within* a clause use a [`group`] so they stay inline when short and
//!   explode one-per-line when long.
//! * **Keywords are generated, not copied.** Spacing/casing is regenerated from structure, so the
//!   output is canonical and idempotent. Identifiers, literals and quoted names are emitted
//!   verbatim.
//! * **Unhandled nodes fall back to verbatim source text** ([`Ctx::verbatim`]) so the formatter is
//!   *total*: it always produces valid SQL even for a construct it has no dedicated rule for yet.
//! * **Comments are re-anchored to nodes** by [`crate::comments`] and emitted by [`Ctx::lower`]
//!   (leading above / trailing after each node). [`crate::format_with`] verifies the result and
//!   falls back to the original if a comment would be dropped or the output isn't stable.

use snow_fmt_syntax::SyntaxKind::*;
use snow_fmt_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::builder::{
    break_parent, concat, group, hard_line, indent, join, line, line_suffix, nil, soft_line, text,
};
use crate::comments::Comments;
use crate::doc::Doc;
use crate::FormatOptions;

/// Lower a whole `SOURCE_FILE` into a single doc, attaching `src`'s comments to the tree.
pub(crate) fn format_source(root: &SyntaxNode, src: &str, opts: &FormatOptions) -> Doc {
    let comments = Comments::build(root, src);
    Ctx {
        opts,
        comments: &comments,
    }
    .source_file(root)
}

struct Ctx<'a> {
    opts: &'a FormatOptions,
    comments: &'a Comments,
}

impl Ctx<'_> {
    // ---- top level ----

    fn source_file(&self, node: &SyntaxNode) -> Doc {
        // Statement comments are emitted here (not via `attach`) so the synthesized `;` lands
        // tightly after the statement, before any trailing comment.
        let mut parts = Vec::new();
        for stmt in node.children() {
            self.push_leading(&mut parts, &stmt);
            parts.push(self.lower_inner(&stmt));
            parts.push(text(";"));
            self.push_trailing(&mut parts, &stmt);
            parts.push(hard_line());
        }
        concat(parts)
    }

    /// Lower a node and wrap it with any comments attached to that node.
    fn lower(&self, node: &SyntaxNode) -> Doc {
        let inner = self.lower_inner(node);
        let has_comments = !self.comments.leading(node).is_empty()
            || !self.comments.dangling(node).is_empty()
            || !self.comments.trailing(node).is_empty();
        if !has_comments {
            return inner;
        }
        let mut parts = Vec::new();
        self.push_leading(&mut parts, node);
        parts.push(inner);
        self.push_trailing(&mut parts, node);
        concat(parts)
    }

    /// Append a node's leading (own-line, above) and dangling comments to `parts`.
    fn push_leading(&self, parts: &mut Vec<Doc>, node: &SyntaxNode) {
        for comment in self.comments.leading(node) {
            parts.push(text(comment.clone()));
            parts.push(hard_line());
        }
        for comment in self.comments.dangling(node) {
            parts.push(text(comment.clone()));
            parts.push(hard_line());
        }
    }

    /// Append a node's trailing comments to `parts`. End-of-line comments become line suffixes
    /// (printed at the end of the current line, forcing it to break); own-line ones go below.
    fn push_trailing(&self, parts: &mut Vec<Doc>, node: &SyntaxNode) {
        for comment in self.comments.trailing(node) {
            if comment.own_line {
                parts.push(hard_line());
                parts.push(text(comment.text.clone()));
            } else {
                parts.push(line_suffix(concat(vec![
                    text(" "),
                    text(comment.text.clone()),
                ])));
                parts.push(break_parent());
            }
        }
    }

    /// Dispatch a node to its formatting rule, falling back to verbatim source text.
    fn lower_inner(&self, node: &SyntaxNode) -> Doc {
        match node.kind() {
            SELECT_STMT => self.select_stmt(node),
            EXPR_STMT => self
                .first_child(node)
                .map(|c| self.lower(&c))
                .unwrap_or_else(nil),
            WITH_QUERY => self.with_query(node),
            SET_OP => self.set_op(node),
            SUBQUERY => self.subquery(node),
            VALUES_CLAUSE => self.values_clause(node),
            VALUES_ROW => self.paren_list(self.child_nodes(node)),

            FROM_CLAUSE => self.lower_from_clause(node),
            JOIN => self.join_node(node),
            TABLE_REF => self.table_ref(node),
            WHERE_CLAUSE => self.prefixed_expr_clause(node, WHERE_KW),
            HAVING_CLAUSE => self.prefixed_expr_clause(node, HAVING_KW),
            QUALIFY_CLAUSE => self.prefixed_expr_clause(node, QUALIFY_KW),
            GROUP_BY_CLAUSE => self.group_by_clause(node),
            ORDER_BY_CLAUSE => self.order_by_clause(node),
            ORDER_BY_ITEM => self.spaced_pieces(node),
            LIMIT_CLAUSE => self.prefixed_expr_clause(node, LIMIT_KW),
            OFFSET_CLAUSE => self.prefixed_expr_clause(node, OFFSET_KW),
            PARTITION_BY_CLAUSE => self.partition_by_clause(node),

            WITH_CLAUSE => self.with_clause(node),
            CTE => self.cte(node),
            COLUMN_LIST => self.column_list(node),

            SELECT_ITEM => self.select_item(node),
            NAME | NAME_REF => self.verbatim_tokens(node),
            TYPE_NAME => self.type_name(node),
            LITERAL => self.literal(node),
            STAR_EXPR => text("*"),

            PAREN_EXPR => self.paren_expr(node),
            PREFIX_EXPR => self.prefix_expr(node),
            BIN_EXPR => self.bin_expr(node),
            CALL_EXPR => self.call_expr(node),
            ARG_LIST => self.arg_list(node),
            INDEX_EXPR => self.index_expr(node),
            CAST_EXPR => self.cast_expr(node),
            CASE_EXPR => self.case_expr(node),
            JSON_ACCESS => self.json_access(node),
            IS_EXPR => self.is_expr(node),
            IN_EXPR => self.in_expr(node),
            BETWEEN_EXPR => self.between_expr(node),
            EXISTS_EXPR => self.exists_expr(node),
            EXPR_LIST => self.paren_list(self.child_nodes(node)),
            WINDOW_EXPR => self.window_expr(node),
            WINDOW_SPEC => self.window_spec(node),
            WINDOW_FRAME => self.spaced_pieces(node),

            _ => self.verbatim(node),
        }
    }

    // ---- statements & queries ----

    fn select_stmt(&self, node: &SyntaxNode) -> Doc {
        // SELECT [DISTINCT|ALL] <list>, then each remaining clause on its own line.
        let mut head = vec![self.kw("SELECT")];
        if self.has_token(node, DISTINCT_KW) {
            head.push(text(" "));
            head.push(self.kw("DISTINCT"));
        } else if self.has_token(node, ALL_KW) {
            head.push(text(" "));
            head.push(self.kw("ALL"));
        }
        let list_node = self.child_of_kind(node, SELECT_LIST);
        let items = list_node
            .as_ref()
            .map(|list| {
                list.children()
                    .filter(|c| c.kind() == SELECT_ITEM)
                    .map(|item| self.lower(&item))
                    .collect()
            })
            .unwrap_or_default();
        // A comment before the first item (`SELECT -- note\n a`) attaches to SELECT_LIST as
        // leading; emit it above the items, inside the indent.
        let mut list_inner = vec![line()];
        if let Some(list) = &list_node {
            self.push_leading(&mut list_inner, list);
        }
        list_inner.push(join(self.comma_line(), items));
        head.push(indent(concat(list_inner)));

        let mut parts = vec![group(concat(head))];
        // A comment after the whole list (`SELECT a, b -- note`) attaches to SELECT_LIST, which is
        // rendered inline above rather than via `lower`; emit its trailing comment here, outside
        // the group, so it doesn't force the list to explode.
        if let Some(list) = &list_node {
            self.push_trailing(&mut parts, list);
        }
        for clause in node.children() {
            if clause.kind() == SELECT_LIST {
                continue; // already rendered above
            }
            parts.push(hard_line());
            parts.push(self.lower(&clause));
        }
        concat(parts)
    }

    fn with_query(&self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for (i, child) in node.children().enumerate() {
            if i > 0 {
                parts.push(hard_line());
            }
            parts.push(self.lower(&child));
        }
        concat(parts)
    }

    fn with_clause(&self, node: &SyntaxNode) -> Doc {
        let mut head = vec![self.kw("WITH"), text(" ")];
        if self.has_token(node, RECURSIVE_KW) {
            head.push(self.kw("RECURSIVE"));
            head.push(text(" "));
        }
        let ctes: Vec<Doc> = node
            .children()
            .filter(|c| c.kind() == CTE)
            .map(|c| self.lower(&c))
            .collect();
        head.push(join(concat(vec![text(","), hard_line()]), ctes));
        concat(head)
    }

    fn cte(&self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        if let Some(name) = self.child_of_kind(node, NAME) {
            parts.push(self.lower(&name));
        }
        if let Some(cols) = self.child_of_kind(node, COLUMN_LIST) {
            parts.push(text(" "));
            parts.push(self.lower(&cols));
        }
        parts.push(text(" "));
        parts.push(self.kw("AS"));
        parts.push(text(" "));
        if let Some(sub) = self.child_of_kind(node, SUBQUERY) {
            parts.push(self.lower(&sub));
        }
        concat(parts)
    }

    fn set_op(&self, node: &SyntaxNode) -> Doc {
        let operands: Vec<SyntaxNode> = self.child_nodes(node);
        let op = self.spaced_keywords(node); // e.g. UNION ALL
        let mut parts = Vec::new();
        if let Some(lhs) = operands.first() {
            parts.push(self.lower(lhs));
        }
        parts.push(hard_line());
        parts.push(op);
        parts.push(hard_line());
        if let Some(rhs) = operands.get(1) {
            parts.push(self.lower(rhs));
        }
        concat(parts)
    }

    fn subquery(&self, node: &SyntaxNode) -> Doc {
        let inner = self
            .first_child(node)
            .map(|c| self.lower(&c))
            .unwrap_or_else(nil);
        group(concat(vec![
            text("("),
            indent(concat(vec![soft_line(), inner])),
            soft_line(),
            text(")"),
        ]))
    }

    fn values_clause(&self, node: &SyntaxNode) -> Doc {
        let rows: Vec<Doc> = node
            .children()
            .filter(|c| c.kind() == VALUES_ROW)
            .map(|r| self.lower(&r))
            .collect();
        group(concat(vec![
            self.kw("VALUES"),
            indent(concat(vec![line(), join(self.comma_line(), rows)])),
        ]))
    }

    // ---- FROM / JOIN ----

    fn lower_from_clause(&self, node: &SyntaxNode) -> Doc {
        let mut parts = vec![self.kw("FROM"), text(" ")];
        for (i, child) in self.child_nodes(node).into_iter().enumerate() {
            match child.kind() {
                _ if i == 0 => parts.push(self.lower(&child)), // first table reference
                JOIN => {
                    parts.push(hard_line());
                    parts.push(self.lower(&child));
                }
                _ => {
                    // A comma-separated table reference (old-style join).
                    parts.push(text(","));
                    parts.push(hard_line());
                    parts.push(self.lower(&child));
                }
            }
        }
        concat(parts)
    }

    fn join_node(&self, node: &SyntaxNode) -> Doc {
        // The join-type keywords (`[NATURAL] [LEFT OUTER|INNER|…] JOIN`), but not the `ON`/`USING`
        // that introduce the join condition — those are rendered separately below.
        let kw_docs: Vec<Doc> = self
            .tokens(node)
            .into_iter()
            .filter(|t| t.kind().is_keyword() && !matches!(t.kind(), ON_KW | USING_KW))
            .map(|t| text(self.token_text(&t)))
            .collect();
        let mut parts = vec![join(text(" "), kw_docs)];
        if let Some(table) = self.child_of_kind(node, TABLE_REF) {
            parts.push(text(" "));
            parts.push(self.lower(&table));
        }
        if self.has_token(node, ON_KW) {
            // The ON predicate is the (only) expression child of the JOIN node.
            if let Some(pred) = self
                .child_nodes(node)
                .into_iter()
                .find(|c| c.kind() != TABLE_REF)
            {
                parts.push(text(" "));
                parts.push(self.kw("ON"));
                parts.push(text(" "));
                parts.push(group(self.lower(&pred)));
            }
        } else if self.has_token(node, USING_KW) {
            if let Some(cols) = self.child_of_kind(node, COLUMN_LIST) {
                parts.push(text(" "));
                parts.push(self.kw("USING"));
                parts.push(text(" "));
                parts.push(self.lower(&cols));
            }
        }
        concat(parts)
    }

    fn table_ref(&self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        let mut saw_base = false;
        for child in node.children() {
            match child.kind() {
                SUBQUERY | NAME_REF if !saw_base => {
                    parts.push(self.lower(&child));
                    saw_base = true;
                }
                NAME => {
                    // An alias. Emit `AS` only when the source did.
                    parts.push(text(" "));
                    if self.has_token(node, AS_KW) {
                        parts.push(self.kw("AS"));
                        parts.push(text(" "));
                    }
                    parts.push(self.lower(&child));
                }
                COLUMN_LIST => {
                    parts.push(text(" "));
                    parts.push(self.lower(&child));
                }
                _ => parts.push(self.lower(&child)),
            }
        }
        concat(parts)
    }

    // ---- clauses ----

    fn prefixed_expr_clause(&self, node: &SyntaxNode, keyword: SyntaxKind) -> Doc {
        let body = self
            .first_child(node)
            .map(|c| group(self.lower(&c)))
            .unwrap_or_else(nil);
        concat(vec![self.kw_for(keyword), text(" "), body])
    }

    fn group_by_clause(&self, node: &SyntaxNode) -> Doc {
        let head = concat(vec![self.kw("GROUP"), text(" "), self.kw("BY")]);
        if self.has_token(node, ALL_KW) {
            return concat(vec![head, text(" "), self.kw("ALL")]);
        }
        self.keyword_then_list(head, self.lower_each(node))
    }

    fn order_by_clause(&self, node: &SyntaxNode) -> Doc {
        let head = concat(vec![self.kw("ORDER"), text(" "), self.kw("BY")]);
        let items = node
            .children()
            .filter(|c| c.kind() == ORDER_BY_ITEM)
            .map(|c| self.lower(&c))
            .collect();
        self.keyword_then_list(head, items)
    }

    fn partition_by_clause(&self, node: &SyntaxNode) -> Doc {
        let head = concat(vec![self.kw("PARTITION"), text(" "), self.kw("BY")]);
        self.keyword_then_list(head, self.lower_each(node))
    }

    /// `<head> <item>, <item>, …` as a group that explodes the list when it doesn't fit.
    fn keyword_then_list(&self, head: Doc, items: Vec<Doc>) -> Doc {
        group(concat(vec![
            head,
            indent(concat(vec![line(), join(self.comma_line(), items)])),
        ]))
    }

    // ---- select items & names ----

    fn select_item(&self, node: &SyntaxNode) -> Doc {
        let mut children = node.children();
        let expr = children.next();
        let alias = children.next(); // a NAME, if there's an alias
        let mut parts = vec![expr.map(|e| self.lower(&e)).unwrap_or_else(nil)];
        if let Some(alias) = alias {
            parts.push(text(" "));
            if self.has_token(node, AS_KW) {
                parts.push(self.kw("AS"));
                parts.push(text(" "));
            }
            parts.push(self.lower(&alias));
        }
        concat(parts)
    }

    fn column_list(&self, node: &SyntaxNode) -> Doc {
        let names = self
            .child_nodes(node)
            .iter()
            .map(|n| self.lower(n))
            .collect();
        concat(vec![text("("), join(text(", "), names), text(")")])
    }

    fn type_name(&self, node: &SyntaxNode) -> Doc {
        // Reconstruct `NUMBER(38, 0)` from tokens with canonical spacing.
        let mut s = String::new();
        for tok in self.tokens(node) {
            match tok.kind() {
                L_PAREN => s.push('('),
                R_PAREN => s.push(')'),
                COMMA => s.push_str(", "),
                _ => s.push_str(tok.text()),
            }
        }
        text(s)
    }

    fn literal(&self, node: &SyntaxNode) -> Doc {
        match self.tokens(node).into_iter().next() {
            Some(tok) => text(self.token_text(&tok)),
            None => nil(),
        }
    }

    // ---- expressions ----

    fn paren_expr(&self, node: &SyntaxNode) -> Doc {
        let inner = self
            .first_child(node)
            .map(|c| self.lower(&c))
            .unwrap_or_else(nil);
        concat(vec![text("("), inner, text(")")])
    }

    fn prefix_expr(&self, node: &SyntaxNode) -> Doc {
        let operand = self
            .first_child(node)
            .map(|c| self.lower(&c))
            .unwrap_or_else(nil);
        match self.tokens(node).into_iter().next() {
            // `NOT x` needs a space; unary `-x` / `+x` do not.
            Some(tok) if tok.kind() == NOT_KW => concat(vec![self.kw("NOT"), text(" "), operand]),
            Some(tok) => concat(vec![text(self.token_text(&tok)), operand]),
            None => operand,
        }
    }

    fn bin_expr(&self, node: &SyntaxNode) -> Doc {
        let operands = self.child_nodes(node);
        let lhs = operands.first().map(|n| self.lower(n)).unwrap_or_else(nil);
        let rhs = operands.get(1).map(|n| self.lower(n)).unwrap_or_else(nil);
        let op_tokens = self.tokens(node);
        let first_op = op_tokens.first().map(|t| t.kind());

        // AND / OR chains break with the operator leading the continuation line.
        if matches!(first_op, Some(AND_KW) | Some(OR_KW)) {
            let op = self.spaced_tokens(&op_tokens);
            return group(concat(vec![
                lhs,
                indent(concat(vec![line(), op, text(" "), rhs])),
            ]));
        }

        // Everything else stays inline: `a = 1`, `b + c`, `x NOT LIKE y`.
        concat(vec![
            lhs,
            text(" "),
            self.spaced_tokens(&op_tokens),
            text(" "),
            rhs,
        ])
    }

    fn call_expr(&self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for child in node.children() {
            parts.push(self.lower(&child));
        }
        concat(parts)
    }

    fn arg_list(&self, node: &SyntaxNode) -> Doc {
        self.paren_list(self.child_nodes(node))
    }

    fn index_expr(&self, node: &SyntaxNode) -> Doc {
        let operands = self.child_nodes(node);
        let base = operands.first().map(|n| self.lower(n)).unwrap_or_else(nil);
        let index = operands.get(1).map(|n| self.lower(n)).unwrap_or_else(nil);
        concat(vec![base, text("["), index, text("]")])
    }

    fn cast_expr(&self, node: &SyntaxNode) -> Doc {
        let nodes = self.child_nodes(node);
        let expr = nodes.first().map(|n| self.lower(n)).unwrap_or_else(nil);
        let ty = self
            .child_of_kind(node, TYPE_NAME)
            .map(|t| self.lower(&t))
            .unwrap_or_else(nil);
        // Two surface forms share the CAST_EXPR node: `CAST(x AS t)` and the postfix `x :: t`.
        if let Some(kw) = self
            .tokens(node)
            .into_iter()
            .find(|t| matches!(t.kind(), CAST_KW | TRY_CAST_KW))
        {
            concat(vec![
                text(self.token_text(&kw)),
                text("("),
                expr,
                text(" "),
                self.kw("AS"),
                text(" "),
                ty,
                text(")"),
            ])
        } else {
            concat(vec![expr, text("::"), ty])
        }
    }

    fn case_expr(&self, node: &SyntaxNode) -> Doc {
        let children: Vec<SyntaxNode> = node.children().collect();
        // Simple CASE has an operand expression before the first WHEN.
        let operand = children.first().filter(|c| c.kind() != CASE_WHEN).cloned();
        // The ELSE expression, when present, is the trailing non-arm child (after all WHENs).
        let else_expr = if self.has_token(node, ELSE_KW) {
            children
                .last()
                .filter(|c| c.kind() != CASE_WHEN && Some(*c) != operand.as_ref())
                .cloned()
        } else {
            None
        };

        let mut head = vec![self.kw("CASE")];
        if let Some(operand) = &operand {
            head.push(text(" "));
            head.push(self.lower(operand));
        }

        let mut body = Vec::new();
        for when in children.iter().filter(|c| c.kind() == CASE_WHEN) {
            let mut arms = when.children();
            let cond = arms.next().map(|n| self.lower(&n)).unwrap_or_else(nil);
            let result = arms.next().map(|n| self.lower(&n)).unwrap_or_else(nil);
            body.push(concat(vec![
                line(),
                self.kw("WHEN"),
                text(" "),
                cond,
                text(" "),
                self.kw("THEN"),
                text(" "),
                result,
            ]));
        }
        if let Some(else_expr) = else_expr {
            body.push(concat(vec![
                line(),
                self.kw("ELSE"),
                text(" "),
                self.lower(&else_expr),
            ]));
        }

        group(concat(vec![
            concat(head),
            indent(concat(body)),
            line(),
            self.kw("END"),
        ]))
    }

    fn json_access(&self, node: &SyntaxNode) -> Doc {
        // Reconstruct `col:path[0].field` from the base expr plus the path tokens/index exprs.
        let mut parts = Vec::new();
        for element in node.children_with_tokens() {
            if let Some(child) = element.as_node() {
                parts.push(self.lower(child));
            } else if let Some(tok) = element.as_token() {
                if !tok.kind().is_trivia() {
                    parts.push(text(tok.text().to_string()));
                }
            }
        }
        concat(parts)
    }

    fn is_expr(&self, node: &SyntaxNode) -> Doc {
        let operands = self.child_nodes(node);
        let lhs = operands.first().map(|n| self.lower(n)).unwrap_or_else(nil);
        let rhs = operands.get(1).map(|n| self.lower(n)).unwrap_or_else(nil);
        let mut parts = vec![lhs, text(" "), self.kw("IS"), text(" ")];
        if self.has_token(node, NOT_KW) {
            parts.push(self.kw("NOT"));
            parts.push(text(" "));
        }
        parts.push(rhs);
        concat(parts)
    }

    fn in_expr(&self, node: &SyntaxNode) -> Doc {
        let nodes = self.child_nodes(node);
        let lhs = nodes.first().map(|n| self.lower(n)).unwrap_or_else(nil);
        let mut parts = vec![lhs, text(" ")];
        if self.has_token(node, NOT_KW) {
            parts.push(self.kw("NOT"));
            parts.push(text(" "));
        }
        parts.push(self.kw("IN"));
        parts.push(text(" "));
        // The right side is either an EXPR_LIST, or a parenthesized subquery.
        match nodes.get(1) {
            Some(rhs) if rhs.kind() == EXPR_LIST => parts.push(self.lower(rhs)),
            Some(rhs) => parts.push(self.parenthesize(self.lower(rhs))),
            None => {}
        }
        concat(parts)
    }

    fn between_expr(&self, node: &SyntaxNode) -> Doc {
        let nodes = self.child_nodes(node);
        let lhs = nodes.first().map(|n| self.lower(n)).unwrap_or_else(nil);
        let lo = nodes.get(1).map(|n| self.lower(n)).unwrap_or_else(nil);
        let hi = nodes.get(2).map(|n| self.lower(n)).unwrap_or_else(nil);
        let mut parts = vec![lhs, text(" ")];
        if self.has_token(node, NOT_KW) {
            parts.push(self.kw("NOT"));
            parts.push(text(" "));
        }
        parts.extend([
            self.kw("BETWEEN"),
            text(" "),
            lo,
            text(" "),
            self.kw("AND"),
            text(" "),
            hi,
        ]);
        concat(parts)
    }

    fn exists_expr(&self, node: &SyntaxNode) -> Doc {
        let sub = self
            .child_of_kind(node, SUBQUERY)
            .map(|s| self.lower(&s))
            .unwrap_or_else(nil);
        concat(vec![self.kw("EXISTS"), text(" "), sub])
    }

    fn window_expr(&self, node: &SyntaxNode) -> Doc {
        let nodes = self.child_nodes(node);
        let func = nodes.first().map(|n| self.lower(n)).unwrap_or_else(nil);
        let spec = nodes.get(1).map(|n| self.lower(n)).unwrap_or_else(nil);
        concat(vec![func, text(" "), self.kw("OVER"), text(" "), spec])
    }

    fn window_spec(&self, node: &SyntaxNode) -> Doc {
        let parts: Vec<Doc> = self
            .child_nodes(node)
            .iter()
            .map(|n| self.lower(n))
            .collect();
        group(concat(vec![
            text("("),
            indent(concat(vec![soft_line(), join(line(), parts)])),
            soft_line(),
            text(")"),
        ]))
    }

    // ---- shared shapes ----

    /// `( item, item, … )` — flat when it fits, one-per-line when it doesn't.
    fn paren_list(&self, items: Vec<SyntaxNode>) -> Doc {
        if items.is_empty() {
            return text("()");
        }
        let docs = items.iter().map(|n| self.lower(n)).collect();
        group(concat(vec![
            text("("),
            indent(concat(vec![soft_line(), join(self.comma_line(), docs)])),
            soft_line(),
            text(")"),
        ]))
    }

    fn parenthesize(&self, inner: Doc) -> Doc {
        group(concat(vec![
            text("("),
            indent(concat(vec![soft_line(), inner])),
            soft_line(),
            text(")"),
        ]))
    }

    /// Join a node's children (nodes and keyword tokens, in source order) with single spaces.
    /// Used for small grammar fragments like `col DESC NULLS LAST` and window frames.
    fn spaced_pieces(&self, node: &SyntaxNode) -> Doc {
        let mut pieces = Vec::new();
        for element in node.children_with_tokens() {
            if let Some(child) = element.as_node() {
                pieces.push(self.lower(child));
            } else if let Some(tok) = element.as_token() {
                if !tok.kind().is_trivia() {
                    pieces.push(text(self.token_text(tok)));
                }
            }
        }
        join(text(" "), pieces)
    }

    // ---- token helpers ----

    /// Concatenate a node's tokens verbatim with no added spacing (`db.sch.t`, `t.*`).
    fn verbatim_tokens(&self, node: &SyntaxNode) -> Doc {
        let mut s = String::new();
        for tok in self.tokens(node) {
            s.push_str(tok.text());
        }
        text(s)
    }

    /// The exact source text of a node — the total fallback for constructs without a rule yet.
    fn verbatim(&self, node: &SyntaxNode) -> Doc {
        text(node.text().to_string())
    }

    /// All keyword tokens of a node, cased and single-space-joined (`LEFT OUTER JOIN`, `UNION ALL`).
    fn spaced_keywords(&self, node: &SyntaxNode) -> Doc {
        let docs: Vec<Doc> = self
            .tokens(node)
            .into_iter()
            .filter(|t| t.kind().is_keyword())
            .map(|t| text(self.token_text(&t)))
            .collect();
        join(text(" "), docs)
    }

    fn spaced_tokens(&self, tokens: &[SyntaxToken]) -> Doc {
        let docs = tokens.iter().map(|t| text(self.token_text(t))).collect();
        join(text(" "), docs)
    }

    /// Text of a token, applying keyword casing to keyword tokens and leaving others verbatim.
    fn token_text(&self, tok: &SyntaxToken) -> String {
        if tok.kind().is_keyword() {
            self.opts.keyword_case.apply(tok.text())
        } else {
            tok.text().to_string()
        }
    }

    /// A generated keyword (canonical upper-case spelling), cased per the options.
    fn kw(&self, keyword: &str) -> Doc {
        text(self.opts.keyword_case.apply(keyword))
    }

    /// A generated keyword identified by its kind (used by the generic clause helpers).
    fn kw_for(&self, kind: SyntaxKind) -> Doc {
        self.kw(keyword_spelling(kind))
    }

    fn comma_line(&self) -> Doc {
        concat(vec![text(","), line()])
    }

    // ---- CST navigation ----

    fn child_nodes(&self, node: &SyntaxNode) -> Vec<SyntaxNode> {
        node.children().collect()
    }

    /// Lower every child node of `node` to a doc, in source order.
    fn lower_each(&self, node: &SyntaxNode) -> Vec<Doc> {
        node.children().map(|c| self.lower(&c)).collect()
    }

    fn first_child(&self, node: &SyntaxNode) -> Option<SyntaxNode> {
        node.children().next()
    }

    fn child_of_kind(&self, node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
        node.children().find(|c| c.kind() == kind)
    }

    fn tokens(&self, node: &SyntaxNode) -> Vec<SyntaxToken> {
        node.children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !t.kind().is_trivia())
            .collect()
    }

    fn has_token(&self, node: &SyntaxNode, kind: SyntaxKind) -> bool {
        self.tokens(node).iter().any(|t| t.kind() == kind)
    }
}

/// Canonical upper-case spelling for a generated clause keyword.
fn keyword_spelling(kind: SyntaxKind) -> &'static str {
    match kind {
        WHERE_KW => "WHERE",
        HAVING_KW => "HAVING",
        QUALIFY_KW => "QUALIFY",
        LIMIT_KW => "LIMIT",
        OFFSET_KW => "OFFSET",
        _ => "",
    }
}
