use crate::HoverKind;

pub const CREATE_PROCEDURE_DOCS: &str =
    "https://docs.snowflake.com/en/sql-reference/sql/create-procedure";
pub const CREATE_TASK_DOCS: &str = "https://docs.snowflake.com/en/sql-reference/sql/create-task";
pub const DATA_TYPES_DOCS: &str = "https://docs.snowflake.com/en/sql-reference/data-types";

pub(crate) const ROUTINE_OPTION_STOPS: &[&str] = &[
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

#[derive(Clone, Copy)]
pub(crate) struct StaticHover {
    pub(crate) kind: HoverKind,
    pub(crate) title: &'static str,
    pub(crate) body: &'static str,
    pub(crate) docs_url: Option<&'static str>,
}

pub(crate) fn type_info(text: &str) -> Option<(&'static str, &'static str)> {
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

pub(crate) fn language_template(text: &str) -> Option<StaticHover> {
    if text.eq_ignore_ascii_case("JAVASCRIPT") {
        Some(StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE JAVASCRIPT",
            body: "JavaScript stored procedure body. The handler is the body itself; SQL argument names can need careful case handling inside JavaScript.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        })
    } else if text.eq_ignore_ascii_case("PYTHON") {
        Some(StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE PYTHON",
            body: "Snowpark Python stored procedure. HANDLER names the Python function; RUNTIME_VERSION pins Python; PACKAGES, IMPORTS, EXTERNAL_ACCESS_INTEGRATIONS, and SECRETS describe the runtime environment.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        })
    } else if text.eq_ignore_ascii_case("JAVA") {
        Some(StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE JAVA",
            body: "Java stored procedure. HANDLER names a class and method; RUNTIME_VERSION, PACKAGES, IMPORTS, and TARGET_PATH describe the JVM runtime and staged artifact.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        })
    } else if text.eq_ignore_ascii_case("SCALA") {
        Some(StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE SCALA",
            body: "Snowpark Scala stored procedure. HANDLER names the Scala entry point; RUNTIME_VERSION, PACKAGES, IMPORTS, and TARGET_PATH describe the JVM runtime and staged artifact.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        })
    } else if text.eq_ignore_ascii_case("SQL") {
        Some(StaticHover {
            kind: HoverKind::Language,
            title: "LANGUAGE SQL",
            body: "Snowflake Scripting stored procedure body. Use SQL procedural constructs such as DECLARE, BEGIN, RETURN, loops, and EXCEPTION handlers.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        })
    } else {
        None
    }
}

pub(crate) fn property_template(text: &str) -> Option<StaticHover> {
    PROPERTIES
        .iter()
        .copied()
        .find(|property| text.eq_ignore_ascii_case(property.title))
}

pub(crate) fn keyword_template(text: &str) -> Option<StaticHover> {
    if text.eq_ignore_ascii_case("PROCEDURE") {
        Some(StaticHover {
            kind: HoverKind::Procedure,
            title: "Stored procedure",
            body: "Schema object that can be called with CALL. Procedures support SQL, JavaScript, Python, Java, and Scala handlers.",
            docs_url: Some(CREATE_PROCEDURE_DOCS),
        })
    } else if text.eq_ignore_ascii_case("TASK") {
        Some(StaticHover {
            kind: HoverKind::Task,
            title: "Task",
            body: "Schema object that executes SQL on a schedule or as part of a task graph. Newly created tasks start suspended.",
            docs_url: Some(CREATE_TASK_DOCS),
        })
    } else {
        None
    }
}

const PROPERTIES: &[StaticHover] = &[
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
