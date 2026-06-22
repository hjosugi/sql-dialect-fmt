//! Snowflake SQL formatting rules: lowering the lossless CST into the [`crate::doc`] IR.
//!
//! This is the first slice of Phase 3. It reflows the statement/clause skeleton of the `SELECT`
//! pipeline — statements separated and terminated, each clause on its own line, the select list
//! expanding one-item-per-line when it does not fit — while normalizing intra-expression
//! whitespace and upper-casing keywords. Anything it does not yet understand structurally is
//! rendered inline from its tokens; any subtree containing a comment or an `ERROR` node is emitted
//! **verbatim** so the formatter never drops or mangles content it cannot model. Round-trip and
//! idempotency tests guard those guarantees.

use snow_fmt_syntax::{SyntaxKind, SyntaxNode};
use SyntaxKind::*;

use crate::doc::{
    concat, empty, group, group_expanded, hard_line, indent, join, line, soft_line, space, text,
    Doc,
};

/// Formatting context threaded through lowering.
#[derive(Clone, Copy)]
pub(crate) struct Ctx {
    /// Upper-case SQL keywords (opinionated default on).
    pub uppercase_keywords: bool,
}

/// Lower a `SOURCE_FILE` node into a document: each statement formatted, separated by a blank line,
/// and terminated with a semicolon.
///
/// A statement's own leading trivia attaches *inside* its node (so inter-statement comments ride
/// along through the verbatim path), but trivia trailing the final statement — including a
/// comment-only file — lands as direct token children of the root. Those comments are re-emitted
/// here so nothing is ever dropped.
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
        parts.push(text(token.text().to_string()));
        need_break = true;
    }

    concat(parts)
}

fn lower_stmt(stmt: &SyntaxNode, ctx: Ctx) -> Doc {
    // Comment- or error-bearing subtrees are reproduced exactly: correctness over prettiness.
    if contains_comment_or_error(stmt) {
        return verbatim(stmt);
    }
    match stmt.kind() {
        SELECT_STMT => lower_select(stmt, ctx),
        _ => inline(stmt, ctx),
    }
}

/// Lower a `SELECT_STMT`: a `SELECT <list>` header group followed by one clause per line.
fn lower_select(select: &SyntaxNode, ctx: Ctx) -> Doc {
    let mut head = vec![text("SELECT")];
    // DISTINCT / ALL quantifier appears as a direct child token before the list.
    if let Some(tok) = select
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| matches!(t.kind(), DISTINCT_KW | ALL_KW))
    {
        head.push(space());
        head.push(keyword_text(&tok, ctx));
    }

    let list = select.children().find(|n| n.kind() == SELECT_LIST);
    let list_doc = list
        .as_ref()
        .map(|list| lower_select_list(list, ctx))
        .unwrap_or_else(empty);
    // Magic trailing comma: a trailing comma the author left in the list is read as "keep this
    // exploded", forcing the header to break regardless of width. We honor the existing comma but
    // never synthesize a new one, so the token stream is preserved exactly.
    let magic_comma = list.as_ref().is_some_and(has_trailing_comma);

    let inner = concat(vec![concat(head), indent(concat(vec![line(), list_doc]))]);
    // A normal header is one group (flat when it fits, else one item per line); a magic comma
    // forces that group to break.
    let header = if magic_comma {
        group_expanded(inner)
    } else {
        group(inner)
    };

    let mut parts = vec![header];
    for clause in select.children() {
        if is_select_clause(clause.kind()) {
            parts.push(hard_line());
            parts.push(inline(&clause, ctx));
        }
    }
    concat(parts)
}

fn lower_select_list(list: &SyntaxNode, ctx: Ctx) -> Doc {
    let items: Vec<Doc> = list
        .children()
        .filter(|n| n.kind() == SELECT_ITEM)
        .map(|item| inline(&item, ctx))
        .collect();
    let mut doc = join(concat(vec![text(","), line()]), items);
    if has_trailing_comma(list) {
        // Re-emit the author's trailing comma (it is a child token of the list, not of any item).
        doc = concat(vec![doc, text(",")]);
    }
    doc
}

/// Whether `list`'s last significant child token is a comma (a tolerated trailing comma).
fn has_trailing_comma(list: &SyntaxNode) -> bool {
    list.children_with_tokens()
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
    )
}

/// Render a node, normalizing spacing and upper-casing keywords. Most constructs are emitted on a
/// single (groupable) line by walking their tokens; parenthesized comma lists (call arguments,
/// `VALUES` rows, column lists) are lowered structurally so they can wrap and honor a magic
/// trailing comma. This is the fallback for any construct without a more specific rule yet.
fn inline(node: &SyntaxNode, ctx: Ctx) -> Doc {
    Lowerer::new(ctx).lower_node(node)
}

/// A cursor that walks a subtree in document order, tracking just enough state (the previous
/// significant token, and whether it was a unary sign) to decide inter-token spacing as it goes.
struct Lowerer {
    ctx: Ctx,
    prev: Option<SyntaxKind>,
    prev_unary: bool,
}

impl Lowerer {
    fn new(ctx: Ctx) -> Self {
        Lowerer {
            ctx,
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

    fn token(&mut self, token: &snow_fmt_syntax::SyntaxToken) -> Doc {
        let sep = self.sep_before(token.kind());
        self.advance(token.kind());
        concat(vec![sep, keyword_text(token, self.ctx)])
    }

    fn lower_node(&mut self, node: &SyntaxNode) -> Doc {
        if is_paren_list(node.kind()) {
            return self.lower_paren_list(node);
        }
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
    /// are the node's child *nodes*; parentheses and commas are its tokens. Spacing across the
    /// boundaries is owned here (no space after `(`, the join provides inter-item separators), so
    /// each item is lowered from a fresh spacing state.
    fn lower_paren_list(&mut self, node: &SyntaxNode) -> Doc {
        let open_sep = self.sep_before(L_PAREN);
        let trailing = paren_list_has_trailing_comma(node);

        let mut items = Vec::new();
        for item in node.children() {
            self.prev = None;
            self.prev_unary = false;
            items.push(self.lower_node(&item));
        }
        // Whatever was inside, we resume after the closing paren.
        self.prev = Some(R_PAREN);
        self.prev_unary = false;

        if items.is_empty() {
            return concat(vec![open_sep, text("("), text(")")]);
        }

        let joined = join(concat(vec![text(","), line()]), items);
        let body = if trailing {
            // Preserve the author's trailing comma; never synthesize a new one.
            concat(vec![soft_line(), joined, text(",")])
        } else {
            concat(vec![soft_line(), joined])
        };
        let content = concat(vec![text("("), indent(body), soft_line(), text(")")]);
        let list = if trailing {
            group_expanded(content)
        } else {
            group(content)
        };
        concat(vec![open_sep, list])
    }
}

/// Parenthesized comma lists with a uniform `( items )` shape that we lower structurally.
fn is_paren_list(kind: SyntaxKind) -> bool {
    matches!(kind, ARG_LIST | VALUES_ROW | COLUMN_LIST)
}

/// Does a parenthesized list end with `, )` — a tolerated trailing comma?
fn paren_list_has_trailing_comma(node: &SyntaxNode) -> bool {
    let significant: Vec<SyntaxKind> = node
        .children_with_tokens()
        .map(|el| el.kind())
        .filter(|k| !k.is_trivia())
        .collect();
    matches!(significant.as_slice(), [.., COMMA, R_PAREN])
}

/// Token text, upper-cased if it is a keyword and keyword-casing is enabled.
fn keyword_text(token: &snow_fmt_syntax::SyntaxToken, ctx: Ctx) -> Doc {
    if ctx.uppercase_keywords && token.kind().is_keyword() {
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
            IDENT | QUOTED_IDENT | R_PAREN | R_BRACKET | CAST_KW | TRY_CAST_KW
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
    )
}

/// Reproduce a node's source text exactly (including its inner trivia/comments).
fn verbatim(node: &SyntaxNode) -> Doc {
    text(node.text().to_string())
}

/// Does the subtree contain a comment token or an `ERROR` node?
fn contains_comment_or_error(node: &SyntaxNode) -> bool {
    node.descendants_with_tokens().any(|el| {
        let k = el.kind();
        k.is_comment() || k == ERROR
    })
}
