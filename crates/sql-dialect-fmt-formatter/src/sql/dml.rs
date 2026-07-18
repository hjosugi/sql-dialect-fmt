//! DML and data-movement statement lowering: the shared clausal walker plus `INSERT`, `UPDATE`,
//! `DELETE`, `MERGE`, `COPY INTO`, Databricks maintenance statements, and stage locations.

use sql_dialect_fmt_syntax::{SyntaxKind, SyntaxNode};
use SyntaxKind::*;

use crate::doc::{concat, hard_line, space, Doc};

use super::{trimmed_text, Lowerer};

impl Lowerer {
    /// The shared shape of every multi-clause statement: walk children in order, emit the header
    /// inline, and put each clause on its own line. `break_token` marks tokens that *introduce* a
    /// clause (e.g. `FROM`/`USING`/`ON`/`ELSE`) and start a new line; `block_node` marks clause
    /// *nodes* that get their own line (rendered via [`Self::lower_query`], so a bare `SELECT`
    /// source is structured). Everything else is part of the header. Node kinds with bespoke inline
    /// rendering (a verbatim stage location, a spaced column-def list) own that in [`Self::lower_node`].
    pub(super) fn lower_clausal(
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
    pub(super) fn lower_insert(&mut self, node: &SyntaxNode) -> Doc {
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
    pub(super) fn lower_update(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(
            node,
            |_| false,
            |k| matches!(k, SET_CLAUSE | FROM_CLAUSE | WHERE_CLAUSE),
        )
    }

    /// `DELETE FROM t [USING ...]` then `WHERE ...` on its own line.
    pub(super) fn lower_delete(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(node, |_| false, |k| k == WHERE_CLAUSE)
    }

    /// `COPY INTO <target> FROM <source>` with `FROM` and each option on their own line.
    pub(super) fn lower_copy(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(node, |k| k == FROM_KW, |k| k == COPY_OPTION)
    }

    /// `MERGE INTO t USING s ON cond` with `USING`, `ON`, and each `WHEN` clause on their own lines.
    pub(super) fn lower_merge(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(node, |k| matches!(k, USING_KW | ON_KW), |k| k == MERGE_WHEN)
    }

    /// Databricks `OPTIMIZE <t> [WHERE p] [ZORDER BY (cols)]`: the `OPTIMIZE <t>` header inline, with
    /// the `WHERE` predicate and the `ZORDER BY` clause each on their own line.
    pub(super) fn lower_optimize(&mut self, node: &SyntaxNode) -> Doc {
        self.lower_clausal(
            node,
            |_| false,
            |k| matches!(k, WHERE_CLAUSE | ZORDER_CLAUSE),
        )
    }

    /// Databricks `CACHE [LAZY] TABLE <t> [OPTIONS (...)]` header inline, with a defining `[AS]` query
    /// laid out on its own indented line(s).
    pub(super) fn lower_cache(&mut self, node: &SyntaxNode) -> Doc {
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

    /// A `@stage/path` reference used as a table/source. Its `/` and `.` connectors would be
    /// re-spaced by the generic token walker, so the run is emitted verbatim, with a normal leading
    /// separator (e.g. the space after `FROM`) and spacing resumed as a value for a trailing alias.
    pub(super) fn lower_stage_ref(&mut self, node: &SyntaxNode) -> Doc {
        let sep = self.sep_before(AT);
        self.resume_after(IDENT);
        concat(vec![sep, trimmed_text(node)])
    }

    /// A COPY target/source location, emitted verbatim (preserving `@stage/path`, whose `/` operator
    /// spacing would mangle) with the leading-trivia space trimmed for idempotency.
    pub(super) fn lower_copy_location(&mut self, node: &SyntaxNode) -> Doc {
        let doc = concat(vec![space(), trimmed_text(node)]);
        self.resume_after(IDENT);
        doc
    }
}
