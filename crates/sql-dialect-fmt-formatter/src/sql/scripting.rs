//! Snowflake Scripting lowering (Phase 8): blocks, branches, loops, and exception handlers —
//! keyword lines flush, statement-list bodies indented.

use sql_dialect_fmt_syntax::{SyntaxKind, SyntaxNode};
use SyntaxKind::*;

use crate::doc::{concat, empty, hard_line, indent, text, Doc};

use super::Lowerer;

impl Lowerer {
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
    pub(super) fn lower_block(&mut self, node: &SyntaxNode) -> Doc {
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
    pub(super) fn lower_declare_section(&mut self, node: &SyntaxNode) -> Doc {
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
    pub(super) fn lower_if(&mut self, node: &SyntaxNode) -> Doc {
        self.reset();
        self.lower_keyword_block(node, |kind| matches!(kind, ELSEIF_KW | ELSE_KW | END_KW))
    }

    /// `CASE [operand] WHEN … THEN … [ELSE …] END [CASE]` — arms are indented one level, with each
    /// arm body using the same statement-list layout as `IF` branches.
    pub(super) fn lower_case_stmt(&mut self, node: &SyntaxNode) -> Doc {
        let mut parts = Vec::new();
        let mut first_case = true;
        let mut pending_else = false;
        self.reset();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                match token.kind() {
                    CASE_KW if first_case => {
                        first_case = false;
                        parts.push(self.token(token));
                    }
                    ELSE_KW => {
                        self.reset();
                        parts.push(indent(concat(vec![hard_line(), self.token(token)])));
                        pending_else = true;
                    }
                    END_KW => {
                        parts.push(hard_line());
                        self.reset();
                        parts.push(self.token(token));
                        pending_else = false;
                    }
                    _ => parts.push(self.token(token)), // trailing CASE in END CASE
                }
            } else if let Some(node) = child.as_node() {
                match node.kind() {
                    CASE_STMT_WHEN => {
                        self.reset();
                        parts.push(indent(concat(vec![
                            hard_line(),
                            self.lower_case_stmt_when(node),
                        ])));
                        pending_else = false;
                    }
                    STMT_LIST if pending_else => {
                        parts.push(indent(self.lower_block_body(node)));
                        pending_else = false;
                    }
                    _ => parts.push(self.lower_node(node)), // simple CASE operand
                }
            }
        }
        concat(parts)
    }

    /// One `WHEN <test> THEN <body>` arm of a procedural CASE statement.
    fn lower_case_stmt_when(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_keyword_block(node, |_| false)
    }

    /// `FOR/WHILE … DO … END`, `LOOP … END LOOP`, `REPEAT … UNTIL … END REPEAT` — body indented.
    pub(super) fn lower_loop(&mut self, node: &SyntaxNode) -> Doc {
        self.reset();
        self.lower_keyword_block(node, |kind| matches!(kind, END_KW | UNTIL_KW))
    }

    /// `EXCEPTION` then each `WHEN … THEN <body>` handler indented.
    pub(super) fn lower_exception_section(&mut self, node: &SyntaxNode) -> Doc {
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
        self.lower_keyword_block(node, |_| false)
    }

    fn lower_keyword_block(
        &mut self,
        node: &SyntaxNode,
        break_before: impl Fn(SyntaxKind) -> bool,
    ) -> Doc {
        let mut parts = Vec::new();
        for child in node.children_with_tokens() {
            if let Some(token) = child.as_token() {
                if token.kind().is_trivia() {
                    continue;
                }
                if break_before(token.kind()) {
                    parts.push(hard_line());
                    self.reset();
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
}
