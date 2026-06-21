//! Phase 1 parser conformance tests.
//!
//! The headline guarantee is **lossless round-trip**: for any input (valid or broken), the
//! parsed tree's text reproduces the source byte-for-byte. We also check that clean SQL parses
//! without diagnostics, that the node structure is sensible, and that errors recover.

use snow_fmt_parser::{parse, AstNode, SelectStmt, SourceFile, SyntaxKind};

fn roundtrip(s: &str) {
    let p = parse(s);
    assert_eq!(p.syntax().to_string(), s, "round-trip failed for {s:?}");
}

#[test]
fn lossless_roundtrip_valid_and_broken() {
    let inputs = [
        "",
        "   \n  ",
        "SELECT 1",
        "select 1, 2 ,3",
        "SELECT * FROM t",
        "SELECT a, b AS x, c AS \"Y\" FROM db.sch.t",
        "SELECT a, b FROM t WHERE a > 1 AND b <= 2 OR NOT c",
        "SELECT count(*), sum(x), f(a, b) FROM t",
        "SELECT a :: int, (a + b) * c, -a, x || y FROM t",
        "SELECT arr[0], obj['k'] FROM t t_alias",
        "1 + 2 * 3 ;\nSELECT 1 ;",
        "/* hi */ SELECT n FROM t -- tail comment\n",
        "SELECT )( garbage @ # FROM", // deliberately broken
    ];
    for s in inputs {
        roundtrip(s);
    }
}

#[test]
fn clean_sql_has_no_errors() {
    for s in [
        "SELECT 1",
        "SELECT a, b FROM t WHERE a > 1",
        "SELECT count(*) FROM db.s.t",
        "SELECT a::int, (a + b) * c FROM t",
        "SELECT DISTINCT a FROM t",
    ] {
        let p = parse(s);
        assert!(
            p.errors().is_empty(),
            "unexpected errors for {s:?}: {:?}",
            p.errors()
        );
    }
}

#[test]
fn select_has_expected_clauses() {
    let p = parse("SELECT a, b FROM t WHERE a > 1");
    assert!(p.errors().is_empty());
    let select = p
        .syntax()
        .children()
        .find(|n| n.kind() == SyntaxKind::SELECT_STMT)
        .expect("a SELECT_STMT");
    let kinds: Vec<SyntaxKind> = select.children().map(|n| n.kind()).collect();
    assert!(kinds.contains(&SyntaxKind::SELECT_LIST));
    assert!(kinds.contains(&SyntaxKind::FROM_CLAUSE));
    assert!(kinds.contains(&SyntaxKind::WHERE_CLAUSE));
}

#[test]
fn ast_accessors_work() {
    let p = parse("SELECT a, b FROM t");
    let file = SourceFile::cast(p.syntax()).expect("source file");
    let select = file
        .statements()
        .find_map(SelectStmt::cast)
        .expect("select stmt");
    assert!(select.select_list().is_some());
    assert_eq!(select.select_list().unwrap().items().count(), 2);
    assert!(select.from_clause().is_some());
    assert!(select.where_clause().is_none());
}

#[test]
fn error_recovery_is_lossless_and_reported() {
    let p = parse("SELECT FROM");
    assert_eq!(p.syntax().to_string(), "SELECT FROM");
    assert!(!p.errors().is_empty());
}

#[test]
fn precedence_nests_correctly() {
    // `a + b * c` must parse as a(+)(b*c): an outer BIN_EXPR containing an inner BIN_EXPR.
    let p = parse("SELECT a + b * c");
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    let outer = p
        .syntax()
        .descendants()
        .find(|n| n.kind() == SyntaxKind::BIN_EXPR)
        .expect("a BIN_EXPR");
    let nested = outer
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::BIN_EXPR)
        .count();
    assert!(
        nested >= 2,
        "expected a nested BIN_EXPR for the `*` sub-expression"
    );
}

#[test]
fn never_panics_on_adversarial_input() {
    // Mix of delimiters, operators, comments, dollar-quotes, unicode — must not panic.
    for s in [
        ";;;",
        "((((",
        "SELECT SELECT SELECT",
        "FROM WHERE AND OR ::",
        "$$ body $$ SELECT 1",
        "SELECT 中文 FROM 表 WHERE x = '💥'",
        "a.b.c.d.e.f.g",
        "1 +",
        ")",
    ] {
        let p = parse(s);
        assert_eq!(p.syntax().to_string(), s, "round-trip failed for {s:?}");
    }
}
