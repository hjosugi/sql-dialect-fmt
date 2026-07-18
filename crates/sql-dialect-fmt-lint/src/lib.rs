//! Lint rules for SQL, independent of any editor protocol.
//!
//! [`lint`] (and its options/dialect-aware variants) checks source text against the SDF rule set
//! and returns [`LintDiagnostic`]s located at byte ranges, so the CLI can render `path:line:col`
//! findings and the language server can convert them to LSP diagnostics without this crate ever
//! depending on LSP types.
//!
//! ## Rules
//!
//! | code | rule |
//! | --- | --- |
//! | `SDF001` | `SELECT *` in a select list |
//! | `SDF002` | large literal `IN (...)` list |
//! | `SDF003` | unsupported embedded `LANGUAGE ... AS $$...$$` |
//! | `SDF004` | `DELETE` without `WHERE` |
//! | `SDF005` | `UPDATE` without `WHERE` |
//! | `SDF006` | comma join in `FROM` (implicit cross join) |
//! | `SDF007` | `ORDER BY` ordinal (e.g. `ORDER BY 1`) |
//!
//! ## Suppression
//!
//! A finding is suppressed by a line comment on the previous line:
//!
//! ```sql
//! -- sql-dialect-fmt: disable-next-line SDF001
//! SELECT * FROM t;
//! ```
//!
//! Omitting the code (or writing `all`/`lint`) suppresses every rule for the next line.

use sql_dialect_fmt_parser::{parse_with_dialect, SyntaxKind, SyntaxNode};
use sql_dialect_fmt_syntax::keyword_kind;
use sql_dialect_fmt_text::LineIndex;

pub use sql_dialect_fmt_parser::Dialect;

/// Lint knobs. Every rule is on by default; each can be disabled independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LintOptions {
    /// Warn on top-level `SELECT *` in a select list (`SDF001`).
    pub select_wildcard: bool,
    /// Warn on large literal `IN (...)` lists (`SDF002`).
    pub large_in_list: bool,
    /// Warn when `LANGUAGE <name> AS $$...$$` uses an embedded language the formatter cannot
    /// format (`SDF003`).
    pub unsupported_embedded_language: bool,
    /// Warn on `DELETE` statements without a `WHERE` clause (`SDF004`).
    pub delete_without_where: bool,
    /// Warn on `UPDATE` statements without a `WHERE` clause (`SDF005`).
    pub update_without_where: bool,
    /// Warn on comma joins in `FROM` — implicit cross joins (`SDF006`).
    pub comma_join: bool,
    /// Warn on ordinal `ORDER BY` items such as `ORDER BY 1` (`SDF007`).
    pub order_by_ordinal: bool,
    /// Item count above which a literal `IN (...)` list is considered large.
    pub large_in_list_threshold: usize,
}

impl Default for LintOptions {
    fn default() -> Self {
        Self {
            select_wildcard: true,
            large_in_list: true,
            unsupported_embedded_language: true,
            delete_without_where: true,
            update_without_where: true,
            comma_join: true,
            order_by_ordinal: true,
            large_in_list_threshold: 100,
        }
    }
}

/// The stable identity of a lint rule; rendered as `SDFxxx` via [`LintCode::as_str`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LintCode {
    SelectWildcard,
    LargeInList,
    UnsupportedEmbeddedLanguage,
    DeleteWithoutWhere,
    UpdateWithoutWhere,
    CommaJoin,
    OrderByOrdinal,
}

impl LintCode {
    /// The `SDFxxx` code string editors and suppression comments use.
    pub fn as_str(self) -> &'static str {
        match self {
            LintCode::SelectWildcard => "SDF001",
            LintCode::LargeInList => "SDF002",
            LintCode::UnsupportedEmbeddedLanguage => "SDF003",
            LintCode::DeleteWithoutWhere => "SDF004",
            LintCode::UpdateWithoutWhere => "SDF005",
            LintCode::CommaJoin => "SDF006",
            LintCode::OrderByOrdinal => "SDF007",
        }
    }

    /// Parse an `SDFxxx` code string back into a [`LintCode`].
    pub fn from_code(value: &str) -> Option<Self> {
        match value {
            "SDF001" => Some(Self::SelectWildcard),
            "SDF002" => Some(Self::LargeInList),
            "SDF003" => Some(Self::UnsupportedEmbeddedLanguage),
            "SDF004" => Some(Self::DeleteWithoutWhere),
            "SDF005" => Some(Self::UpdateWithoutWhere),
            "SDF006" => Some(Self::CommaJoin),
            "SDF007" => Some(Self::OrderByOrdinal),
            _ => None,
        }
    }
}

/// How severe a finding is. Every current rule emits [`LintSeverity::Warning`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LintSeverity {
    Warning,
    Error,
}

/// One lint finding, located at a byte range into the linted source text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintDiagnostic {
    /// Which rule fired.
    pub code: LintCode,
    /// Severity of the finding.
    pub severity: LintSeverity,
    /// Human-readable description of the problem.
    pub message: String,
    /// Byte range of the offending source span.
    pub range: std::ops::Range<usize>,
}

/// Lint `text` as Snowflake SQL with default [`LintOptions`].
pub fn lint(text: &str) -> Vec<LintDiagnostic> {
    lint_with_options(text, LintOptions::default())
}

/// Lint `text` as Snowflake SQL with explicit options.
pub fn lint_with_options(text: &str, options: LintOptions) -> Vec<LintDiagnostic> {
    lint_with_dialect(text, Dialect::Snowflake, options)
}

/// Lint `text` as `dialect`, honoring `options` and suppression comments. Findings are sorted by
/// source position.
pub fn lint_with_dialect(
    text: &str,
    dialect: Dialect,
    options: LintOptions,
) -> Vec<LintDiagnostic> {
    let context = LintContext {
        tokens: lex_tokens(text, dialect),
        root: parse_with_dialect(text, dialect).syntax(),
    };

    let mut diagnostics = Vec::new();
    run_rule::<SelectWildcardRule>(&context, &options, &mut diagnostics);
    run_rule::<LargeInListRule>(&context, &options, &mut diagnostics);
    run_rule::<UnsupportedEmbeddedLanguageRule>(&context, &options, &mut diagnostics);
    run_rule::<DeleteWithoutWhereRule>(&context, &options, &mut diagnostics);
    run_rule::<UpdateWithoutWhereRule>(&context, &options, &mut diagnostics);
    run_rule::<CommaJoinRule>(&context, &options, &mut diagnostics);
    run_rule::<OrderByOrdinalRule>(&context, &options, &mut diagnostics);

    let index = LineIndex::new(text);
    diagnostics.retain(|diagnostic| !is_suppressed(text, &index, diagnostic));
    diagnostics.sort_by_key(|diagnostic| (diagnostic.range.start, diagnostic.range.end));
    diagnostics
}

/// Everything a rule can look at: the classified token stream and the lossless CST.
struct LintContext<'a> {
    tokens: Vec<LintToken<'a>>,
    root: SyntaxNode,
}

fn run_rule<R: LintRule>(
    context: &LintContext<'_>,
    options: &LintOptions,
    out: &mut Vec<LintDiagnostic>,
) {
    if R::enabled(options) {
        R::check(context, options, out);
    }
}

trait LintRule {
    const CODE: LintCode;

    fn enabled(options: &LintOptions) -> bool;

    fn check(context: &LintContext<'_>, options: &LintOptions, out: &mut Vec<LintDiagnostic>);

    fn warning(range: std::ops::Range<usize>, message: &str) -> LintDiagnostic {
        LintDiagnostic {
            code: Self::CODE,
            severity: LintSeverity::Warning,
            message: message.to_string(),
            range,
        }
    }
}

/// The coarse token classification the token-stream rules key off. Deliberately smaller than a
/// highlighter's palette: lint rules only care about keywords, plain words, dollar-quoted bodies,
/// and trivia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenClass {
    /// Whitespace or a comment.
    Trivia,
    /// A reserved, contextual, or soft keyword.
    Keyword,
    /// An unquoted identifier that is not a keyword (includes type names).
    Word,
    /// A dollar-quoted string body (`$$...$$`).
    DollarString,
    /// Anything else: punctuation, operators, literals, quoted identifiers, errors.
    Other,
}

#[derive(Clone, Debug)]
struct LintToken<'a> {
    class: TokenClass,
    text: &'a str,
    range: std::ops::Range<usize>,
}

fn lex_tokens(text: &str, dialect: Dialect) -> Vec<LintToken<'_>> {
    let lexed = sql_dialect_fmt_lexer::tokenize_for_dialect(text, dialect);
    lexed
        .tokens
        .into_iter()
        .scan(0usize, |offset, token| {
            let start = *offset;
            *offset += token.text.len();
            Some(LintToken {
                class: classify(token.kind, token.text),
                text: token.text,
                range: start..*offset,
            })
        })
        .collect()
}

fn classify(kind: SyntaxKind, text: &str) -> TokenClass {
    if kind.is_trivia() {
        return TokenClass::Trivia;
    }
    if kind.is_keyword()
        || kind == SyntaxKind::CONTEXTUAL_KEYWORD
        || (kind == SyntaxKind::IDENT && keyword_kind(text).is_some())
    {
        return TokenClass::Keyword;
    }
    match kind {
        SyntaxKind::IDENT => TokenClass::Word,
        SyntaxKind::DOLLAR_STRING => TokenClass::DollarString,
        _ => TokenClass::Other,
    }
}

fn is_significant(token: &LintToken<'_>) -> bool {
    token.class != TokenClass::Trivia
}

fn is_keyword(token: &LintToken<'_>, expected: &str) -> bool {
    token.class == TokenClass::Keyword && token.text.eq_ignore_ascii_case(expected)
}

// ---------------------------------------------------------------------------
// SDF001: SELECT *
// ---------------------------------------------------------------------------

struct SelectWildcardRule;

impl LintRule for SelectWildcardRule {
    const CODE: LintCode = LintCode::SelectWildcard;

    fn enabled(options: &LintOptions) -> bool {
        options.select_wildcard
    }

    fn check(context: &LintContext<'_>, _options: &LintOptions, out: &mut Vec<LintDiagnostic>) {
        let mut in_select_list = false;
        let mut paren_depth = 0usize;
        let mut previous_significant: Option<&str> = None;

        for token in context.tokens.iter().filter(|token| is_significant(token)) {
            if is_keyword(token, "select") {
                in_select_list = true;
                paren_depth = 0;
                previous_significant = Some(token.text);
                continue;
            }

            if in_select_list {
                match token.text {
                    "(" => paren_depth += 1,
                    ")" => paren_depth = paren_depth.saturating_sub(1),
                    ";" if paren_depth == 0 => in_select_list = false,
                    _ => {
                        if paren_depth == 0 && is_keyword(token, "from") {
                            in_select_list = false;
                        } else if paren_depth == 0
                            && token.text == "*"
                            && previous_significant.is_some_and(is_wildcard_prefix)
                        {
                            out.push(Self::warning(
                                token.range.clone(),
                                "avoid SELECT * in shared SQL; list columns explicitly",
                            ));
                        }
                    }
                }
            }

            previous_significant = Some(token.text);
        }
    }
}

fn is_wildcard_prefix(text: &str) -> bool {
    matches!(text, "," | ".")
        || text.eq_ignore_ascii_case("select")
        || text.eq_ignore_ascii_case("distinct")
        || text.eq_ignore_ascii_case("all")
}

// ---------------------------------------------------------------------------
// SDF002: large IN list
// ---------------------------------------------------------------------------

struct LargeInListRule;

impl LintRule for LargeInListRule {
    const CODE: LintCode = LintCode::LargeInList;

    fn enabled(options: &LintOptions) -> bool {
        options.large_in_list
    }

    fn check(context: &LintContext<'_>, options: &LintOptions, out: &mut Vec<LintDiagnostic>) {
        #[derive(Debug)]
        struct InList {
            start: usize,
            depth: usize,
            commas: usize,
            saw_top_level_item: bool,
            possible_subquery: bool,
        }

        let mut pending_in: Option<usize> = None;
        let mut list: Option<InList> = None;

        for token in context.tokens.iter().filter(|token| is_significant(token)) {
            let mut close_list_at: Option<usize> = None;
            if let Some(active) = list.as_mut() {
                match token.text {
                    "(" => active.depth += 1,
                    ")" => {
                        active.depth = active.depth.saturating_sub(1);
                        if active.depth == 0 {
                            close_list_at = Some(token.range.end);
                        }
                    }
                    "," if active.depth == 1 => active.commas += 1,
                    _ if active.depth == 1 && !active.saw_top_level_item => {
                        active.saw_top_level_item = true;
                        if is_keyword(token, "select") || is_keyword(token, "with") {
                            active.possible_subquery = true;
                        }
                    }
                    _ => {}
                }

                if let Some(end) = close_list_at {
                    if let Some(active) = list.take() {
                        let item_count = if active.saw_top_level_item {
                            active.commas + 1
                        } else {
                            0
                        };
                        if !active.possible_subquery && item_count > options.large_in_list_threshold
                        {
                            out.push(Self::warning(
                                active.start..end,
                                "large IN list; prefer a temp table, CTE, or semi-join when practical",
                            ));
                        }
                    }
                }
                continue;
            }

            if let Some(start) = pending_in.take() {
                if token.text == "(" {
                    list = Some(InList {
                        start,
                        depth: 1,
                        commas: 0,
                        saw_top_level_item: false,
                        possible_subquery: false,
                    });
                    continue;
                }
            }

            if is_keyword(token, "in") {
                pending_in = Some(token.range.start);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SDF003: unsupported embedded language
// ---------------------------------------------------------------------------

struct UnsupportedEmbeddedLanguageRule;

impl LintRule for UnsupportedEmbeddedLanguageRule {
    const CODE: LintCode = LintCode::UnsupportedEmbeddedLanguage;

    fn enabled(options: &LintOptions) -> bool {
        options.unsupported_embedded_language
    }

    fn check(context: &LintContext<'_>, _options: &LintOptions, out: &mut Vec<LintDiagnostic>) {
        let mut expect_language_name = false;
        let mut language_name: Option<(&str, std::ops::Range<usize>)> = None;
        let mut saw_as_after_language = false;

        for token in &context.tokens {
            if token.class == TokenClass::Trivia {
                continue;
            }
            if token.text == ";" {
                expect_language_name = false;
                language_name = None;
                saw_as_after_language = false;
                continue;
            }
            match token.class {
                TokenClass::DollarString => {
                    if saw_as_after_language {
                        if let Some((word, range)) = language_name.take() {
                            if !is_supported_embedded_language(word) {
                                out.push(Self::warning(
                                    range,
                                    &format!(
                                        "unsupported embedded language {word}; expected SQL, JAVASCRIPT, PYTHON, JAVA, or SCALA"
                                    ),
                                ));
                            }
                        }
                    }
                    expect_language_name = false;
                    saw_as_after_language = false;
                }
                TokenClass::Keyword if token.text.eq_ignore_ascii_case("language") => {
                    expect_language_name = true;
                    language_name = None;
                    saw_as_after_language = false;
                }
                TokenClass::Keyword | TokenClass::Word if expect_language_name => {
                    language_name = Some((token.text, token.range.clone()));
                    expect_language_name = false;
                }
                TokenClass::Keyword
                    if language_name.is_some() && token.text.eq_ignore_ascii_case("as") =>
                {
                    saw_as_after_language = true;
                }
                _ => {
                    expect_language_name = false;
                }
            }
        }
    }
}

fn is_supported_embedded_language(word: &str) -> bool {
    ["SQL", "JAVASCRIPT", "PYTHON", "JAVA", "SCALA"]
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(word))
}

// ---------------------------------------------------------------------------
// SDF004 / SDF005: DELETE / UPDATE without WHERE
// ---------------------------------------------------------------------------

struct DeleteWithoutWhereRule;

impl LintRule for DeleteWithoutWhereRule {
    const CODE: LintCode = LintCode::DeleteWithoutWhere;

    fn enabled(options: &LintOptions) -> bool {
        options.delete_without_where
    }

    fn check(context: &LintContext<'_>, _options: &LintOptions, out: &mut Vec<LintDiagnostic>) {
        check_statement_without_where(
            &context.root,
            SyntaxKind::DELETE_STMT,
            "DELETE without WHERE affects every row; add a WHERE clause or use TRUNCATE",
            Self::warning,
            out,
        );
    }
}

struct UpdateWithoutWhereRule;

impl LintRule for UpdateWithoutWhereRule {
    const CODE: LintCode = LintCode::UpdateWithoutWhere;

    fn enabled(options: &LintOptions) -> bool {
        options.update_without_where
    }

    fn check(context: &LintContext<'_>, _options: &LintOptions, out: &mut Vec<LintDiagnostic>) {
        check_statement_without_where(
            &context.root,
            SyntaxKind::UPDATE_STMT,
            "UPDATE without WHERE affects every row; add a WHERE clause",
            Self::warning,
            out,
        );
    }
}

/// Shared walk for SDF004/SDF005: flag `kind` statements with no direct `WHERE_CLAUSE` child,
/// anchored at the statement's leading keyword. `DELETE`/`UPDATE` actions inside `MERGE ... WHEN`
/// arms are separate node kinds (`MERGE_WHEN`), so they never trip this.
fn check_statement_without_where(
    root: &SyntaxNode,
    kind: SyntaxKind,
    message: &str,
    warning: impl Fn(std::ops::Range<usize>, &str) -> LintDiagnostic,
    out: &mut Vec<LintDiagnostic>,
) {
    for statement in root.descendants().filter(|node| node.kind() == kind) {
        let has_where = statement
            .children()
            .any(|child| child.kind() == SyntaxKind::WHERE_CLAUSE);
        if has_where {
            continue;
        }
        if let Some(range) = first_significant_token_range(&statement) {
            out.push(warning(range, message));
        }
    }
}

// ---------------------------------------------------------------------------
// SDF006: comma join in FROM
// ---------------------------------------------------------------------------

struct CommaJoinRule;

impl LintRule for CommaJoinRule {
    const CODE: LintCode = LintCode::CommaJoin;

    fn enabled(options: &LintOptions) -> bool {
        options.comma_join
    }

    fn check(context: &LintContext<'_>, _options: &LintOptions, out: &mut Vec<LintDiagnostic>) {
        for from_clause in context
            .root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::FROM_CLAUSE)
        {
            let elements: Vec<_> = from_clause.children_with_tokens().collect();
            for (position, element) in elements.iter().enumerate() {
                let Some(comma) = element
                    .as_token()
                    .filter(|token| token.kind() == SyntaxKind::COMMA)
                else {
                    continue;
                };
                // `FROM t, LATERAL ...` and `FROM t, TABLE(...)` are the documented lateral /
                // table-function idioms (e.g. LATERAL FLATTEN), not accidental cross joins.
                if joined_table_ref_is_lateral_or_table_function(&elements[position + 1..]) {
                    continue;
                }
                let start: usize = comma.text_range().start().into();
                let end: usize = comma.text_range().end().into();
                out.push(Self::warning(
                    start..end,
                    "comma join is an implicit cross join; use an explicit JOIN ... ON instead",
                ));
            }
        }
    }
}

/// Whether the first node following a `FROM`-clause comma is a `TABLE_REF` that begins with
/// `LATERAL` or `TABLE(` — Snowflake's lateral/table-function idioms rather than a cross join.
fn joined_table_ref_is_lateral_or_table_function(
    rest: &[sql_dialect_fmt_syntax::SyntaxElement],
) -> bool {
    let Some(node) = rest.iter().find_map(|element| element.as_node()) else {
        return false;
    };
    if node.kind() != SyntaxKind::TABLE_REF {
        return false;
    }
    node.descendants_with_tokens()
        .filter_map(|child| child.into_token())
        .find(|token| !token.kind().is_trivia())
        .is_some_and(|token| matches!(token.kind(), SyntaxKind::LATERAL_KW | SyntaxKind::TABLE_KW))
}

// ---------------------------------------------------------------------------
// SDF007: ORDER BY ordinal
// ---------------------------------------------------------------------------

struct OrderByOrdinalRule;

impl LintRule for OrderByOrdinalRule {
    const CODE: LintCode = LintCode::OrderByOrdinal;

    fn enabled(options: &LintOptions) -> bool {
        options.order_by_ordinal
    }

    fn check(context: &LintContext<'_>, _options: &LintOptions, out: &mut Vec<LintDiagnostic>) {
        for order_by in context
            .root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::ORDER_BY_CLAUSE)
        {
            // Inside `OVER (...)` and `WITHIN GROUP (...)` an integer is a constant expression,
            // not a positional column reference, so only query-level ORDER BY is checked.
            if order_by.parent().is_some_and(|parent| {
                matches!(
                    parent.kind(),
                    SyntaxKind::WINDOW_SPEC | SyntaxKind::WITHIN_GROUP
                )
            }) {
                continue;
            }
            for item in order_by
                .children()
                .filter(|node| node.kind() == SyntaxKind::ORDER_BY_ITEM)
            {
                let Some(range) = ordinal_literal_range(&item) else {
                    continue;
                };
                out.push(Self::warning(
                    range,
                    "ORDER BY ordinal is fragile; order by the column name or expression instead",
                ));
            }
        }
    }
}

/// If the ORDER BY item's sort key is a bare integer literal, the byte range of that integer.
fn ordinal_literal_range(item: &SyntaxNode) -> Option<std::ops::Range<usize>> {
    let expr = item.children().next()?;
    if expr.kind() != SyntaxKind::LITERAL {
        return None;
    }
    let token = expr
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())?;
    if token.kind() != SyntaxKind::INT_NUMBER {
        return None;
    }
    let start: usize = token.text_range().start().into();
    let end: usize = token.text_range().end().into();
    Some(start..end)
}

fn first_significant_token_range(node: &SyntaxNode) -> Option<std::ops::Range<usize>> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| {
            let start: usize = token.text_range().start().into();
            let end: usize = token.text_range().end().into();
            start..end
        })
}

// ---------------------------------------------------------------------------
// Suppression comments
// ---------------------------------------------------------------------------

fn is_suppressed(text: &str, index: &LineIndex<'_>, diagnostic: &LintDiagnostic) -> bool {
    let line = index.utf8_position(diagnostic.range.start).line as usize;
    if line == 0 {
        return false;
    }
    let Some(previous_line) = text.lines().nth(line - 1) else {
        return false;
    };
    line_suppresses_code(previous_line, diagnostic.code)
}

fn line_suppresses_code(line: &str, code: LintCode) -> bool {
    let trimmed = line.trim_start();
    let Some(comment) = trimmed.strip_prefix("--") else {
        return false;
    };
    let Some((_, rest)) = comment.split_once("sql-dialect-fmt: disable-next-line") else {
        return false;
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return true;
    }
    rest.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
        .filter(|part| !part.is_empty())
        .any(|part| {
            part.eq_ignore_ascii_case(code.as_str())
                || part.eq_ignore_ascii_case("all")
                || part.eq_ignore_ascii_case("lint")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(text: &str) -> Vec<&'static str> {
        lint(text)
            .into_iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    fn find(text: &str, code: LintCode) -> LintDiagnostic {
        lint(text)
            .into_iter()
            .find(|diagnostic| diagnostic.code == code)
            .unwrap_or_else(|| panic!("expected a {} finding in {text:?}", code.as_str()))
    }

    #[test]
    fn select_wildcard_is_flagged_at_the_star() {
        let text = "SELECT * FROM t;";
        let diagnostic = find(text, LintCode::SelectWildcard);
        assert_eq!(diagnostic.severity, LintSeverity::Warning);
        assert_eq!(
            diagnostic.range,
            text.find('*').unwrap()..text.find('*').unwrap() + 1
        );
    }

    #[test]
    fn select_wildcard_ignores_function_stars() {
        assert!(codes("SELECT count(*) FROM t;").is_empty());
    }

    #[test]
    fn large_in_list_respects_threshold_option() {
        let text = "SELECT id FROM t WHERE id IN (1, 2, 3);";
        assert!(codes(text).is_empty());
        let options = LintOptions {
            large_in_list_threshold: 2,
            ..LintOptions::default()
        };
        let diagnostics = lint_with_options(text, options);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, LintCode::LargeInList);
        assert_eq!(diagnostics[0].range.start, text.find("IN").unwrap());
    }

    #[test]
    fn in_subquery_is_not_a_large_in_list() {
        let options = LintOptions {
            large_in_list_threshold: 0,
            ..LintOptions::default()
        };
        assert!(lint_with_options(
            "SELECT id FROM t WHERE id IN (SELECT id FROM src);",
            options
        )
        .is_empty());
    }

    #[test]
    fn unsupported_embedded_language_is_flagged() {
        let text = "CREATE FUNCTION f() RETURNS STRING LANGUAGE RUBY AS $$x$$;";
        let diagnostic = find(text, LintCode::UnsupportedEmbeddedLanguage);
        assert!(diagnostic.message.contains("RUBY"));
        let start = text.find("RUBY").unwrap();
        assert_eq!(diagnostic.range, start..start + 4);
    }

    #[test]
    fn embedded_language_does_not_fire_for_plain_columns_or_dynamic_sql() {
        assert!(codes("SELECT language FROM t;").is_empty());
        assert!(codes("EXECUTE IMMEDIATE $$ SELECT 1 $$;").is_empty());
    }

    #[test]
    fn delete_without_where_is_flagged_at_the_delete_keyword() {
        let text = "DELETE FROM t;";
        let diagnostic = find(text, LintCode::DeleteWithoutWhere);
        assert_eq!(diagnostic.severity, LintSeverity::Warning);
        assert_eq!(diagnostic.range, 0..6);
    }

    #[test]
    fn delete_with_where_is_clean() {
        assert!(codes("DELETE FROM t WHERE id = 1;").is_empty());
        assert!(codes("DELETE FROM t USING s WHERE t.id = s.id;").is_empty());
    }

    #[test]
    fn merge_matched_delete_is_not_a_bare_delete() {
        let text = "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN DELETE;";
        assert!(codes(text).is_empty());
    }

    #[test]
    fn delete_inside_a_scripting_block_is_flagged() {
        let text = "BEGIN\n    DELETE FROM t;\nEND;";
        let diagnostic = find(text, LintCode::DeleteWithoutWhere);
        assert_eq!(diagnostic.range.start, text.find("DELETE").unwrap());
    }

    #[test]
    fn update_without_where_is_flagged() {
        let text = "SELECT 1;\nUPDATE t SET a = 1;";
        let diagnostic = find(text, LintCode::UpdateWithoutWhere);
        assert_eq!(diagnostic.range.start, text.find("UPDATE").unwrap());
    }

    #[test]
    fn update_with_where_is_clean() {
        assert!(codes("UPDATE t SET a = 1 WHERE b = 2;").is_empty());
    }

    #[test]
    fn merge_matched_update_is_not_a_bare_update() {
        let text = "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET t.a = s.a;";
        assert!(codes(text).is_empty());
    }

    #[test]
    fn comma_join_is_flagged_at_the_comma() {
        let text = "SELECT a FROM t1, t2 WHERE t1.id = t2.id;";
        let diagnostic = find(text, LintCode::CommaJoin);
        let comma = text.find(',').unwrap();
        assert_eq!(diagnostic.range, comma..comma + 1);
    }

    #[test]
    fn explicit_joins_and_argument_commas_are_clean() {
        assert!(codes("SELECT a FROM t1 JOIN t2 ON t1.id = t2.id;").is_empty());
        assert!(codes("SELECT a FROM t1 CROSS JOIN t2;").is_empty());
        assert!(codes("SELECT d FROM TABLE(f(x, y));").is_empty());
        assert!(codes("SELECT greatest(a, b) FROM t;").is_empty());
    }

    #[test]
    fn lateral_and_table_function_commas_are_clean() {
        assert!(codes("SELECT a FROM t, LATERAL FLATTEN(input => t.v) f;").is_empty());
        assert!(codes("SELECT b FROM t1, TABLE(GENERATOR(ROWCOUNT => 10));").is_empty());
    }

    #[test]
    fn multiple_comma_joins_produce_one_finding_each() {
        let diagnostics = lint("SELECT a FROM t1, t2, t3;");
        let commas = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == LintCode::CommaJoin)
            .count();
        assert_eq!(commas, 2);
    }

    #[test]
    fn order_by_ordinal_is_flagged_per_item() {
        let text = "SELECT a, b FROM t ORDER BY 1 DESC, b ASC;";
        let diagnostics = lint(text);
        let ordinals: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == LintCode::OrderByOrdinal)
            .collect();
        assert_eq!(ordinals.len(), 1);
        let one = text.find("BY 1").unwrap() + 3;
        assert_eq!(ordinals[0].range, one..one + 1);
    }

    #[test]
    fn order_by_expressions_and_limits_are_clean() {
        assert!(codes("SELECT a FROM t ORDER BY a;").is_empty());
        assert!(codes("SELECT a FROM t ORDER BY a + 1;").is_empty());
        assert!(codes("SELECT a FROM t ORDER BY a LIMIT 1;").is_empty());
    }

    #[test]
    fn window_and_within_group_order_by_are_not_ordinals() {
        assert!(codes("SELECT row_number() OVER (ORDER BY 1) FROM t;").is_empty());
        assert!(codes("SELECT listagg(a) WITHIN GROUP (ORDER BY 1) FROM t;").is_empty());
    }

    #[test]
    fn each_new_rule_is_gated_by_its_option() {
        for (text, disabled) in [
            (
                "DELETE FROM t;",
                LintOptions {
                    delete_without_where: false,
                    ..LintOptions::default()
                },
            ),
            (
                "UPDATE t SET a = 1;",
                LintOptions {
                    update_without_where: false,
                    ..LintOptions::default()
                },
            ),
            (
                "SELECT a FROM t1, t2;",
                LintOptions {
                    comma_join: false,
                    ..LintOptions::default()
                },
            ),
            (
                "SELECT a FROM t ORDER BY 1;",
                LintOptions {
                    order_by_ordinal: false,
                    ..LintOptions::default()
                },
            ),
        ] {
            assert!(!lint(text).is_empty(), "{text} should lint by default");
            assert!(
                lint_with_options(text, disabled).is_empty(),
                "{text} should be clean when its rule is disabled"
            );
        }
    }

    #[test]
    fn databricks_dialect_is_honored() {
        let diagnostics = lint_with_dialect(
            "DELETE FROM `my schema`.`t`;",
            Dialect::Databricks,
            LintOptions::default(),
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, LintCode::DeleteWithoutWhere);
    }

    #[test]
    fn findings_are_sorted_by_position() {
        let text = "SELECT * FROM t1, t2 ORDER BY 1;";
        let diagnostics = lint(text);
        let starts: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.range.start)
            .collect();
        let mut sorted = starts.clone();
        sorted.sort_unstable();
        assert_eq!(starts, sorted);
        assert_eq!(diagnostics.len(), 3);
    }

    #[test]
    fn disable_next_line_suppresses_specific_lint_code() {
        let text = "-- sql-dialect-fmt: disable-next-line SDF001\nSELECT * FROM t;";
        assert!(lint(text).is_empty());
    }

    #[test]
    fn disable_next_line_suppresses_all_lint_codes_when_code_is_omitted() {
        let text = "-- sql-dialect-fmt: disable-next-line\nSELECT * FROM t;";
        assert!(lint(text).is_empty());
    }

    #[test]
    fn disable_next_line_does_not_suppress_other_lines() {
        let text =
            "-- sql-dialect-fmt: disable-next-line SDF001\nSELECT a FROM t;\nSELECT * FROM t;";
        assert_eq!(lint(text).len(), 1);
    }

    #[test]
    fn disable_next_line_suppresses_new_rules_too() {
        for (text, code) in [
            (
                "-- sql-dialect-fmt: disable-next-line SDF004\nDELETE FROM t;",
                "SDF004",
            ),
            (
                "-- sql-dialect-fmt: disable-next-line SDF005\nUPDATE t SET a = 1;",
                "SDF005",
            ),
            (
                "-- sql-dialect-fmt: disable-next-line SDF006\nSELECT a FROM t1, t2;",
                "SDF006",
            ),
            (
                "-- sql-dialect-fmt: disable-next-line SDF007\nSELECT a FROM t ORDER BY 1;",
                "SDF007",
            ),
        ] {
            assert!(lint(text).is_empty(), "{code} should be suppressed");
        }
    }

    #[test]
    fn codes_round_trip() {
        for code in [
            LintCode::SelectWildcard,
            LintCode::LargeInList,
            LintCode::UnsupportedEmbeddedLanguage,
            LintCode::DeleteWithoutWhere,
            LintCode::UpdateWithoutWhere,
            LintCode::CommaJoin,
            LintCode::OrderByOrdinal,
        ] {
            assert_eq!(LintCode::from_code(code.as_str()), Some(code));
        }
        assert_eq!(LintCode::from_code("SDF999"), None);
    }
}
