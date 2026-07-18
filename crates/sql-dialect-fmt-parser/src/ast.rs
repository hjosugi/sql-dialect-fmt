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
        impl $name {
            pub fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }

            pub fn child<N: AstNode>(&self) -> Option<N> {
                self.syntax.children().find_map(N::cast)
            }

            pub fn children<'a, N: AstNode + 'a>(&'a self) -> impl Iterator<Item = N> + 'a {
                self.syntax.children().filter_map(N::cast)
            }
        }
    };
}

macro_rules! ast_nodes {
    ($($name:ident, $kind:ident;)*) => {
        $(ast_node!($name, $kind);)*
    };
}

ast_node!(SourceFile, SOURCE_FILE);
ast_node!(ExprStmt, EXPR_STMT);
ast_node!(SelectStmt, SELECT_STMT);
ast_node!(SelectList, SELECT_LIST);
ast_node!(SelectItem, SELECT_ITEM);
ast_node!(FromClause, FROM_CLAUSE);
ast_node!(WhereClause, WHERE_CLAUSE);

ast_nodes! {
    TableRef, TABLE_REF;
    LateralView, LATERAL_VIEW;
    AsOfTravel, AS_OF_TRAVEL;
    ArgList, ARG_LIST;
    TypeName, TYPE_NAME;
    Name, NAME;
    NameRef, NAME_REF;
    Literal, LITERAL;
    StarExpr, STAR_EXPR;
    ParenExpr, PAREN_EXPR;
    PrefixExpr, PREFIX_EXPR;
    BinExpr, BIN_EXPR;
    CallExpr, CALL_EXPR;
    IndexExpr, INDEX_EXPR;
    CastExpr, CAST_EXPR;
    BindMarker, BIND_MARKER;
    IntervalLiteral, INTERVAL_LITERAL;
    ArrayLiteral, ARRAY_LITERAL;
    ObjectLiteral, OBJECT_LITERAL;
    ObjectField, OBJECT_FIELD;
    WithQuery, WITH_QUERY;
    WithClause, WITH_CLAUSE;
    Cte, CTE;
    ColumnList, COLUMN_LIST;
    SetOp, SET_OP;
    Subquery, SUBQUERY;
    GroupByClause, GROUP_BY_CLAUSE;
    HavingClause, HAVING_CLAUSE;
    QualifyClause, QUALIFY_CLAUSE;
    OrderByClause, ORDER_BY_CLAUSE;
    OrderByItem, ORDER_BY_ITEM;
    LimitClause, LIMIT_CLAUSE;
    OffsetClause, OFFSET_CLAUSE;
    Join, JOIN;
    IsExpr, IS_EXPR;
    InExpr, IN_EXPR;
    BetweenExpr, BETWEEN_EXPR;
    ExistsExpr, EXISTS_EXPR;
    ExprList, EXPR_LIST;
    WindowExpr, WINDOW_EXPR;
    WindowSpec, WINDOW_SPEC;
    PartitionByClause, PARTITION_BY_CLAUSE;
    WindowFrame, WINDOW_FRAME;
    CaseExpr, CASE_EXPR;
    CaseWhen, CASE_WHEN;
    JsonAccess, JSON_ACCESS;
    LambdaExpr, LAMBDA_EXPR;
    LambdaParams, LAMBDA_PARAMS;
    ValuesClause, VALUES_CLAUSE;
    ValuesRow, VALUES_ROW;
    InsertStmt, INSERT_STMT;
    UpdateStmt, UPDATE_STMT;
    DeleteStmt, DELETE_STMT;
    MergeStmt, MERGE_STMT;
    SetClause, SET_CLAUSE;
    Assignment, ASSIGNMENT;
    MergeWhen, MERGE_WHEN;
    CreateStmt, CREATE_STMT;
    DropStmt, DROP_STMT;
    AlterStmt, ALTER_STMT;
    GrantStmt, GRANT_STMT;
    RevokeStmt, REVOKE_STMT;
    CallStmt, CALL_STMT;
    UseStmt, USE_STMT;
    ShowStmt, SHOW_STMT;
    DescribeStmt, DESCRIBE_STMT;
    TruncateStmt, TRUNCATE_STMT;
    CommentStmt, COMMENT_STMT;
    TransactionStmt, TRANSACTION_STMT;
    UndropStmt, UNDROP_STMT;
    BlockStmt, BLOCK_STMT;
    DeclareSection, DECLARE_SECTION;
    DeclareItem, DECLARE_ITEM;
    StmtList, STMT_LIST;
    ExceptionSection, EXCEPTION_SECTION;
    ExceptionWhen, EXCEPTION_WHEN;
    LetStmt, LET_STMT;
    AssignStmt, ASSIGN_STMT;
    ReturnStmt, RETURN_STMT;
    IfStmt, IF_STMT;
    LoopStmt, LOOP_STMT;
    CaseStmt, CASE_STMT;
    CaseStmtWhen, CASE_STMT_WHEN;
    ScriptStmt, SCRIPT_STMT;
    ColumnDefList, COLUMN_DEF_LIST;
    ColumnDef, COLUMN_DEF;
    RoutineReturnsClause, ROUTINE_RETURNS_CLAUSE;
    RoutineLanguageClause, ROUTINE_LANGUAGE_CLAUSE;
    WithinGroup, WITHIN_GROUP;
    PivotClause, PIVOT_CLAUSE;
    NamedArg, NAMED_ARG;
    MatchRecognize, MATCH_RECOGNIZE;
    MeasuresClause, MEASURES_CLAUSE;
    RowMatchClause, ROW_MATCH_CLAUSE;
    AfterMatchClause, AFTER_MATCH_CLAUSE;
    PatternClause, PATTERN_CLAUSE;
    PatternBody, PATTERN_BODY;
    SubsetClause, SUBSET_CLAUSE;
    DefineClause, DEFINE_CLAUSE;
    DefineItem, DEFINE_ITEM;
    StartWithClause, START_WITH_CLAUSE;
    ConnectByClause, CONNECT_BY_CLAUSE;
    FlowStmt, FLOW_STMT;
    SetStmt, SET_STMT;
    ExecuteStmt, EXECUTE_STMT;
    GroupingSets, GROUPING_SETS;
    CopyStmt, COPY_STMT;
    StageFileStmt, STAGE_FILE_STMT;
    CopyLocation, COPY_LOCATION;
    CopyOption, COPY_OPTION;
    StageRef, STAGE_REF;
    IntoClause, INTO_CLAUSE;
    InsertWhen, INSERT_WHEN;
    ObjectProperty, OBJECT_PROPERTY;
    AlterAction, ALTER_ACTION;
    StreamSource, STREAM_SOURCE;
    TaskAfter, TASK_AFTER;
    SemanticViewClause, SEMANTIC_VIEW_CLAUSE;
    SemanticViewItem, SEMANTIC_VIEW_ITEM;
    PrivList, PRIV_LIST;
    GrantTarget, GRANT_TARGET;
    Grantee, GRANTEE;
    VacuumStmt, VACUUM_STMT;
    OptimizeStmt, OPTIMIZE_STMT;
    ZorderClause, ZORDER_CLAUSE;
    CacheStmt, CACHE_STMT;
    UncacheStmt, UNCACHE_STMT;
    RefreshStmt, REFRESH_STMT;
    DescribeHistoryStmt, DESCRIBE_HISTORY_STMT;
}

impl SourceFile {
    /// All top-level statement nodes (e.g. `SELECT_STMT`, `EXPR_STMT`).
    pub fn statements(&self) -> impl Iterator<Item = SyntaxNode> + '_ {
        self.syntax.children()
    }

    /// Top-level statements of one typed kind.
    pub fn statements_of<'a, N: AstNode + 'a>(&'a self) -> impl Iterator<Item = N> + 'a {
        self.children::<N>()
    }
}

impl SelectStmt {
    pub fn select_list(&self) -> Option<SelectList> {
        self.child()
    }
    pub fn from_clause(&self) -> Option<FromClause> {
        self.child()
    }
    pub fn where_clause(&self) -> Option<WhereClause> {
        self.child()
    }
}

impl SelectList {
    pub fn items(&self) -> impl Iterator<Item = SelectItem> + '_ {
        self.children()
    }
}
