//! Snowflake SQL formatting rules: lowering the lossless CST into the [`crate::doc`] IR.
//!
//! This is the first slice of Phase 3. It reflows the statement/clause skeleton of the `SELECT`
//! pipeline — statements separated and terminated, each clause on its own line, lists expanding
//! one-item-per-line when they do not fit and honoring a magic trailing comma — while normalizing
//! intra-expression whitespace and upper-casing keywords.
//!
//! ## Comments
//! Comments are attached to the significant tokens they belong to: a comment on the same line as
//! the preceding token trails it (line comments via [`crate::doc::line_suffix`]); a comment on its
//! own line leads the next token. Each comment is emitted exactly once. As a safety net, if any
//! comment cannot be attached to a token we actually render (e.g. it sits on a synthesized
//! punctuation token), the whole statement falls back to a **verbatim** copy, so the formatter
//! never drops or mangles a comment. Round-trip and idempotency tests guard these guarantees.

use std::collections::HashMap;

use snow_fmt_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use SyntaxKind::*;

use crate::doc::{
    break_parent, concat, empty, group, group_expanded, hard_line, indent, join, line, line_suffix,
    soft_line, space, text, Doc,
};

/// Formatting context.
#[derive(Clone, Copy)]
pub(crate) struct Ctx {
    /// Upper-case SQL keywords (opinionated default on).
    pub uppercase_keywords: bool,
}

/// Lower a `SOURCE_FILE` node into a document: each statement formatted, separated by a blank line,
/// and terminated with a semicolon.
///
/// A statement's own leading/interior comments attach *inside* its node and are placed by the
/// statement lowering. Trivia trailing the final statement — including a comment-only file — lands
/// as direct token children of the root; those comments are re-emitted here so nothing is dropped.
pub(crate) fn lower_source(root: &SyntaxNode, ctx: Ctx) -> Doc {
    let mut parts = Vec::new();
    let mut emitted = false;
    for stmt in root.children() {
        if emitted {
            parts.push(text(";"));
            parts.push(hard_line());
            parts.push(hard_line());
        }
        emitted = true;
        parts.push(lower_stmt(&stmt, ctx));
    }
    if emitted {
        parts.push(text(";"));
    }

    // Root-level (trailing) comments, kept verbatim each on its own line.
    let mut need_break = emitted;
    for token in root
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind().is_comment())
    {
        if need_break {
            parts.push(hard_line());
        }
        parts.push(text(token.text().trim_end().to_string()));
        need_break = true;
    }

    concat(parts)
}

/// Lower one statement. Builds its comment attachment, lowers structurally, and — if any comment
/// could not be placed onto an emitted token — falls back to an exact verbatim copy.
fn lower_stmt(stmt: &SyntaxNode, ctx: Ctx) -> Doc {
    let mut low = Lowerer::new(ctx, Comments::build(stmt));
    // Hoist the statement's own leading comments above its first group, so a banner comment does
    // not force the first construct (e.g. the SELECT list) to explode.
    let prefix = low.statement_leading(stmt);
    let body = match stmt.kind() {
        SELECT_STMT => low.lower_select(stmt),
        _ => low.lower_node(stmt),
    };
    if low.comments.all_placed() {
        concat(vec![prefix, body])
    } else {
        verbatim(stmt)
    }
}

// ---- comment attachment ----

/// A single comment, ready to render.
struct CommentInfo {
    text: String,
    /// A `--`/`//` line comment (must end its line) vs a `/* */` block comment (can sit inline).
    is_line: bool,
}

/// Comments of one statement, keyed by the start offset of the significant token they attach to.
/// Entries are *removed* as they are emitted, so a non-empty map afterwards means something was
/// left unplaced.
#[derive(Default)]
struct Comments {
    leading: HashMap<u32, Vec<CommentInfo>>,
    trailing: HashMap<u32, Vec<CommentInfo>>,
}

impl Comments {
    /// Walk the statement's tokens in order, assigning each comment to a significant token: trailing
    /// the previous token when on the same line, otherwise leading the next one.
    fn build(stmt: &SyntaxNode) -> Self {
        let mut comments = Comments::default();
        let mut last_significant: Option<u32> = None;
        let mut newline_since = true; // statement start behaves like "on its own line"
        let mut pending_leading: Vec<CommentInfo> = Vec::new();

        for token in stmt
            .descendants_with_tokens()
            .filter_map(|el| el.into_token())
        {
            let kind = token.kind();
            if kind == NEWLINE {
                newline_since = true;
                continue;
            }
            if kind == WHITESPACE {
                continue;
            }
            if kind.is_comment() {
                let info = CommentInfo {
                    text: token.text().trim_end().to_string(),
                    is_line: kind == COMMENT,
                };
                match last_significant {
                    Some(anchor) if !newline_since => {
                        comments.trailing.entry(anchor).or_default().push(info);
                    }
                    _ => pending_leading.push(info),
                }
                newline_since = false;
                continue;
            }
            // A comma is transparent: we synthesize list separators ourselves and never emit the
            // real comma token, so a comment written after one (`col, -- note`) belongs to the item
            // before it. Keep the anchor and pending leads pointed at the surrounding real tokens.
            if kind == COMMA {
                newline_since = false;
                continue;
            }
            // A significant token: it owns any pending leading comments and becomes the new anchor.
            let start = offset(&token);
            if !pending_leading.is_empty() {
                comments
                    .leading
                    .entry(start)
                    .or_default()
                    .append(&mut pending_leading);
            }
            last_significant = Some(start);
            newline_since = false;
        }

        // Comments with no following token become trailing of the last significant token (dangling).
        if !pending_leading.is_empty() {
            if let Some(anchor) = last_significant {
                comments
                    .trailing
                    .entry(anchor)
                    .or_default()
                    .append(&mut pending_leading);
            }
        }
        comments
    }

    fn all_placed(&self) -> bool {
        self.leading.is_empty() && self.trailing.is_empty()
    }
}

fn offset(token: &SyntaxToken) -> u32 {
    token.text_range().start().into()
}

// ---- the lowerer ----

/// A cursor that walks a subtree in document order, tracking the previous significant token (for
/// spacing) and consuming attached comments as it emits each token.
struct Lowerer {
    ctx: Ctx,
    comments: Comments,
    prev: Option<SyntaxKind>,
    prev_unary: bool,
}

impl Lowerer {
    fn new(ctx: Ctx, comments: Comments) -> Self {
        Lowerer {
            ctx,
            comments,
            prev: None,
            prev_unary: false,
        }
    }

    /// The separator (a space or nothing) that belongs before a token of kind `cur`.
    fn sep_before(&self, cur: SyntaxKind) -> Doc {
        match self.prev {
            Some(prev) if !self.prev_unary && needs_space(prev, cur) => space(),
            _ => empty(),
        }
    }

    fn advance(&mut self, kind: SyntaxKind) {
        self.prev_unary =
            matches!(kind, PLUS | MINUS) && self.prev.is_none_or(|p| !is_value_end(p));
        self.prev = Some(kind);
    }

    /// Reset spacing state so the next token starts a fresh run (used at item/clause boundaries
    /// where the surrounding structure owns the spacing).
    fn reset(&mut self) {
        self.prev = None;
        self.prev_unary = false;
    }

    /// Resume spacing as if the previous significant token were `kind` (used after structurally
    /// emitting a `)` so the following token spaces correctly).
    fn resume_after(&mut self, kind: SyntaxKind) {
        self.prev = Some(kind);
        self.prev_unary = false;
    }

    /// Remove and render the leading comments of the statement's first significant token, each on
    /// its own line. Returned content is placed *before* the statement body (outside its groups) so
    /// it does not force the first construct to break.
    fn statement_leading(&mut self, stmt: &SyntaxNode) -> Doc {
        let first = stmt
            .descendants_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| !t.kind().is_trivia());
        let Some(token) = first else {
            return empty();
        };
        let mut parts = Vec::new();
        for comment in self
            .comments
            .leading
            .remove(&offset(&token))
            .unwrap_or_default()
        {
            parts.push(text(comment.text));
            parts.push(hard_line());
        }
        concat(parts)
    }

    /// Emit a significant token together with any comments attached to it.
    fn token(&mut self, token: &SyntaxToken) -> Doc {
        let start = offset(token);
        let leading = self.comments.leading.remove(&start).unwrap_or_default();
        let trailing = self.comments.trailing.remove(&start).unwrap_or_default();

        let mut parts = Vec::new();
        let has_leading = !leading.is_empty();
        for comment in leading {
            parts.push(text(comment.text));
            parts.push(hard_line());
        }
        // After a leading comment the token begins a fresh line, so it takes no leading space.
        let sep = if has_leading {
            empty()
        } else {
            self.sep_before(token.kind())
        };
        self.advance(token.kind());
        parts.push(sep);
        parts.push(keyword_text(token, self.ctx));
        for comment in trailing {
            if comment.is_line {
                // A line comment must end its line: defer it, and force the line to break.
                parts.push(line_suffix(concat(vec![space(), text(comment.text)])));
                parts.push(break_parent());
            } else {
                parts.push(space());
                parts.push(text(comment.text));
            }
        }
        concat(parts)
    }

    /// Lower a `SELECT_STMT`: a `SELECT <list>` header group followed by one clause per line.
    fn lower_select(&mut self, select: &SyntaxNode) -> Doc {
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

    /// The shared shape of every multi-clause statement: walk children in order, emit the header
    /// inline, and put each clause on its own line. `break_token` marks tokens that *introduce* a
    /// clause (e.g. `FROM`/`USING`/`ON`/`ELSE`) and start a new line; `block_node` marks clause
    /// *nodes* that get their own line (rendered via [`Self::lower_query`], so a bare `SELECT`
    /// source is structured). Everything else is part of the header. Node kinds with bespoke inline
    /// rendering (a verbatim stage location, a spaced column-def list) own that in [`Self::lower_node`].
    fn lower_clausal(
        &mut self,
        node: &SyntaxNode,
        break_token: impl Fn(SyntaxKind) -> bool,
        block_node: impl Fn(SyntaxKind) -> bool,
    ) -> Doc {
        let mut parts = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                if break_token(token.kind()) {
                    parts.push(hard_line());
                    self.reset();
                }
                parts.push(self.token(token));
            } else if let Some(node) = child.as_node() {
                if block_node(node.kind()) {
                    parts.push(hard_line());
                    self.reset();
                    parts.push(self.lower_query(node));
                } else {
                    parts.push(self.lower_node(node));
                }
            }
        }
        concat(parts)
    }

    /// Single-table `INSERT INTO t [(cols)]` with the `VALUES`/query below, or a multi-table
    /// `INSERT {ALL|FIRST}` with each `WHEN`/`INTO`/`ELSE` and the source query on their own lines.
    fn lower_insert(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(
            node,
            |k| k == ELSE_KW,
            |k| {
                matches!(
                    k,
                    INSERT_WHEN
                        | INTO_CLAUSE
                        | VALUES_CLAUSE
                        | SELECT_STMT
                        | SET_OP
                        | SUBQUERY
                        | WITH_QUERY
                )
            },
        )
    }

    /// `UPDATE t` then `SET ...`, `FROM ...`, `WHERE ...` each on its own line.
    fn lower_update(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(
            node,
            |_| false,
            |k| matches!(k, SET_CLAUSE | FROM_CLAUSE | WHERE_CLAUSE),
        )
    }

    /// `DELETE FROM t [USING ...]` then `WHERE ...` on its own line.
    fn lower_delete(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(node, |_| false, |k| k == WHERE_CLAUSE)
    }

    /// `COPY INTO <target> FROM <source>` with `FROM` and each option on their own line.
    fn lower_copy(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(node, |k| k == FROM_KW, |k| k == COPY_OPTION)
    }

    /// `MERGE INTO t USING s ON cond` with `USING`, `ON`, and each `WHEN` clause on their own lines.
    fn lower_merge(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(node, |k| matches!(k, USING_KW | ON_KW), |k| k == MERGE_WHEN)
    }

    /// `CREATE [OR REPLACE] ... TABLE/VIEW ...`: the header inline (a column-def list expanded in
    /// place) and a defining/CTAS query after `AS` on its own line(s).
    fn lower_create(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(
            node,
            |_| false,
            |k| {
                matches!(
                    k,
                    SELECT_STMT | SET_OP | WITH_QUERY | SUBQUERY | VALUES_CLAUSE
                )
            },
        )
    }

    /// A COPY target/source location, emitted verbatim (preserving `@stage/path`, whose `/` operator
    /// spacing would mangle) with the leading-trivia space trimmed for idempotency.
    fn lower_copy_location(&mut self, node: &SyntaxNode) -> Doc {
        let doc = concat(vec![
            space(),
            text(node.text().to_string().trim().to_string()),
        ]);
        self.resume_after(IDENT);
        doc
    }

    /// `( col type ..., col type ... )` — one column definition per line when it does not fit.
    fn lower_column_def_list(&mut self, node: &SyntaxNode) -> Doc {
        let trailing = paren_list_has_trailing_comma(node);
        let defs: Vec<Doc> = node
            .children()
            .filter(|n| n.kind() == COLUMN_DEF)
            .map(|def| {
                self.reset();
                self.lower_node(&def)
            })
            .collect();
        self.resume_after(R_PAREN);
        bracketed(empty(), defs, trailing)
    }

    /// `MATCH_RECOGNIZE ( ... )` with each body clause (PARTITION BY / ORDER BY / MEASURES /
    /// PER MATCH / AFTER MATCH SKIP / PATTERN / SUBSET / DEFINE) on its own indented line.
    fn lower_match_recognize(&mut self, node: &SyntaxNode) -> Doc {
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
    fn lower_pattern_clause(&mut self, node: &SyntaxNode) -> Doc {
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
                let body = node.text().to_string().trim().to_string();
                parts.push(concat(vec![space(), text(body)]));
                self.resume_after(R_PAREN);
            }
        }
        concat(parts)
    }

    /// Lower a single top-level `SELECT` clause. Most are inline; a few get structural layout.
    fn lower_clause(&mut self, clause: &SyntaxNode) -> Doc {
        match clause.kind() {
            FROM_CLAUSE => self.lower_from(clause),
            ORDER_BY_CLAUSE | GROUP_BY_CLAUSE => self.lower_keyword_item_list(clause),
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
                if node.kind() == JOIN {
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
    fn lower_keyword_item_list(&mut self, node: &SyntaxNode) -> Doc {
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

    /// Render a node, normalizing spacing and upper-casing keywords. Most constructs are emitted on
    /// a single (groupable) line by walking their tokens; parenthesized comma lists and `IN (...)`
    /// are lowered structurally so they can wrap and honor a magic trailing comma.
    fn lower_node(&mut self, node: &SyntaxNode) -> Doc {
        match node.kind() {
            // Parenthesized comma lists, lowered structurally (wrap + magic trailing comma).
            ARG_LIST | VALUES_ROW | COLUMN_LIST => self.lower_paren_list(node),
            IN_EXPR => self.lower_in_expr(node),
            CASE_EXPR => self.lower_case(node),
            SUBQUERY => self.lower_subquery(node),
            WITH_QUERY => self.lower_with_query(node),
            SET_OP => self.lower_set_op(node),
            INSERT_STMT => self.lower_insert(node),
            UPDATE_STMT => self.lower_update(node),
            DELETE_STMT => self.lower_delete(node),
            MERGE_STMT => self.lower_merge(node),
            CREATE_STMT => self.lower_create(node),
            COPY_STMT => self.lower_copy(node),
            COPY_LOCATION => self.lower_copy_location(node),
            COLUMN_DEF_LIST => concat(vec![space(), self.lower_column_def_list(node)]),
            MATCH_RECOGNIZE => self.lower_match_recognize(node),
            PATTERN_CLAUSE => self.lower_pattern_clause(node),
            // `SET col = ...` and `VALUES (...), (...)` are keyword + comma-list clauses.
            SET_CLAUSE | VALUES_CLAUSE => self.lower_keyword_item_list(node),
            _ => self.lower_children(node),
        }
    }

    /// The generic fallback: emit a node's significant tokens with spacing, recursing into child
    /// nodes. Used for any construct without a more specific rule.
    fn lower_children(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                parts.push(self.token(token));
            } else if let Some(node) = child.as_node() {
                parts.push(self.lower_node(node));
            }
        }
        concat(parts)
    }

    /// `( item, item )` with width-driven wrapping and magic-trailing-comma explosion. The items
    /// are the node's child *nodes*; parentheses and commas are its tokens. An aggregate quantifier
    /// (`DISTINCT`/`ALL`) is a leading token of the list and is emitted just inside the `(`.
    fn lower_paren_list(&mut self, node: &SyntaxNode) -> Doc {
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

    /// Dispatch a query expression to its structural rule (a bare `SELECT`) or the generic walker.
    fn lower_query(&mut self, node: &SyntaxNode) -> Doc {
        match node.kind() {
            SELECT_STMT => self.lower_select(node),
            _ => self.lower_node(node),
        }
    }

    /// A parenthesized subquery `( query )`: inline when it fits, otherwise the body is indented on
    /// its own lines. A multi-clause inner `SELECT` carries hard lines, which force the break.
    fn lower_subquery(&mut self, node: &SyntaxNode) -> Doc {
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
    fn lower_set_op(&mut self, node: &SyntaxNode) -> Doc {
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
        let mut parts = Vec::new();
        if let Some(lhs) = operands.first() {
            parts.push(lhs.clone());
        }
        // Operator keywords (e.g. `UNION ALL`) share one line between the operands.
        parts.push(hard_line());
        parts.push(concat(ops));
        if let Some(rhs) = operands.get(1) {
            parts.push(hard_line());
            parts.push(rhs.clone());
        }
        concat(parts)
    }

    /// A `WITH` query: the CTE clause, then the main query on its own line.
    fn lower_with_query(&mut self, node: &SyntaxNode) -> Doc {
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

    /// A `CASE` expression: flat when it fits, otherwise one arm per line with `END` dedented:
    ///
    /// ```text
    /// CASE
    ///     WHEN c THEN r
    ///     ELSE e
    /// END
    /// ```
    fn lower_case(&mut self, node: &SyntaxNode) -> Doc {
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
    fn lower_in_expr(&mut self, node: &SyntaxNode) -> Doc {
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
    fn lower_items(&mut self, nodes: impl Iterator<Item = SyntaxNode>) -> Vec<Doc> {
        nodes
            .map(|item| {
                self.reset();
                self.lower_node(&item)
            })
            .collect()
    }
}

/// Build `( items )`: flat when it fits, one-per-line when it does not, and force-exploded (with
/// the preserved trailing comma) when `trailing` is set. An exploded list propagates the break to
/// its ancestors, so a multiline collection never sits inline.
fn bracketed(prefix: Doc, items: Vec<Doc>, trailing: bool) -> Doc {
    if items.is_empty() {
        return concat(vec![text("("), prefix, text(")")]);
    }
    let joined = join(item_sep(), items);
    let body = if trailing {
        concat(vec![soft_line(), joined, text(",")])
    } else {
        concat(vec![soft_line(), joined])
    };
    // `prefix` (e.g. an aggregate `DISTINCT`) hugs the open paren, before the (soft) first break.
    let content = concat(vec![
        text("("),
        prefix,
        indent(body),
        soft_line(),
        text(")"),
    ]);
    if trailing {
        group_expanded(content)
    } else {
        group(content)
    }
}

/// Whether `node`'s last significant child token is a comma (a tolerated trailing comma).
fn has_trailing_comma(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter(|el| !el.kind().is_trivia())
        .last()
        .is_some_and(|el| el.kind() == COMMA)
}

fn is_select_clause(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        FROM_CLAUSE
            | WHERE_CLAUSE
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

/// The separator between comma-list items: a comma, then a space (flat) or newline (broken).
fn item_sep() -> Doc {
    concat(vec![text(","), line()])
}

/// Does a parenthesized list end with `, )` — a tolerated trailing comma? (The last two significant
/// tokens are `COMMA R_PAREN`.)
fn paren_list_has_trailing_comma(node: &SyntaxNode) -> bool {
    let mut prev = None;
    let mut last = None;
    for el in node.children_with_tokens() {
        if !el.kind().is_trivia() {
            prev = last;
            last = Some(el.kind());
        }
    }
    last == Some(R_PAREN) && prev == Some(COMMA)
}

/// Token text, upper-cased if it is a keyword and keyword-casing is enabled.
fn keyword_text(token: &SyntaxToken, ctx: Ctx) -> Doc {
    // Soft (contextual) keywords are tagged `CONTEXTUAL_KEYWORD` rather than living in the keyword
    // range, but they upper-case just like real keywords.
    let is_keyword = token.kind().is_keyword() || token.kind() == CONTEXTUAL_KEYWORD;
    if ctx.uppercase_keywords && is_keyword {
        text(token.text().to_ascii_uppercase())
    } else {
        text(token.text().to_string())
    }
}

/// Whether a single space belongs between adjacent tokens `prev` and `cur`.
fn needs_space(prev: SyntaxKind, cur: SyntaxKind) -> bool {
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

/// Token kinds that end a value expression — used to tell a binary `-`/`+` (after a value) from a
/// unary one, and to recognize an indexable expression before `[`.
fn is_value_end(kind: SyntaxKind) -> bool {
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

/// Reproduce a node's source text exactly (including its inner trivia/comments).
fn verbatim(node: &SyntaxNode) -> Doc {
    text(node.text().to_string())
}
