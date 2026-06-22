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

use crate::doc::{concat, empty, group, hard_line, indent, join, line, space, text, Doc};

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

    let list_doc = select
        .children()
        .find(|n| n.kind() == SELECT_LIST)
        .map(|list| lower_select_list(&list, ctx))
        .unwrap_or_else(empty);

    // `SELECT` + list share one group so a short query stays on a single line, and a long one
    // expands to one item per indented line.
    let header = group(concat(vec![
        concat(head),
        indent(concat(vec![line(), list_doc])),
    ]));

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
    join(concat(vec![text(","), line()]), items)
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

/// Render a node from its significant tokens on a single (groupable) line, normalizing spacing and
/// upper-casing keywords. This is the fallback for any construct without a bespoke rule yet.
fn inline(node: &SyntaxNode, ctx: Ctx) -> Doc {
    let mut out = Vec::new();
    let mut prev: Option<SyntaxKind> = None;
    // Whether the previously emitted token was a unary `+`/`-`, which suppresses the next space.
    let mut prev_unary = false;

    for token in node
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
    {
        let kind = token.kind();
        if kind.is_trivia() {
            continue;
        }
        if let Some(prev_kind) = prev {
            if !prev_unary && needs_space(prev_kind, kind) {
                out.push(space());
            }
        }
        out.push(keyword_text(&token, ctx));

        prev_unary = matches!(kind, PLUS | MINUS) && prev.is_none_or(|p| !is_value_end(p));
        prev = Some(kind);
    }
    concat(out)
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
