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
            parts.push(hard_line());
            parts.push(hard_line());
        }
        let lowered = lower_stmt(&stmt, ctx);
        parts.push(lowered.body);
        parts.push(text(";"));
        for comment in lowered.end_comments {
            parts.push(hard_line());
            parts.push(text(comment.text));
        }
        emitted = true;
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
fn lower_stmt(stmt: &SyntaxNode, ctx: Ctx) -> LoweredStmt {
    let mut comments = Comments::build(stmt);
    let end_comments = comments.take_statement_end_comments(stmt);
    let mut low = Lowerer::new(ctx, comments);
    // Hoist the statement's own leading comments above its first group, so a banner comment does
    // not force the first construct (e.g. the SELECT list) to explode.
    let prefix = low.statement_leading(stmt);
    let body = match stmt.kind() {
        SELECT_STMT => low.lower_select(stmt),
        _ => low.lower_node(stmt),
    };
    let body = if low.comments.all_placed() {
        concat(vec![prefix, body])
    } else {
        verbatim(stmt)
    };
    LoweredStmt { body, end_comments }
}

// ---- comment attachment ----

struct LoweredStmt {
    body: Doc,
    end_comments: Vec<CommentInfo>,
}

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

    /// Pull comments attached to the final significant token out of the statement body. The source
    /// has no semicolon token there, but the formatter synthesizes one. Emitting those comments
    /// after that synthesized semicolon, each on its own line, keeps `format(format(x)) == format(x)`
    /// for statement-end comments.
    fn take_statement_end_comments(&mut self, stmt: &SyntaxNode) -> Vec<CommentInfo> {
        let last = stmt
            .descendants_with_tokens()
            .filter_map(|el| el.into_token())
            .filter(|t| !t.kind().is_trivia() && t.kind() != COMMA)
            .last();
        last.and_then(|token| self.trailing.remove(&offset(&token)))
            .unwrap_or_default()
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
    line_comment_pending: bool,
}

impl Lowerer {
    fn new(ctx: Ctx, comments: Comments) -> Self {
        Lowerer {
            ctx,
            comments,
            prev: None,
            prev_unary: false,
            line_comment_pending: false,
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
        self.line_comment_pending = false;
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
        self.token_cased(token, false)
    }

    /// Like [`Self::token`], but `force_keyword` up-cases the token like a keyword even though it is
    /// a plain identifier in the tree — used for recognized option-key words (see
    /// [`Self::lower_option_node`]). It only ever changes ASCII case, never the spelling.
    fn token_cased(&mut self, token: &SyntaxToken, force_keyword: bool) -> Doc {
        let start = offset(token);
        let leading = self.comments.leading.remove(&start).unwrap_or_default();
        let trailing = self.comments.trailing.remove(&start).unwrap_or_default();

        let mut parts = Vec::new();
        if self.line_comment_pending {
            parts.push(hard_line());
            self.prev = None;
            self.prev_unary = false;
            self.line_comment_pending = false;
        }
        let has_leading = !leading.is_empty();
        if has_leading && self.prev.is_some() {
            parts.push(hard_line());
            self.prev = None;
            self.prev_unary = false;
        }
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
        parts.push(keyword_text_forced(token, self.ctx, force_keyword));
        for comment in trailing {
            if comment.is_line {
                // A line comment must end its line: defer it, and force the line to break.
                parts.push(line_suffix(concat(vec![space(), text(comment.text)])));
                parts.push(break_parent());
                self.line_comment_pending = true;
            } else {
                parts.push(space());
                parts.push(text(comment.text));
            }
        }
        concat(parts)
    }

    /// A keyword the formatter synthesizes (not present as its own token in the tree), cased per the
    /// options. Used where layout re-emits a keyword on its own line — e.g. the `AS` before an object
    /// DDL body. Spacing state is left untouched; callers control the surrounding lines.
    fn synth_kw(&self, word: &str) -> Doc {
        if self.ctx.uppercase_keywords {
            text(word.to_ascii_uppercase())
        } else {
            text(word.to_ascii_lowercase())
        }
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
    /// place) and a defining/CTAS query after `AS` on its own line(s). For object DDL (SCHEMA /
    /// WAREHOUSE / STAGE / FILE FORMAT / SEQUENCE / STREAM / TASK / DYNAMIC TABLE) each property
    /// (`KEY = value`), the stream source (`ON …`), and a task's `AFTER …` predecessor list each get
    /// their own indented line; a `TASK`/`DYNAMIC TABLE` body after `AS` is laid out structurally.
    fn lower_create(&mut self, node: &SyntaxNode) -> Doc {
        // Properties / clauses that stack one-per-line, indented under the CREATE header.
        let has_props = node
            .children()
            .any(|c| matches!(c.kind(), OBJECT_PROPERTY | STREAM_SOURCE | TASK_AFTER));
        if has_props {
            return self.lower_create_object(node);
        }
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

    /// Object DDL with a property region: the `CREATE <kind> <name> [(cols)]` header stays inline,
    /// then each property / stream source / `AFTER` list / body clause on its own indented line.
    fn lower_create_object(&mut self, node: &SyntaxNode) -> Doc {
        let mut head = Vec::new();
        let mut clauses = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() || token.kind() == AS_KW {
                    continue; // `AS` is re-synthesized on the body's own line below
                }
                head.push(self.token(token));
            } else if let Some(node) = child.into_node() {
                match node.kind() {
                    // The object name and any header-attached column list (`DYNAMIC TABLE dt (a, b)`)
                    // stay inline on the CREATE header.
                    NAME | NAME_REF | COLUMN_DEF_LIST => head.push(self.lower_node(&node)),
                    // Properties and sub-clauses each break onto their own indented line.
                    OBJECT_PROPERTY | STREAM_SOURCE | TASK_AFTER => {
                        self.reset();
                        clauses.push(concat(vec![hard_line(), self.lower_node(&node)]));
                    }
                    // The `AS <body>` query/statement: `AS` flush on its own line, body below.
                    _ => {
                        self.reset();
                        clauses.push(concat(vec![
                            hard_line(),
                            self.synth_kw("AS"),
                            hard_line(),
                            self.lower_query(&node),
                        ]));
                    }
                }
            }
        }
        concat(vec![concat(head), indent(concat(clauses))])
    }

    /// `GRANT <privs> ON <object> TO [ROLE] r [WITH GRANT OPTION]` /
    /// `REVOKE [GRANT OPTION FOR] <privs> ON <object> FROM [ROLE] r [CASCADE|RESTRICT]`: the keyword
    /// and privilege list on the header line, the `ON …` securable and the `TO|FROM …` grantee each
    /// on their own indented line. A trailing `WITH GRANT OPTION` / `CASCADE` / `RESTRICT` rides with
    /// the grantee. The privilege list, securable, and grantee bodies stay inline (token runs).
    fn lower_grant(&mut self, node: &SyntaxNode) -> Doc {
        let mut head = Vec::new();
        let mut clauses = Vec::new();
        let mut seen_clause = false;
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                if seen_clause {
                    // A trailing tail token (`WITH GRANT OPTION`, `CASCADE`, …) rides after the
                    // grantee on its line.
                    clauses.push(self.token(token));
                } else {
                    head.push(self.token(token));
                }
            } else if let Some(node) = child.into_node() {
                match node.kind() {
                    PRIV_LIST => head.push(self.lower_node(&node)),
                    GRANT_TARGET | GRANTEE => {
                        seen_clause = true;
                        self.reset();
                        clauses.push(concat(vec![hard_line(), self.lower_node(&node)]));
                    }
                    _ => head.push(self.lower_node(&node)),
                }
            }
        }
        concat(vec![concat(head), indent(concat(clauses))])
    }

    // ---- Snowflake Scripting (Phase 8) ----

    /// A `STMT_LIST` (a block / branch / loop / handler body): each statement on its own line with a
    /// synthesized terminating `;`, the whole indented one level. Returns the indented body only —
    /// the caller emits the surrounding keyword lines (`BEGIN`/`END`, `THEN`, `DO`, …).
    fn lower_block_body(&mut self, list: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for stmt in list.children() {
            parts.push(hard_line());
            self.reset();
            parts.push(self.lower_node(&stmt));
            parts.push(text(";"));
        }
        indent(concat(parts))
    }

    /// `[DECLARE …] BEGIN <body> [EXCEPTION …] END [label]` — keyword lines flush, bodies indented.
    fn lower_block(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        let mut started = false;
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                match token.kind() {
                    BEGIN_KW => {
                        if started {
                            parts.push(hard_line());
                        }
                        self.reset();
                        parts.push(self.token(token));
                        started = true;
                    }
                    END_KW => {
                        parts.push(hard_line());
                        self.reset();
                        parts.push(self.token(token));
                    }
                    _ => parts.push(self.token(token)),
                }
            } else if let Some(node) = child.as_node() {
                match node.kind() {
                    DECLARE_SECTION => {
                        parts.push(self.lower_declare_section(node));
                        started = true;
                    }
                    STMT_LIST => parts.push(self.lower_block_body(node)),
                    EXCEPTION_SECTION => {
                        parts.push(hard_line());
                        self.reset();
                        parts.push(self.lower_exception_section(node));
                    }
                    _ => parts.push(self.lower_node(node)), // END label
                }
            }
        }
        concat(parts)
    }

    /// `DECLARE` then each declaration on its own indented line with a synthesized `;`.
    fn lower_declare_section(&mut self, node: &SyntaxNode) -> Doc {
        let mut head = empty();
        let mut body = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                if token.kind() == DECLARE_KW {
                    self.reset();
                    head = self.token(token);
                }
            } else if let Some(node) = child.as_node() {
                body.push(hard_line());
                self.reset();
                body.push(self.lower_node(node));
                body.push(text(";"));
            }
        }
        concat(vec![head, indent(concat(body))])
    }

    /// `IF <cond> THEN … [ELSEIF … THEN …] [ELSE …] END IF` — branch keywords flush, bodies indented.
    fn lower_if(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        self.reset();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                match token.kind() {
                    ELSEIF_KW | ELSE_KW | END_KW => {
                        parts.push(hard_line());
                        self.reset();
                        parts.push(self.token(token));
                    }
                    _ => parts.push(self.token(token)), // IF / THEN / the IF after END
                }
            } else if let Some(node) = child.as_node() {
                if node.kind() == STMT_LIST {
                    parts.push(self.lower_block_body(node));
                } else {
                    parts.push(self.lower_node(node)); // condition
                }
            }
        }
        concat(parts)
    }

    /// `FOR/WHILE … DO … END`, `LOOP … END LOOP`, `REPEAT … UNTIL … END REPEAT` — body indented.
    fn lower_loop(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        self.reset();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                match token.kind() {
                    END_KW | UNTIL_KW => {
                        parts.push(hard_line());
                        self.reset();
                        parts.push(self.token(token));
                    }
                    _ => parts.push(self.token(token)),
                }
            } else if let Some(node) = child.as_node() {
                if node.kind() == STMT_LIST {
                    parts.push(self.lower_block_body(node));
                } else {
                    parts.push(self.lower_node(node)); // UNTIL condition
                }
            }
        }
        concat(parts)
    }

    /// `EXCEPTION` then each `WHEN … THEN <body>` handler indented.
    fn lower_exception_section(&mut self, node: &SyntaxNode) -> Doc {
        let mut head = empty();
        let mut body = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                if token.kind() == EXCEPTION_KW {
                    self.reset();
                    head = self.token(token);
                }
            } else if let Some(node) = child.as_node() {
                body.push(hard_line());
                self.reset();
                body.push(self.lower_exception_when(node));
            }
        }
        concat(vec![head, indent(concat(body))])
    }

    /// `WHEN <exc> THEN <body>` — the `WHEN … THEN` line then the handler body indented.
    fn lower_exception_when(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                parts.push(self.token(token));
            } else if let Some(node) = child.as_node() {
                if node.kind() == STMT_LIST {
                    parts.push(self.lower_block_body(node));
                } else {
                    parts.push(self.lower_node(node));
                }
            }
        }
        concat(parts)
    }

    /// A `@stage/path` reference used as a table/source. Its `/` and `.` connectors would be
    /// re-spaced by the generic token walker, so the run is emitted verbatim, with a normal leading
    /// separator (e.g. the space after `FROM`) and spacing resumed as a value for a trailing alias.
    fn lower_stage_ref(&mut self, node: &SyntaxNode) -> Doc {
        let sep = self.sep_before(AT);
        self.resume_after(IDENT);
        concat(vec![sep, trimmed_text(node)])
    }

    /// A COPY target/source location, emitted verbatim (preserving `@stage/path`, whose `/` operator
    /// spacing would mangle) with the leading-trivia space trimmed for idempotency.
    fn lower_copy_location(&mut self, node: &SyntaxNode) -> Doc {
        let doc = concat(vec![space(), trimmed_text(node)]);
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
                parts.push(concat(vec![space(), trimmed_text(node)]));
                self.resume_after(R_PAREN);
            }
        }
        concat(parts)
    }

    /// A flow-operator pipeline `<stmt> ->> <stmt> ->> ...`: each statement formatted normally, the
    /// `->>` operator leading each continuation line. No semicolons are inserted between steps.
    fn lower_flow(&mut self, node: &SyntaxNode) -> Doc {
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
            GRANT_STMT | REVOKE_STMT => self.lower_grant(node),
            COPY_STMT => self.lower_copy(node),
            COPY_OPTION | OBJECT_PROPERTY => self.lower_option_node(node),
            COPY_LOCATION => self.lower_copy_location(node),
            STAGE_REF => self.lower_stage_ref(node),
            COLUMN_DEF_LIST => concat(vec![space(), self.lower_column_def_list(node)]),
            MATCH_RECOGNIZE => self.lower_match_recognize(node),
            PATTERN_CLAUSE => self.lower_pattern_clause(node),
            FLOW_STMT => self.lower_flow(node),
            // Snowflake Scripting (Phase 8).
            BLOCK_STMT => self.lower_block(node),
            IF_STMT => self.lower_if(node),
            LOOP_STMT => self.lower_loop(node),
            DECLARE_SECTION => self.lower_declare_section(node),
            EXCEPTION_SECTION => self.lower_exception_section(node),
            // `SET col = ...` and `VALUES (...), (...)` are keyword + comma-list clauses.
            SET_CLAUSE | VALUES_CLAUSE => self.lower_keyword_item_list(node),
            _ => self.lower_children(node),
        }
    }

    /// Lower a COPY `COPY_OPTION` or object-DDL `OBJECT_PROPERTY` node, up-casing the recognized
    /// option-**key** word(s) so they match the surrounding reserved keywords when keyword-casing is
    /// on.
    ///
    /// ## Casing policy (option keys)
    /// The parser captures an option key (`FILE_FORMAT`, `ON_ERROR`, `WAREHOUSE_SIZE`, `TARGET_LAG`,
    /// the nested `TYPE`/`SKIP_HEADER`/… inside `FILE_FORMAT = ( … )`, …) as a plain `IDENT`, because
    /// these words are *not* reserved and double as ordinary identifiers — so by default they were
    /// emitted verbatim, producing mixed-case output next to up-cased reserved keywords. Here we
    /// up-case a token **only** when it is a key in **key position**:
    ///   * it is an `IDENT`/`CONTEXTUAL_KEYWORD` whose lower-cased text is a known canonical option
    ///     key (`is_option_key`), and
    ///   * it is not a *value* (its preceding significant sibling is not `=`), and
    ///   * it sits in key position — immediately followed by `=`, or by the `WITH`/`BY` connector of
    ///     the `=`-less `START WITH n` / `INCREMENT BY n` sequence forms, or it is a no-value flag
    ///     word (`is_option_flag`, e.g. `ORDER` / `NOORDER`).
    ///
    /// Option **values**, user identifiers, string/numeric literals, and `@stage` names are never
    /// touched — only the ASCII case of a recognized key word changes — so the round-trip and
    /// token-preservation guarantees (which case-fold) still hold.
    fn lower_option_node(&mut self, node: &SyntaxNode) -> Doc {
        // The node's significant tokens, in order, with their original kinds — used for the
        // key-position lookahead/lookbehind below. (Child *nodes*, e.g. a `PARTITION BY (expr)`
        // body, never contain option keys, so they are lowered normally.)
        let sig: Vec<SyntaxKind> = node
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .filter(|t| !t.kind().is_trivia())
            .map(|t| t.kind())
            .collect();

        let mut parts = Vec::new();
        let mut sig_idx: usize = 0;
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                let prev = sig_idx.checked_sub(1).map(|i| sig[i]);
                let next = sig.get(sig_idx + 1).copied();
                let force = self.is_option_key_position(token, prev, next);
                parts.push(self.token_cased(token, force));
                sig_idx += 1;
            } else if let Some(node) = child.as_node() {
                parts.push(self.lower_node(node));
            }
        }
        concat(parts)
    }

    /// Whether `token` is a recognized option key sitting in key position (see the policy on
    /// [`Self::lower_option_node`]); `prev`/`next` are its neighbouring significant token kinds.
    fn is_option_key_position(
        &self,
        token: &SyntaxToken,
        prev: Option<SyntaxKind>,
        next: Option<SyntaxKind>,
    ) -> bool {
        // Only identifier-like words are ever keys; a literal/`@stage`/operator never is.
        if !matches!(token.kind(), IDENT | CONTEXTUAL_KEYWORD) {
            return false;
        }
        // A token right after `=` is a value, never a key (`ON_ERROR = SKIP_FILE`).
        if prev == Some(EQ) {
            return false;
        }
        let word = token.text();
        // A bare flag (`NOORDER`) is itself a recognized key needing no `= value`.
        if is_option_flag(word) {
            return true;
        }
        // Otherwise a recognized key in `KEY = …` (any nesting) or the `=`-less `START WITH n` /
        // `INCREMENT BY n` sequence forms (key word immediately followed by the `WITH`/`BY` word).
        matches!(next, Some(EQ | WITH_KW | BY_KW)) && is_option_key(word)
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
///
/// `force_keyword` treats the token as a keyword for casing even though it is a plain identifier in
/// the tree — used for recognized COPY/object-DDL option-key words (see [`Lowerer::lower_option_node`]
/// and [`is_option_key`]). It only ever toggles ASCII case, never re-spells the word.
fn keyword_text_forced(token: &SyntaxToken, ctx: Ctx, force_keyword: bool) -> Doc {
    // Soft (contextual) keywords are tagged `CONTEXTUAL_KEYWORD` rather than living in the keyword
    // range, but they upper-case just like real keywords.
    let is_keyword = force_keyword || token.kind().is_keyword() || token.kind() == CONTEXTUAL_KEYWORD;
    if ctx.uppercase_keywords && is_keyword {
        text(token.text().to_ascii_uppercase())
    } else {
        text(token.text().to_string())
    }
}

/// Whether `word` (case-insensitively) is a canonical Snowflake COPY / object-DDL option **key** —
/// a word the formatter up-cases when it appears in key position (see [`Lowerer::lower_option_node`]).
///
/// The list is the union of the documented `copyOptions`, `formatTypeOptions`, `CREATE … <object>`
/// property keys, and the nested file-format / stage option keys
/// (docs.snowflake.com `copy-into-table`, `copy-into-location`, `create-file-format`,
/// `create-stage`, `create-warehouse`, `create-dynamic-table`, `create-task`, `create-sequence`,
/// `create-stream`, `create-schema`). It is matched case-insensitively against the lower-case
/// canonical spelling. Adding a word here only changes the *casing* of a recognized key in key
/// position — never a value, identifier, or literal.
fn is_option_key(word: &str) -> bool {
    OPTION_KEYS.binary_search(&word.to_ascii_lowercase().as_str()).is_ok()
}

/// A no-value option flag word (`ORDER` / `NOORDER`): a recognized key that stands alone with no
/// `= value`. Used so a trailing flag still up-cases even though no `=` follows it.
fn is_option_flag(word: &str) -> bool {
    OPTION_FLAGS.binary_search(&word.to_ascii_lowercase().as_str()).is_ok()
}

/// Canonical option-key spellings, lower-cased and **kept sorted** for `binary_search`. (Multi-word
/// keys such as `PARTITION BY`, `START WITH`, `CLUSTER BY`, `COPY GRANTS` are recognized via their
/// already-reserved leading keyword and the `WITH`/`BY` connector, so only single identifier words
/// live here.) Verified against docs.snowflake.com.
const OPTION_KEYS: &[&str] = &[
    "allow_duplicate",
    "append_only",
    "auto_refresh",
    "auto_resume",
    "auto_suspend",
    "base_location",
    "binary_as_text",
    "binary_format",
    "catalog",
    "comment",
    "compression",
    "config",
    "credentials",
    "data_retention_time_in_days",
    "date_format",
    "default_ddl_collation",
    "detailed_output",
    "directory",
    "disable_auto_convert",
    "empty_field_as_null",
    "enable",
    "enable_octal",
    "enable_query_acceleration",
    "enable_schema_evolution",
    "encoding",
    "encryption",
    "enforce_length",
    "error_integration",
    "error_on_column_count_mismatch",
    "escape",
    "escape_unenclosed_field",
    "execute_as",
    "external_volume",
    "field_delimiter",
    "field_optionally_enclosed_by",
    "file_extension",
    "file_format",
    "files",
    "finalize",
    "force",
    "format_name",
    "ignore_utf8_errors",
    "include_metadata",
    "include_query_id",
    "increment",
    "initialize",
    "initially_suspended",
    "insert_only",
    "log_level",
    "master_key",
    "match_by_column_name",
    "max_cluster_count",
    "max_concurrency_level",
    "max_data_extension_time_in_days",
    "max_file_size",
    "min_cluster_count",
    "multi_line",
    "null_if",
    "on_error",
    "overlap_policy",
    "overwrite",
    "parse_header",
    "pattern",
    "preserve_space",
    "purge",
    "query_acceleration_max_scale_factor",
    "record_delimiter",
    "refresh_mode",
    "refresh_on_create",
    "replace_invalid_characters",
    "resource_constraint",
    "resource_monitor",
    "return_failed_only",
    "runtime_version",
    "scaling_policy",
    "schedule",
    "show_initial_rows",
    "single",
    "size_limit",
    "skip_blank_lines",
    "skip_byte_order_mark",
    "skip_header",
    "snappy_compression",
    "start",
    "statement_queued_timeout_in_seconds",
    "statement_timeout_in_seconds",
    "storage_integration",
    "strip_null_values",
    "strip_outer_array",
    "strip_outer_element",
    "suspend_task_after_num_failures",
    "target_completion_interval",
    "target_lag",
    "time_format",
    "timestamp_format",
    "trim_space",
    "truncatecolumns",
    "type",
    "url",
    "use_logical_type",
    "use_vectorized_scanner",
    "user_task_managed_initial_warehouse_size",
    "user_task_timeout_ms",
    "validation_mode",
    "warehouse",
    "warehouse_size",
    "warehouse_type",
];

/// No-value option flag words, lower-cased and **kept sorted** (see [`is_option_flag`]).
const OPTION_FLAGS: &[&str] = &["noorder", "order"];

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

/// A node's source text with surrounding whitespace removed, materialized in a single allocation.
/// `node.text()` is a rope view, so it must be collected into a `String`; trimming in place then
/// avoids the second `String` the `…to_string().trim().to_string()` idiom would allocate.
fn trimmed_text(node: &SyntaxNode) -> Doc {
    let mut s = node.text().to_string();
    s.truncate(s.trim_end().len());
    let start = s.len() - s.trim_start().len();
    if start > 0 {
        s.drain(..start);
    }
    text(s)
}
