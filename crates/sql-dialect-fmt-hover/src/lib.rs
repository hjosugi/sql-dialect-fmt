//! Hover information for Snowflake SQL editor integrations.
//!
//! This crate is intentionally LSP-agnostic. LSP, Tree-sitter adapters, and CLI
//! diagnostics can all call [`hover_at`] and translate the result into their own
//! wire format.

use sql_dialect_fmt_lexer::tokenize;
use sql_dialect_fmt_syntax::SyntaxKind;
use std::ops::Range;

pub const CREATE_PROCEDURE_DOCS: &str =
    "https://docs.snowflake.com/en/sql-reference/sql/create-procedure";
pub const CREATE_TASK_DOCS: &str = "https://docs.snowflake.com/en/sql-reference/sql/create-task";
pub const DATA_TYPES_DOCS: &str = "https://docs.snowflake.com/en/sql-reference/data-types";

const ROUTINE_OPTION_STOPS: &[&str] = &[
    "AS",
    "ARTIFACT_REPOSITORY",
    "CALLED",
    "COMMENT",
    "COPY",
    "EXECUTE",
    "EXTERNAL_ACCESS_INTEGRATIONS",
    "HANDLER",
    "IMMUTABLE",
    "IMPORTS",
    "LANGUAGE",
    "MEMOIZABLE",
    "NULL",
    "PACKAGES",
    "RETURNS",
    "RUNTIME_VERSION",
    "SECRETS",
    "SECURE",
    "STRICT",
    "TARGET_PATH",
    "VOLATILE",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hover {
    pub kind: HoverKind,
    pub title: String,
    pub body: String,
    pub range: Range<usize>,
    pub docs_url: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverKind {
    Keyword,
    Type,
    Procedure,
    Task,
    Language,
    Property,
}

#[derive(Clone, Debug)]
struct SpannedToken<'a> {
    kind: SyntaxKind,
    text: &'a str,
    range: Range<usize>,
}

#[derive(Clone, Copy)]
struct StaticHover {
    kind: HoverKind,
    title: &'static str,
    body: &'static str,
    docs_url: Option<&'static str>,
}

/// Return hover information for the token at `offset`.
///
/// Offsets are byte offsets, matching LSP's UTF-8 internal representation after
/// the caller converts from line/column. Trivia currently has no hover.
pub fn hover_at(source: &str, offset: usize) -> Option<Hover> {
    let tokens = spanned_tokens(source);
    let index = token_at(&tokens, offset)?;
    let token = tokens[index].clone();

    if let Some(hover) = procedure_symbol_hover(source, &tokens, index) {
        return Some(hover);
    }
    if let Some(hover) = task_symbol_hover(source, &tokens, index) {
        return Some(hover);
    }
    if let Some(hover) = type_hover(&token) {
        return Some(hover);
    }
    if let Some(hover) = language_hover(&token) {
        return Some(hover);
    }
    if let Some(hover) = property_hover(&token) {
        return Some(hover);
    }
    keyword_hover(&token)
}

fn spanned_tokens(source: &str) -> Vec<SpannedToken<'_>> {
    let mut offset = 0usize;
    tokenize(source)
        .tokens
        .into_iter()
        .filter_map(|token| {
            let start = offset;
            offset += token.text.len();
            (!token.kind.is_trivia()).then_some(SpannedToken {
                kind: token.kind,
                text: token.text,
                range: start..offset,
            })
        })
        .collect()
}

fn token_at(tokens: &[SpannedToken<'_>], offset: usize) -> Option<usize> {
    tokens
        .iter()
        .position(|token| token.range.start <= offset && offset < token.range.end)
        .or_else(|| {
            offset.checked_sub(1).and_then(|previous| {
                tokens
                    .iter()
                    .position(|token| token.range.start <= previous && previous < token.range.end)
            })
        })
}

fn procedure_symbol_hover(
    source: &str,
    tokens: &[SpannedToken<'_>],
    index: usize,
) -> Option<Hover> {
    let object = object_declaration(tokens, index, "PROCEDURE")?;
    if word(tokens.get(object.keyword + 1)?, "SCOPED") {
        return None;
    }
    let name_range = procedure_name_range(tokens, object.keyword, object.end)?;
    if !name_range.contains(&index) {
        return None;
    }

    let name = compact_token_text(tokens, name_range.clone());
    let args = procedure_args(source, tokens, name_range.end, object.end)
        .unwrap_or_else(|| String::from(""));
    let returns = clause_after_keyword(
        source,
        tokens,
        object.keyword,
        object.end,
        "RETURNS",
        &[
            "LANGUAGE",
            "RUNTIME_VERSION",
            "PACKAGES",
            "IMPORTS",
            "HANDLER",
            "AS",
            "COMMENT",
            "EXECUTE",
        ],
    );
    let language = value_after_keyword(source, tokens, object.keyword, object.end, "LANGUAGE");
    let handler =
        routine_option_after_keyword(source, tokens, object.keyword, object.end, "HANDLER");
    let runtime = routine_option_after_keyword(
        source,
        tokens,
        object.keyword,
        object.end,
        "RUNTIME_VERSION",
    );
    let packages =
        routine_option_after_keyword(source, tokens, object.keyword, object.end, "PACKAGES");
    let imports =
        routine_option_after_keyword(source, tokens, object.keyword, object.end, "IMPORTS");
    let target_path =
        routine_option_after_keyword(source, tokens, object.keyword, object.end, "TARGET_PATH");

    let mut lines = vec![format!("Stored procedure `{name}`.")];
    if !args.is_empty() {
        lines.push(format!("Arguments: `{args}`."));
    }
    if let Some(returns) = returns {
        lines.push(format!("Returns: `{returns}`."));
    }
    if let Some(language) = language {
        lines.push(format!("Language: `{language}`."));
    }
    if let Some(handler) = handler {
        lines.push(format!("Handler: `{handler}`."));
    }
    if let Some(runtime) = runtime {
        lines.push(format!("Runtime: `{runtime}`."));
    }
    if let Some(packages) = packages {
        lines.push(format!("Packages: `{packages}`."));
    }
    if let Some(imports) = imports {
        lines.push(format!("Imports: `{imports}`."));
    }
    if let Some(target_path) = target_path {
        lines.push(format!("Target path: `{target_path}`."));
    }
    lines.push(String::from(
        "Snowflake resolves stored procedures by name plus argument types.",
    ));
    lines.push(String::from(
        "External-language procedures usually pair LANGUAGE with HANDLER, PACKAGES, IMPORTS, and RUNTIME_VERSION.",
    ));

    Some(Hover {
        kind: HoverKind::Procedure,
        title: format!("Stored procedure `{name}`"),
        body: lines.join("\n"),
        range: combined_range(tokens, name_range),
        docs_url: Some(CREATE_PROCEDURE_DOCS),
    })
}

fn task_symbol_hover(source: &str, tokens: &[SpannedToken<'_>], index: usize) -> Option<Hover> {
    let object = object_declaration(tokens, index, "TASK")?;
    let name_range = task_name_range(tokens, object.keyword, object.end)?;
    if !name_range.contains(&index) {
        return None;
    }

    let name = compact_token_text(tokens, name_range.clone());
    let warehouse = value_after_keyword(source, tokens, object.keyword, object.end, "WAREHOUSE")
        .or_else(|| {
            value_after_keyword(
                source,
                tokens,
                object.keyword,
                object.end,
                "USER_TASK_MANAGED_INITIAL_WAREHOUSE_SIZE",
            )
        });
    let schedule = value_after_keyword(source, tokens, object.keyword, object.end, "SCHEDULE");
    let after = clause_after_keyword(
        source,
        tokens,
        object.keyword,
        object.end,
        "AFTER",
        &["WHEN", "AS", "EXECUTE", "COMMENT", "FINALIZE"],
    );
    let when = clause_after_keyword(source, tokens, object.keyword, object.end, "WHEN", &["AS"]);

    let mut lines = vec![format!("Task `{name}`.")];
    if let Some(warehouse) = warehouse {
        lines.push(format!("Compute: `{warehouse}`."));
    }
    if let Some(schedule) = schedule {
        lines.push(format!("Schedule: `{schedule}`."));
    }
    if let Some(after) = after {
        lines.push(format!("Predecessors: `{after}`."));
    }
    if let Some(when) = when {
        lines.push(format!("Condition: `{when}`."));
    }
    lines.push(String::from(
        "Tasks run SQL on a schedule or after predecessor tasks; newly created tasks start suspended.",
    ));

    Some(Hover {
        kind: HoverKind::Task,
        title: format!("Task `{name}`"),
        body: lines.join("\n"),
        range: combined_range(tokens, name_range),
        docs_url: Some(CREATE_TASK_DOCS),
    })
}

#[derive(Clone, Copy)]
struct ObjectDeclaration {
    keyword: usize,
    end: usize,
}

fn object_declaration(
    tokens: &[SpannedToken<'_>],
    index: usize,
    object_keyword: &str,
) -> Option<ObjectDeclaration> {
    let (start, end) = statement_bounds(tokens, index);
    for keyword in (start..=index.min(end.saturating_sub(1))).rev() {
        if !word(&tokens[keyword], object_keyword) {
            continue;
        }
        if tokens[start..keyword]
            .iter()
            .any(|token| word(token, "CREATE"))
        {
            return Some(ObjectDeclaration { keyword, end });
        }
    }
    None
}

fn statement_bounds(tokens: &[SpannedToken<'_>], index: usize) -> (usize, usize) {
    let start = tokens[..index]
        .iter()
        .rposition(|token| token.kind == SyntaxKind::SEMICOLON)
        .map_or(0, |idx| idx + 1);
    let end = tokens[index..]
        .iter()
        .position(|token| token.kind == SyntaxKind::SEMICOLON)
        .map_or(tokens.len(), |relative| index + relative);
    (start, end)
}

fn procedure_name_range(
    tokens: &[SpannedToken<'_>],
    procedure_keyword: usize,
    end: usize,
) -> Option<Range<usize>> {
    let start = procedure_keyword + 1;
    let mut cursor = start;
    while cursor < end && tokens[cursor].kind != SyntaxKind::L_PAREN {
        if !is_name_part(&tokens[cursor]) {
            return None;
        }
        cursor += 1;
    }
    (start < cursor).then_some(start..cursor)
}

fn task_name_range(
    tokens: &[SpannedToken<'_>],
    task_keyword: usize,
    end: usize,
) -> Option<Range<usize>> {
    let mut start = task_keyword + 1;
    if word_seq(tokens, start, &["IF", "NOT", "EXISTS"]) {
        start += 3;
    }

    let mut cursor = start;
    while cursor < end && is_name_part(&tokens[cursor]) && !is_clause_boundary(&tokens[cursor]) {
        cursor += 1;
    }
    (start < cursor).then_some(start..cursor)
}

fn procedure_args(
    source: &str,
    tokens: &[SpannedToken<'_>],
    start: usize,
    end: usize,
) -> Option<String> {
    let open = (start..end).find(|&idx| tokens[idx].kind == SyntaxKind::L_PAREN)?;
    let close = matching_paren(tokens, open, end)?;
    let inside = token_slice(source, tokens, open + 1, close);
    Some(inside)
}

fn matching_paren(tokens: &[SpannedToken<'_>], open: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, token) in tokens.iter().enumerate().take(end).skip(open) {
        match token.kind {
            SyntaxKind::L_PAREN => depth += 1,
            SyntaxKind::R_PAREN => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn value_after_keyword(
    source: &str,
    tokens: &[SpannedToken<'_>],
    start: usize,
    end: usize,
    keyword: &str,
) -> Option<String> {
    let idx = (start..end).find(|&idx| word(&tokens[idx], keyword))?;
    let mut value_start = idx + 1;
    if value_start < end && tokens[value_start].kind == SyntaxKind::EQ {
        value_start += 1;
    }
    let mut value_end = value_start;
    while value_end < end
        && !is_clause_boundary(&tokens[value_end])
        && tokens[value_end].kind != SyntaxKind::COMMA
    {
        value_end += 1;
    }
    let value = token_slice(source, tokens, value_start, value_end);
    (!value.is_empty()).then_some(value)
}

fn routine_option_after_keyword(
    source: &str,
    tokens: &[SpannedToken<'_>],
    start: usize,
    end: usize,
    keyword: &str,
) -> Option<String> {
    clause_after_keyword(source, tokens, start, end, keyword, ROUTINE_OPTION_STOPS)
        .map(|value| value.strip_prefix("= ").unwrap_or(&value).to_string())
}

fn clause_after_keyword(
    source: &str,
    tokens: &[SpannedToken<'_>],
    start: usize,
    end: usize,
    keyword: &str,
    stops: &[&str],
) -> Option<String> {
    let idx = (start..end).find(|&idx| word(&tokens[idx], keyword))?;
    let value_start = idx + 1;
    let mut depth = 0usize;
    let mut value_end = value_start;
    while value_end < end {
        let token = &tokens[value_end];
        match token.kind {
            SyntaxKind::L_PAREN | SyntaxKind::L_BRACKET => depth += 1,
            SyntaxKind::R_PAREN | SyntaxKind::R_BRACKET => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && stops.iter().any(|stop| word(token, stop)) {
            break;
        }
        value_end += 1;
    }
    let value = token_slice(source, tokens, value_start, value_end);
    (!value.is_empty()).then_some(value)
}

fn token_slice(source: &str, tokens: &[SpannedToken<'_>], start: usize, end: usize) -> String {
    if start >= end || start >= tokens.len() {
        return String::new();
    }
    let end = end.min(tokens.len());
    compact_text(&source[tokens[start].range.start..tokens[end - 1].range.end])
}

fn compact_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compact_token_text(tokens: &[SpannedToken<'_>], range: Range<usize>) -> String {
    tokens[range]
        .iter()
        .map(|token| token.text)
        .collect::<Vec<_>>()
        .join("")
}

fn combined_range(tokens: &[SpannedToken<'_>], range: Range<usize>) -> Range<usize> {
    tokens[range.start].range.start..tokens[range.end - 1].range.end
}

fn is_name_part(token: &SpannedToken<'_>) -> bool {
    matches!(
        token.kind,
        SyntaxKind::IDENT | SyntaxKind::QUOTED_IDENT | SyntaxKind::DOT
    )
}

fn is_clause_boundary(token: &SpannedToken<'_>) -> bool {
    matches!(
        token.kind,
        SyntaxKind::SEMICOLON | SyntaxKind::R_PAREN | SyntaxKind::R_BRACKET
    ) || [
        "AS",
        "AFTER",
        "ARTIFACT_REPOSITORY",
        "CALLED",
        "COMMENT",
        "CONFIG",
        "COPY",
        "ERROR_INTEGRATION",
        "EXECUTE",
        "EXTERNAL_ACCESS_INTEGRATIONS",
        "FINALIZE",
        "HANDLER",
        "IMMUTABLE",
        "IMPORTS",
        "LANGUAGE",
        "MEMOIZABLE",
        "NULL",
        "OVERLAP_POLICY",
        "PACKAGES",
        "RETURNS",
        "RUNTIME_VERSION",
        "SCHEDULE",
        "SECRETS",
        "SECURE",
        "STRICT",
        "SUCCESS_INTEGRATION",
        "TASK_AUTO_RETRY_ATTEMPTS",
        "TARGET_PATH",
        "USER_TASK_MANAGED_INITIAL_WAREHOUSE_SIZE",
        "USER_TASK_TIMEOUT_MS",
        "VOLATILE",
        "WAREHOUSE",
        "WHEN",
    ]
    .iter()
    .any(|boundary| word(token, boundary))
}

fn word(token: &SpannedToken<'_>, expected: &str) -> bool {
    (token.kind == SyntaxKind::IDENT || token.kind.is_keyword())
        && token.text.eq_ignore_ascii_case(expected)
}

fn word_seq(tokens: &[SpannedToken<'_>], start: usize, words: &[&str]) -> bool {
    words.iter().enumerate().all(|(offset, expected)| {
        tokens
            .get(start + offset)
            .is_some_and(|t| word(t, expected))
    })
}

fn type_hover(token: &SpannedToken<'_>) -> Option<Hover> {
    let (canonical, body) = type_info(token.text)?;
    Some(Hover {
        kind: HoverKind::Type,
        title: format!("Snowflake type `{canonical}`"),
        body: body.to_string(),
        range: token.range.clone(),
        docs_url: Some(DATA_TYPES_DOCS),
    })
}

fn type_info(text: &str) -> Option<(&'static str, &'static str)> {
    let item = [
        (
            &["NUMBER", "DECIMAL", "NUMERIC", "INT", "INTEGER", "BIGINT"][..],
            "NUMBER",
            "Exact fixed-point numeric type. Use precision and scale for stable financial or identifier-like values.",
        ),
        (
            &["FLOAT", "DOUBLE", "REAL"][..],
            "FLOAT",
            "Approximate floating-point numeric type. Prefer NUMBER when exact decimal behavior matters.",
        ),
        (
            &["VARCHAR", "STRING", "TEXT", "CHAR"][..],
            "VARCHAR",
            "Variable-length character data. STRING and TEXT are Snowflake aliases for VARCHAR.",
        ),
        (&["BOOLEAN"][..], "BOOLEAN", "TRUE/FALSE logical value."),
        (
            &["VARIANT"][..],
            "VARIANT",
            "Semi-structured value that can hold JSON-like OBJECT, ARRAY, scalar, or SQL NULL distinctions.",
        ),
        (
            &["OBJECT"][..],
            "OBJECT",
            "Semi-structured key/value object. Common with JSON ingestion and colon path access.",
        ),
        (
            &["ARRAY"][..],
            "ARRAY",
            "Semi-structured ordered collection. Use bracket indexing and FLATTEN/TABLE for traversal.",
        ),
        (
            &["MAP"][..],
            "MAP",
            "Structured key/value collection type. Useful when key and value types are part of the schema.",
        ),
        (
            &["VECTOR"][..],
            "VECTOR",
            "Vector type for embeddings and similarity workloads; keep element type and dimension explicit.",
        ),
        (
            &["DATE"][..],
            "DATE",
            "Calendar date without time of day.",
        ),
        (
            &["TIME"][..],
            "TIME",
            "Time of day without date.",
        ),
        (
            &["TIMESTAMP", "TIMESTAMP_NTZ", "TIMESTAMP_LTZ", "TIMESTAMP_TZ"][..],
            "TIMESTAMP",
            "Timestamp family. NTZ has no time zone, LTZ uses the session time zone, and TZ stores an explicit offset.",
        ),
        (&["BINARY"][..], "BINARY", "Variable-length binary data."),
        (
            &["GEOGRAPHY", "GEOMETRY"][..],
            "GEOSPATIAL",
            "Geospatial data for spherical geography or planar geometry operations.",
        ),
    ]
    .into_iter()
    .find(|(aliases, _, _)| aliases.iter().any(|alias| text.eq_ignore_ascii_case(alias)))?;

    Some((item.1, item.2))
}

fn language_hover(token: &SpannedToken<'_>) -> Option<Hover> {
    let template = if token.text.eq_ignore_ascii_case("JAVASCRIPT") {
        StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE JAVASCRIPT",
            body: "JavaScript stored procedure body. The handler is the body itself; SQL argument names can need careful case handling inside JavaScript.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        }
    } else if token.text.eq_ignore_ascii_case("PYTHON") {
        StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE PYTHON",
            body: "Snowpark Python stored procedure. HANDLER names the Python function; RUNTIME_VERSION pins Python; PACKAGES, IMPORTS, EXTERNAL_ACCESS_INTEGRATIONS, and SECRETS describe the runtime environment.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        }
    } else if token.text.eq_ignore_ascii_case("JAVA") {
        StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE JAVA",
            body: "Java stored procedure. HANDLER names a class and method; RUNTIME_VERSION, PACKAGES, IMPORTS, and TARGET_PATH describe the JVM runtime and staged artifact.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        }
    } else if token.text.eq_ignore_ascii_case("SCALA") {
        StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE SCALA",
            body: "Snowpark Scala stored procedure. HANDLER names the Scala entry point; RUNTIME_VERSION, PACKAGES, IMPORTS, and TARGET_PATH describe the JVM runtime and staged artifact.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        }
    } else if token.text.eq_ignore_ascii_case("SQL") {
        StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE SQL",
            body: "Snowflake Scripting stored procedure body. Use SQL procedural constructs such as DECLARE, BEGIN, RETURN, loops, and EXCEPTION handlers.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        }
    } else {
        return None;
    };
    Some(from_static(token.range.clone(), template))
}

fn property_hover(token: &SpannedToken<'_>) -> Option<Hover> {
    let properties = [
        StaticHover {
            kind: HoverKind::Property,
            title: "RETURNS",
            body: "Declares a procedure result type. It can be a scalar type or RETURNS TABLE(...), depending on the procedure language and support level.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "LANGUAGE",
            body: "Selects the stored procedure handler language: SQL, JAVASCRIPT, PYTHON, JAVA, or SCALA.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "HANDLER",
            body: "Names the external-language entry point, such as a Python function or JVM method.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "PACKAGES",
            body: "Declares runtime packages available to Snowpark Java, Scala, or Python procedure handlers.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "IMPORTS",
            body: "Adds staged files that the stored procedure handler can read at runtime.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "RUNTIME_VERSION",
            body: "Pins the language runtime version for Java, Python, or Scala procedure handlers.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "TARGET_PATH",
            body: "Stage path for the compiled Java or Scala procedure artifact. Use it when Snowflake should write the generated handler artifact to a stage.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "ARTIFACT_REPOSITORY",
            body: "Selects an artifact repository for resolving supported external-language dependencies.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "EXTERNAL_ACCESS_INTEGRATIONS",
            body: "Allows an external-language procedure to use one or more external access integrations for outbound network access.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "SECRETS",
            body: "Maps Snowflake secrets to names the external-language handler can use at runtime.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "STRICT",
            body: "Alias for RETURNS NULL ON NULL INPUT: Snowflake does not call the handler when any input argument is NULL.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "CALLED",
            body: "Part of CALLED ON NULL INPUT: Snowflake calls the procedure even when an input argument is NULL.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "IMMUTABLE",
            body: "Declares that the result depends only on inputs and not on database state or side effects.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "VOLATILE",
            body: "Declares that the procedure can depend on state or side effects; this is the conservative behavior for procedural code.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "EXECUTE",
            body: "Controls whether a procedure or task runs with owner, caller, restricted caller, or user execution context.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "WAREHOUSE",
            body: "Virtual warehouse that supplies compute for a task run. Omit it only when using serverless task sizing.",
            docs_url: Some(CREATE_TASK_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "USER_TASK_MANAGED_INITIAL_WAREHOUSE_SIZE",
            body: "Initial serverless task size. Snowflake can manage task compute after enough run history exists.",
            docs_url: Some(CREATE_TASK_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "SCHEDULE",
            body: "Task schedule. Snowflake accepts interval strings or USING CRON expressions with a time zone.",
            docs_url: Some(CREATE_TASK_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "AFTER",
            body: "Creates a task graph dependency; this task runs after one or more predecessor tasks.",
            docs_url: Some(CREATE_TASK_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "WHEN",
            body: "Boolean task condition evaluated before the task body runs. Common with stream checks such as SYSTEM$STREAM_HAS_DATA.",
            docs_url: Some(CREATE_TASK_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "FINALIZE",
            body: "Marks a task as a finalizer for a task graph root.",
            docs_url: Some(CREATE_TASK_DOCS),
        },
        StaticHover {
            kind: HoverKind::Property,
            title: "TASK_AUTO_RETRY_ATTEMPTS",
            body: "Configures automatic retries for failed task graph runs.",
            docs_url: Some(CREATE_TASK_DOCS),
        },
    ];

    properties
        .into_iter()
        .find(|property| token.text.eq_ignore_ascii_case(property.title))
        .map(|template| from_static(token.range.clone(), template))
}

fn keyword_hover(token: &SpannedToken<'_>) -> Option<Hover> {
    let template = if token.text.eq_ignore_ascii_case("PROCEDURE") {
        StaticHover {
            kind: HoverKind::Procedure,
            title: "Stored procedure",
            body: "Schema object that can be called with CALL. Procedures support SQL, JavaScript, Python, Java, and Scala handlers.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        }
    } else if token.text.eq_ignore_ascii_case("TASK") {
        StaticHover {
            kind: HoverKind::Task,
            title: "Task",
            body: "Schema object that executes SQL on a schedule or as part of a task graph. Newly created tasks start suspended.",
            docs_url: Some(CREATE_TASK_DOCS),
        }
    } else {
        return None;
    };
    Some(from_static(token.range.clone(), template))
}

fn from_static(range: Range<usize>, template: StaticHover) -> Hover {
    Hover {
        kind: template.kind,
        title: template.title.to_string(),
        body: template.body.to_string(),
        range,
        docs_url: template.docs_url,
    }
}
