//! A thin typed layer over the untyped CST: zero-cost newtypes around `SyntaxNode` with
//! `cast`/`syntax` plus a few accessors. This bootstraps the pattern; coverage grows with the
//! grammar (and can later be code-generated from an ungrammar).

use sql_dialect_fmt_syntax::{SyntaxKind, SyntaxNode};

/// A typed view of a CST node of a particular kind.
pub trait AstNode {
    fn can_cast(kind: SyntaxKind) -> bool
    where
        Self: Sized;
    fn cast(node: SyntaxNode) -> Option<Self>
    where
        Self: Sized;
    fn syntax(&self) -> &SyntaxNode;
}

macro_rules! ast_node {
    ($name:ident, $kind:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            syntax: SyntaxNode,
        }
        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == SyntaxKind::$kind
            }
            fn cast(node: SyntaxNode) -> Option<Self> {
                if node.kind() == SyntaxKind::$kind {
                    Some(Self { syntax: node })
                } else {
                    None
                }
            }
            fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }
        }
    };
}

ast_node!(SourceFile, SOURCE_FILE);
ast_node!(SelectStmt, SELECT_STMT);
ast_node!(SelectList, SELECT_LIST);
ast_node!(SelectItem, SELECT_ITEM);
ast_node!(FromClause, FROM_CLAUSE);
ast_node!(WhereClause, WHERE_CLAUSE);

impl SourceFile {
    /// All top-level statement nodes (e.g. `SELECT_STMT`, `EXPR_STMT`).
    pub fn statements(&self) -> impl Iterator<Item = SyntaxNode> + '_ {
        self.syntax.children()
    }
}

impl SelectStmt {
    pub fn select_list(&self) -> Option<SelectList> {
        self.syntax.children().find_map(SelectList::cast)
    }
    pub fn from_clause(&self) -> Option<FromClause> {
        self.syntax.children().find_map(FromClause::cast)
    }
    pub fn where_clause(&self) -> Option<WhereClause> {
        self.syntax.children().find_map(WhereClause::cast)
    }
}

impl SelectList {
    pub fn items(&self) -> impl Iterator<Item = SelectItem> + '_ {
        self.syntax.children().filter_map(SelectItem::cast)
    }
}
