//! DDL statement lowering: `CREATE` headers and bodies (objects, routines), `GRANT`/`REVOKE`,
//! `EXECUTE IMMEDIATE`, and COPY/object-DDL option-key casing.

use sql_dialect_fmt_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use SyntaxKind::*;

use crate::doc::{concat, empty, hard_line, indent, join, space, text, Doc};

use super::expr::{bracketed, paren_list_has_trailing_comma};
use super::options::{is_option_flag, is_option_key};
use super::routine_body::{
    format_embedded_body_token, is_create_routine, is_routine_header_word, routine_body_language,
    RoutineBodyLanguage,
};
use super::Lowerer;

impl Lowerer {
    /// `CREATE [OR REPLACE] ... TABLE/VIEW ...`: the header inline (a column-def list expanded in
    /// place) and a defining/CTAS query after `AS` on its own line(s). For object DDL (SCHEMA /
    /// WAREHOUSE / STAGE / FILE FORMAT / SEQUENCE / STREAM / TASK / DYNAMIC TABLE) each property
    /// (`KEY = value`), the stream source (`ON …`), and a task's `AFTER …` predecessor list each get
    /// their own indented line; a `TASK`/`DYNAMIC TABLE` body after `AS` is laid out structurally.
    pub(super) fn lower_create(&mut self, node: &SyntaxNode) -> Doc {
        if is_create_routine(node) {
            return self.lower_create_routine(node);
        }
        // Properties / clauses that stack one-per-line, indented under the CREATE header.
        let has_props = node.children().any(|c| {
            matches!(
                c.kind(),
                OBJECT_PROPERTY | STREAM_SOURCE | TASK_AFTER | SEMANTIC_VIEW_CLAUSE
            )
        });
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

    /// `CREATE PROCEDURE/FUNCTION ... AS <body>`: keep the signature inline, format supported
    /// dollar-quoted bodies through their language formatter, and lower unquoted Snowflake
    /// Scripting blocks structurally. Unknown languages stay verbatim.
    fn lower_create_routine(&mut self, node: &SyntaxNode) -> Doc {
        let body_language = routine_body_language(node).unwrap_or(RoutineBodyLanguage::Sql);
        let mut parts = Vec::new();
        let mut prev_sig = None;
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                if matches!(token.kind(), DOLLAR_STRING | STRING) && prev_sig == Some(AS_KW) {
                    if let Some(formatted) =
                        format_embedded_body_token(token.text(), body_language, self.ctx)
                    {
                        parts.push(self.token_rendered(token, text(formatted)));
                        prev_sig = Some(token.kind());
                        continue;
                    }
                }
                if token.kind() == L_PAREN && prev_sig == Some(TABLE_KW) {
                    parts.push(space());
                }
                parts.push(self.token_cased(token, is_routine_header_word(token)));
                prev_sig = Some(token.kind());
            } else if let Some(node) = child.as_node() {
                if node.kind() == BLOCK_STMT {
                    parts.push(hard_line());
                    self.reset();
                }
                parts.push(self.lower_node(node));
                prev_sig = None;
            }
        }
        concat(parts)
    }

    /// A structured `RETURNS <type>` clause stays inline. Snowflake's table-return form is spelled
    /// `RETURNS TABLE (...)`; preserve the separating space while keeping the open-ended return
    /// column definitions lossless rather than applying parameter-list expansion rules.
    pub(super) fn lower_routine_returns_clause(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        let mut prev_sig = None;
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                if token.kind() == L_PAREN && prev_sig == Some(TABLE_KW) {
                    parts.push(space());
                }
                parts.push(self.token(token));
                prev_sig = Some(token.kind());
            } else if let Some(node) = child.as_node() {
                parts.push(self.lower_node(node));
                prev_sig = None;
            }
        }
        concat(parts)
    }

    /// `EXECUTE IMMEDIATE $$ ... $$`: the statement header stays inline, and a dollar-quoted body
    /// immediately after `IMMEDIATE` is formatted as embedded SQL when it parses cleanly.
    pub(super) fn lower_execute(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        let mut prev_sig = None;
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                parts.push(self.token(token));
                prev_sig = Some(token.kind());
            } else if let Some(node) = child.as_node() {
                if prev_sig == Some(IMMEDIATE_KW) {
                    if let Some(formatted) = self.lower_execute_immediate_body(node) {
                        parts.push(formatted);
                        prev_sig = None;
                        continue;
                    }
                }
                parts.push(self.lower_node(node));
                prev_sig = None;
            }
        }
        concat(parts)
    }

    fn lower_execute_immediate_body(&mut self, node: &SyntaxNode) -> Option<Doc> {
        if node.kind() != LITERAL {
            return None;
        }
        let token = node
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| !token.kind().is_trivia())?;
        if token.kind() != DOLLAR_STRING {
            return None;
        }
        format_embedded_body_token(token.text(), RoutineBodyLanguage::Sql, self.ctx)
            .map(|formatted| self.token_rendered(&token, text(formatted)))
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
                    OBJECT_PROPERTY | STREAM_SOURCE | TASK_AFTER | SEMANTIC_VIEW_CLAUSE => {
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
    pub(super) fn lower_grant(&mut self, node: &SyntaxNode) -> Doc {
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

    /// `( col type ..., col type ... )` — one column definition per line when it does not fit.
    pub(super) fn lower_column_def_list(&mut self, node: &SyntaxNode) -> Doc {
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
    pub(super) fn lower_option_node(&mut self, node: &SyntaxNode) -> Doc {
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

    /// A `CREATE SEMANTIC VIEW` top-level clause. The parser gives us the outer clause and the
    /// top-level comma items; each item body remains a lossless token run because the semantic-view
    /// grammar embeds table references, metric expressions, and verified-query metadata.
    pub(super) fn lower_semantic_view_clause(&mut self, node: &SyntaxNode) -> Doc {
        let mut head = Vec::new();
        let mut items = Vec::new();
        let mut saw_parens = false;

        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() || token.kind() == COMMA {
                    continue;
                }
                match token.kind() {
                    L_PAREN => saw_parens = true,
                    // The closing delimiter is re-synthesized after the lowered item list below.
                    R_PAREN => {}
                    _ => head.push(self.token(token)),
                }
            } else if let Some(node) = child.as_node() {
                if node.kind() == SEMANTIC_VIEW_ITEM {
                    self.reset();
                    items.push(self.lower_node(node));
                } else {
                    head.push(self.lower_node(node));
                }
            }
        }

        if !saw_parens {
            return concat(head);
        }

        self.resume_after(R_PAREN);
        let body = if items.is_empty() {
            text("()")
        } else {
            concat(vec![
                text("("),
                indent(concat(vec![
                    hard_line(),
                    join(concat(vec![text(","), hard_line()]), items),
                ])),
                hard_line(),
                text(")"),
            ])
        };
        concat(vec![concat(head), space(), body])
    }
}
