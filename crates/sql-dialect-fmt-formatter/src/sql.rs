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

use sql_dialect_fmt_syntax::{Dialect, SyntaxKind, SyntaxNode, SyntaxToken};
use SyntaxKind::*;

use crate::doc::{break_parent, concat, empty, hard_line, line_suffix, space, text, Doc};
use crate::KeywordCase;

mod comments;
mod ddl;
mod dml;
mod expr;
mod lenient;
mod options;
mod query;
mod routine_body;
mod scripting;
mod spacing;

use comments::{directive_comment_same_line_after_stmt, CommentInfo, Comments};
use spacing::{is_value_end, must_separate_to_preserve_tokens, needs_space};

/// Formatting context.
#[derive(Clone, Copy)]
pub(crate) struct Ctx {
    /// Keyword casing policy.
    pub keyword_case: KeywordCase,
    pub line_width: usize,
    pub indent_width: usize,
    /// The SQL dialect being formatted. Used to parse the source with matching grammar/lexing
    /// rules; dialect-specific lowering will gate on this in later phases.
    pub dialect: Dialect,
}

/// Lower a `SOURCE_FILE` node into a document: each statement formatted, separated according to
/// the author's grouping (at most one blank line), and terminated with a semicolon.
///
/// A statement's own leading/interior comments attach *inside* its node and are placed by the
/// statement lowering. Trivia trailing the final statement — including a comment-only file — lands
/// as direct token children of the root; those comments are re-emitted here so nothing is dropped.
pub(crate) fn lower_source(root: &SyntaxNode, ctx: Ctx) -> Doc {
    let mut parts = Vec::new();
    let mut emitted = false;
    let mut last_stmt_end: Option<usize> = None;
    for stmt in root.children() {
        if emitted {
            parts.push(hard_line());
            if statement_has_leading_blank_line(&stmt) {
                parts.push(hard_line());
            }
        }
        let lowered = lower_stmt(&stmt, ctx);
        parts.push(lowered.body);
        parts.push(text(";"));
        for comment in lowered.end_comments {
            if comment.is_line && comment.is_directive {
                parts.push(space());
                parts.push(text(comment.text));
            } else {
                parts.push(hard_line());
                parts.push(text(comment.text));
            }
        }
        emitted = true;
        last_stmt_end = Some(stmt.text_range().end().into());
    }

    // Root-level (trailing) comments, kept verbatim each on its own line.
    let mut need_break = emitted;
    for token in root
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind().is_comment())
    {
        if need_break {
            if directive_comment_same_line_after_stmt(last_stmt_end, &token) {
                parts.push(space());
            } else {
                parts.push(hard_line());
            }
        }
        parts.push(text(token.text().trim_end().to_string()));
        need_break = true;
    }

    concat(parts)
}

fn statement_has_leading_blank_line(stmt: &SyntaxNode) -> bool {
    let mut saw_newline = false;
    let mut line_has_content = false;
    for token in stmt
        .children_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            WHITESPACE => {}
            NEWLINE => {
                if saw_newline && !line_has_content {
                    return true;
                }
                saw_newline = true;
                line_has_content = false;
            }
            kind if kind.is_comment() => {
                line_has_content = true;
            }
            _ => break,
        }
    }
    false
}

/// Lower one statement. Builds its comment attachment, lowers structurally, and — if any comment
/// could not be placed onto an emitted token — falls back to the statement text with surrounding
/// whitespace trimmed so statement separators remain idempotent.
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
        trimmed_text(stmt)
    };
    LoweredStmt { body, end_comments }
}

struct LoweredStmt {
    body: Doc,
    end_comments: Vec<CommentInfo>,
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
            Some(prev) if must_separate_to_preserve_tokens(prev, cur) => space(),
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

    fn take_line_comment_break(&mut self) -> Doc {
        if !self.line_comment_pending {
            return empty();
        }
        self.prev = None;
        self.prev_unary = false;
        self.line_comment_pending = false;
        hard_line()
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
        for comment in self.comments.take_leading(&token) {
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
        self.token_rendered(token, keyword_text_forced(token, self.ctx, force_keyword))
    }

    /// Emit a token's spacing/comments while replacing its rendered text. Used for embedded SQL
    /// routine bodies: the CST token is still a single `DOLLAR_STRING`, but its body may be
    /// reformatted if it is declared as SQL.
    fn token_rendered(&mut self, token: &SyntaxToken, rendered: Doc) -> Doc {
        let leading = self.comments.take_leading(token);
        let trailing = self.comments.take_trailing(token);

        let mut parts = Vec::new();
        parts.push(self.take_line_comment_break());
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
        parts.push(rendered);
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
        match self.ctx.keyword_case {
            KeywordCase::Upper => text(word.to_ascii_uppercase()),
            KeywordCase::Lower => text(word.to_ascii_lowercase()),
            KeywordCase::Preserve => text(word.to_ascii_lowercase()),
        }
    }

    /// Render a node, normalizing spacing and upper-casing keywords. Most constructs are emitted on
    /// a single (groupable) line by walking their tokens; parenthesized comma lists and `IN (...)`
    /// are lowered structurally so they can wrap and honor a magic trailing comma.
    fn lower_node(&mut self, node: &SyntaxNode) -> Doc {
        let pending_break = self.take_line_comment_break();
        let body = match node.kind() {
            // Parenthesized comma lists, lowered structurally (wrap + magic trailing comma).
            ARG_LIST | VALUES_ROW | COLUMN_LIST | LAMBDA_PARAMS => self.lower_paren_list(node),
            ARRAY_LITERAL => self.lower_delimited_list(node, L_BRACKET, R_BRACKET, "[", "]"),
            OBJECT_LITERAL => self.lower_delimited_list(node, L_BRACE, R_BRACE, "{", "}"),
            OBJECT_FIELD => self.lower_object_field(node),
            BIND_MARKER => self.lower_bind_marker(node),
            INTERVAL_LITERAL => self.lower_value_children(node),
            BIN_EXPR => self.lower_logical_expr(node),
            IN_EXPR => self.lower_in_expr(node),
            CASE_EXPR => self.lower_case(node),
            WINDOW_SPEC => self.lower_window_spec(node),
            PARTITION_BY_CLAUSE => self.lower_keyword_item_list(node),
            SUBQUERY => self.lower_subquery(node),
            WITH_QUERY => self.lower_with_query(node),
            SET_OP => self.lower_set_op(node),
            INSERT_STMT => self.lower_insert(node),
            UPDATE_STMT => self.lower_update(node),
            DELETE_STMT => self.lower_delete(node),
            MERGE_STMT => self.lower_merge(node),
            // Databricks / Delta maintenance + cache statements.
            OPTIMIZE_STMT => self.lower_optimize(node),
            CACHE_STMT => self.lower_cache(node),
            // `VACUUM`, `UNCACHE`, `REFRESH`, and `DESCRIBE HISTORY` are single-line statements.
            VACUUM_STMT
            | UNCACHE_STMT
            | REFRESH_STMT
            | DESCRIBE_HISTORY_STMT
            | RESTORE_STMT
            | ANALYZE_STMT
            | MSCK_REPAIR_STMT => self.lower_children(node),
            CREATE_STMT => self.lower_create(node),
            EXECUTE_STMT => self.lower_execute(node),
            ALTER_STMT | USE_STMT | SHOW_STMT | DESCRIBE_STMT | TRUNCATE_STMT
            | TRANSACTION_STMT | UNDROP_STMT | COMMENT_STMT => self.lower_lenient_stmt(node),
            GRANT_STMT | REVOKE_STMT => self.lower_grant(node),
            COPY_STMT => self.lower_copy(node),
            STAGE_FILE_STMT => self.lower_children(node),
            COPY_OPTION | OBJECT_PROPERTY => self.lower_option_node(node),
            SEMANTIC_VIEW_CLAUSE => self.lower_semantic_view_clause(node),
            COPY_LOCATION => self.lower_copy_location(node),
            STAGE_REF => self.lower_stage_ref(node),
            TIME_TRAVEL => self.lower_time_travel(node),
            SAMPLE_CLAUSE => self.lower_sample_clause(node),
            COLUMN_DEF_LIST => concat(vec![space(), self.lower_column_def_list(node)]),
            ROUTINE_RETURNS_CLAUSE => self.lower_routine_returns_clause(node),
            MATCH_RECOGNIZE => self.lower_match_recognize(node),
            PATTERN_CLAUSE => self.lower_pattern_clause(node),
            FLOW_STMT => self.lower_flow(node),
            // Snowflake Scripting (Phase 8).
            BLOCK_STMT => self.lower_block(node),
            IF_STMT => self.lower_if(node),
            CASE_STMT => self.lower_case_stmt(node),
            LOOP_STMT => self.lower_loop(node),
            DECLARE_SECTION => self.lower_declare_section(node),
            EXCEPTION_SECTION => self.lower_exception_section(node),
            DECLARE_ITEM | LET_STMT | ASSIGN_STMT | RETURN_STMT | SCRIPT_STMT => {
                self.lower_lenient_stmt(node)
            }
            // `SET col = ...` and `VALUES (...), (...)` are keyword + comma-list clauses.
            SET_CLAUSE | VALUES_CLAUSE => self.lower_keyword_item_list(node),
            _ => self.lower_children(node),
        };
        concat(vec![pending_break, body])
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
}

/// Token text, upper-cased if it is a keyword and keyword-casing is enabled.
///
/// `force_keyword` treats the token as a keyword for casing even though it is a plain identifier in
/// the tree — used for recognized COPY/object-DDL option-key words (see [`Lowerer::lower_option_node`]
/// and [`options::is_option_key`]). It only ever toggles ASCII case, never re-spells the word.
fn keyword_text_forced(token: &SyntaxToken, ctx: Ctx, force_keyword: bool) -> Doc {
    // Soft (contextual) keywords are tagged `CONTEXTUAL_KEYWORD` rather than living in the keyword
    // range, but they upper-case just like real keywords.
    let is_keyword =
        force_keyword || token.kind().is_keyword() || token.kind() == CONTEXTUAL_KEYWORD;
    if is_keyword {
        match ctx.keyword_case {
            KeywordCase::Upper => text(token.text().to_ascii_uppercase()),
            KeywordCase::Lower => text(token.text().to_ascii_lowercase()),
            KeywordCase::Preserve => text(token.text().to_string()),
        }
    } else {
        text(token.text().to_string())
    }
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
