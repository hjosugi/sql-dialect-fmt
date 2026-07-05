//! The Snowflake SQL grammar.
//!
//! Phase 1 covered a single `SELECT` plus a Pratt expression parser. Phase 2 grows this toward
//! "parse the most common queries completely": all single-`SELECT` clauses, `JOIN`s, subqueries
//! and derived tables, set operations, CTEs, and the compound predicates (`IS [NOT] NULL`,
//! `[NOT] IN/BETWEEN/LIKE`).
//!
//! Every rule is total: on unexpected input it records a diagnostic and recovers, never panics.

use sql_dialect_fmt_syntax::SyntaxKind;
use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{CompletedMarker, ContextualKeyword, Parser};

mod delta;
mod dml;
mod stmt;

// Binding powers for the Pratt parser. Higher binds tighter; (left, right) for infix.
const BP_OR: (u8, u8) = (1, 2);
const BP_AND: (u8, u8) = (3, 4);
const BP_CMP: (u8, u8) = (7, 8);
const BP_CONCAT: (u8, u8) = (9, 10);
const BP_ADD: (u8, u8) = (11, 12);
const BP_MUL: (u8, u8) = (13, 14);
const BP_PREFIX_NOT: u8 = 6; // looser than comparison: `NOT a = b` == `NOT (a = b)`
const BP_PREFIX_NEG: u8 = 15; // unary +/- bind tighter than `*`

const INTERVAL_UNIT_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Year,
    ContextualKeyword::Years,
    ContextualKeyword::Month,
    ContextualKeyword::Months,
    ContextualKeyword::Week,
    ContextualKeyword::Weeks,
    ContextualKeyword::Day,
    ContextualKeyword::Days,
    ContextualKeyword::Hour,
    ContextualKeyword::Hours,
    ContextualKeyword::Minute,
    ContextualKeyword::Minutes,
    ContextualKeyword::Second,
    ContextualKeyword::Seconds,
    ContextualKeyword::Millisecond,
    ContextualKeyword::Milliseconds,
    ContextualKeyword::Microsecond,
    ContextualKeyword::Microseconds,
];

const CREATE_MODIFIER_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Materialized,
    ContextualKeyword::Local,
    ContextualKeyword::Global,
];

const NAMED_OBJECT_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Schema,
    ContextualKeyword::Database,
    ContextualKeyword::Stage,
    ContextualKeyword::Sequence,
    ContextualKeyword::Stream,
    ContextualKeyword::Dynamic,
    ContextualKeyword::Semantic,
    ContextualKeyword::File,
    ContextualKeyword::Tag,
];

const SEMANTIC_VIEW_CLAUSE_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Tables,
    ContextualKeyword::Relationships,
    ContextualKeyword::Facts,
    ContextualKeyword::Dimensions,
    ContextualKeyword::Metrics,
    ContextualKeyword::AiSqlGeneration,
    ContextualKeyword::AiQuestionCategorization,
    ContextualKeyword::AiVerifiedQueries,
];

const SEMANTIC_VIEW_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Tables,
    ContextualKeyword::Relationships,
    ContextualKeyword::Facts,
    ContextualKeyword::Dimensions,
    ContextualKeyword::Metrics,
    ContextualKeyword::Public,
    ContextualKeyword::Private,
    ContextualKeyword::Primary,
    ContextualKeyword::Key,
    ContextualKeyword::References,
    ContextualKeyword::Synonyms,
    ContextualKeyword::Labels,
    ContextualKeyword::AiSqlGeneration,
    ContextualKeyword::AiQuestionCategorization,
    ContextualKeyword::AiVerifiedQueries,
    ContextualKeyword::Question,
    ContextualKeyword::VerifiedAt,
    ContextualKeyword::OnboardingQuestion,
    ContextualKeyword::VerifiedBy,
    ContextualKeyword::Tag,
];

const DDL_CONSTRAINT_START_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Constraint,
    ContextualKeyword::Primary,
    ContextualKeyword::Unique,
    ContextualKeyword::Foreign,
    ContextualKeyword::Check,
];

const DDL_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Default,
    ContextualKeyword::Primary,
    ContextualKeyword::Key,
    ContextualKeyword::Unique,
    ContextualKeyword::Foreign,
    ContextualKeyword::References,
    ContextualKeyword::Constraint,
    ContextualKeyword::Check,
    ContextualKeyword::Collate,
    ContextualKeyword::Comment,
    ContextualKeyword::Cluster,
    ContextualKeyword::Clone,
    ContextualKeyword::Cascade,
    ContextualKeyword::Restrict,
    ContextualKeyword::Materialized,
    ContextualKeyword::Masking,
    ContextualKeyword::Policy,
    ContextualKeyword::Access,
    ContextualKeyword::Tag,
    ContextualKeyword::AllowedValues,
    ContextualKeyword::Propagate,
    ContextualKeyword::ExemptOtherPolicies,
];

const OBJECT_TYPE_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Schema,
    ContextualKeyword::Database,
    ContextualKeyword::Stage,
    ContextualKeyword::Sequence,
    ContextualKeyword::Stream,
];

const GRANTEE_KIND_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Role,
    ContextualKeyword::User,
    ContextualKeyword::Share,
    ContextualKeyword::Database,
];

const GRANT_TAIL_CONTEXTUAL_WORDS: &[ContextualKeyword] = &[
    ContextualKeyword::Option,
    ContextualKeyword::Cascade,
    ContextualKeyword::Restrict,
];

const GRANT_TAIL_START_CONTEXTUAL_WORDS: &[ContextualKeyword] =
    &[ContextualKeyword::Cascade, ContextualKeyword::Restrict];

// ---- top level ----

pub(crate) fn source_file(p: &mut Parser) {
    let m = p.start();
    while !p.at_eof() {
        if p.at(SEMICOLON) {
            p.bump(SEMICOLON); // statement separator / empty statement
        } else if stmt::at_stmt_start(p) {
            stmt::statement_or_flow(p);
        } else {
            p.err_and_bump("expected a statement");
        }
    }
    m.complete(p, SOURCE_FILE);
}

/// A top-level statement ends at `;`, EOF, or the flow operator `->>` that chains it to the next
/// statement. Lenient statement parsers consult this so `->>` is left for `stmt::statement_or_flow`
/// instead of being swallowed into the preceding flat token run.
fn at_stmt_terminator(p: &Parser) -> bool {
    p.at(SEMICOLON) || (p.dialect().supports_flow_operator() && p.at(FLOW_PIPE)) || p.at_eof()
}

// ---- DDL (Phase 7) ----

/// `IF [NOT] EXISTS`, tolerated wherever Snowflake allows it.
fn if_exists_clause(p: &mut Parser) {
    if p.at(IF_KW) {
        p.bump(IF_KW);
        p.eat(NOT_KW);
        p.eat(EXISTS_KW);
    }
}

fn create_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(CREATE_KW);
    if p.at(OR_KW) {
        p.bump(OR_KW);
        if p.at(REPLACE_KW) {
            p.bump(REPLACE_KW);
        } else {
            p.expect(ALTER_KW);
        }
    }
    // Modifiers before the object kind (SECURE / TEMPORARY / TRANSIENT / MATERIALIZED / ...). Stop
    // at the object-kind word so the right sub-rule sees it; also stop at a query/body `AS` and the
    // statement end so a malformed prefix can never run away.
    while !at_object_kind(p) && !at_stmt_terminator(p) && !at_create_body(p) {
        // Contextual modifier words (`MATERIALIZED`, `LOCAL`, `GLOBAL`) precede the object kind;
        // up-case them like keywords. Reserved modifiers (SECURE/TEMP/TRANSIENT/…) up-case already.
        if p.nth_any_contextual(0, CREATE_MODIFIER_CONTEXTUAL_WORDS) {
            p.bump_as(CONTEXTUAL_KEYWORD);
        } else {
            p.bump_any();
        }
    }
    if p.at(VIEW_KW) {
        create_view(p);
    } else if p.at(TABLE_KW) {
        create_table(p);
    } else if p.at(PROCEDURE_KW) || p.at(FUNCTION_KW) {
        create_routine(p);
    } else if at_policy_object_kind(p) {
        create_policy(p);
    } else if at_named_object_kind(p) {
        create_object(p);
    } else {
        create_other(p);
    }
    m.complete(p, CREATE_STMT);
}

/// At the keyword/word that names the kind of object being created: a reserved object keyword
/// (`TABLE`, `VIEW`, `PROCEDURE`, `FUNCTION`, `TASK`, `WAREHOUSE`) or one of the contextual object
/// words (`SCHEMA`, `DATABASE`, `STAGE`, `SEQUENCE`, `STREAM`, `DYNAMIC` table, `FILE` format).
fn at_object_kind(p: &Parser) -> bool {
    p.at(TABLE_KW)
        || p.at(VIEW_KW)
        || p.at(PROCEDURE_KW)
        || p.at(FUNCTION_KW)
        || at_policy_object_kind(p)
        || at_named_object_kind(p)
}

/// At a Phase-7 "named object" kind — the property-region creates this rule structures
/// (`SCHEMA`/`DATABASE`/`WAREHOUSE`/`STAGE`/`SEQUENCE`/`STREAM`/`TASK`/`DYNAMIC TABLE`/
/// `SEMANTIC VIEW`/`FILE FORMAT`).
fn at_named_object_kind(p: &Parser) -> bool {
    p.at(TASK_KW) || p.at(WAREHOUSE_KW) || p.nth_any_contextual(0, NAMED_OBJECT_CONTEXTUAL_WORDS)
}

/// `CREATE MASKING POLICY` / `CREATE ROW ACCESS POLICY`.
fn at_policy_object_kind(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::Masking)
        && p.nth_contextual(1, ContextualKeyword::Policy))
        || (p.at(ROW_KW)
            && p.nth_contextual(1, ContextualKeyword::Access)
            && p.nth_contextual(2, ContextualKeyword::Policy))
}

/// `CREATE [OR REPLACE] [modifiers] <kind> [IF NOT EXISTS] <name> <property>* [<clause>]* [AS <body>]`
/// for the object kinds whose body is a property region rather than a column list:
/// SCHEMA / DATABASE / WAREHOUSE / STAGE / SEQUENCE / FILE FORMAT, plus the body-bearing
/// STREAM (`ON TABLE …`), TASK (`SCHEDULE = … AFTER … AS <sql>`), and DYNAMIC TABLE
/// (`TARGET_LAG = … WAREHOUSE = … AS <query>`).
///
/// The object-kind word and any optional column list are kept inline on the `CREATE_STMT` header;
/// each property (`KEY = value`, `KEY = ( … )`, or a bare flag word) becomes an [`OBJECT_PROPERTY`],
/// the stream source a [`STREAM_SOURCE`], a task predecessor list a [`TASK_AFTER`], and the
/// `AS <body>` is parsed structurally so it lays out like any other query/statement.
fn create_object(p: &mut Parser) {
    // The object-kind word(s): one word, or the two-word `FILE FORMAT` / `DYNAMIC TABLE`.
    object_kind_words(p);
    if_exists_clause(p);
    if p.at_name() {
        name_ref(p);
    }
    // A few object kinds carry a column list (DYNAMIC TABLE, sometimes STREAM); keep it inline.
    if p.at(L_PAREN) {
        column_def_list(p);
    }
    p.eat(WITH_KW); // optional `WITH` before the property region (e.g. CREATE WAREHOUSE w WITH …)
    object_property_region(p);
    if at_create_body(p) {
        p.bump(AS_KW);
        create_body(p);
    }
}

/// Consume the object-kind word(s): the two-word kinds `FILE FORMAT` and `DYNAMIC TABLE`, otherwise
/// a single word. `FILE`/`DYNAMIC` are contextual, so they round-trip and the formatter up-cases the
/// whole kind via the [`OBJECT_PROPERTY`]-free header walk.
fn object_kind_words(p: &mut Parser) {
    if p.nth_contextual(0, ContextualKeyword::File) {
        p.bump_as(CONTEXTUAL_KEYWORD); // FILE
        if p.nth_contextual(0, ContextualKeyword::Format) {
            p.bump_as(CONTEXTUAL_KEYWORD); // FORMAT
        }
    } else if p.nth_contextual(0, ContextualKeyword::Dynamic) {
        p.bump_as(CONTEXTUAL_KEYWORD); // DYNAMIC
        if p.at(TABLE_KW) {
            p.bump(TABLE_KW);
        }
    } else if p.nth_contextual(0, ContextualKeyword::Semantic) {
        p.bump_as(CONTEXTUAL_KEYWORD); // SEMANTIC
        if p.at(VIEW_KW) {
            p.bump(VIEW_KW);
        }
    } else if p.nth_contextual(0, ContextualKeyword::Tag) {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else if p.at_keyword() {
        // A reserved object keyword (TASK / WAREHOUSE / TABLE / …) — already up-cased by bump_any.
        p.bump_any();
    } else {
        // SCHEMA / DATABASE / STAGE / SEQUENCE / STREAM — contextual words, up-cased like keywords.
        p.bump_as(CONTEXTUAL_KEYWORD);
    }
}

/// `CREATE [OR REPLACE|OR ALTER] {MASKING POLICY|ROW ACCESS POLICY} <name>
/// AS (<arg> <type>[, ...]) RETURNS <type> -> <expr> [options...]`.
///
/// The signature and policy expression surface is intentionally kept as an inline token run: it is
/// expression-like but open-ended, and semicolon-free in valid Snowflake DDL. This gets clean
/// parsing, keyword casing, and lossless formatting without pretending to fully type-check the
/// policy language.
fn create_policy(p: &mut Parser) {
    policy_kind_words(p);
    if_exists_clause(p);
    if p.at_name() {
        name_ref(p);
    }
    while !at_stmt_terminator(p) {
        bump_ddl_word(p);
    }
}

fn policy_kind_words(p: &mut Parser) {
    if p.nth_contextual(0, ContextualKeyword::Masking) {
        p.bump_as(CONTEXTUAL_KEYWORD); // MASKING
        if p.nth_contextual(0, ContextualKeyword::Policy) {
            p.bump_as(CONTEXTUAL_KEYWORD); // POLICY
        }
    } else {
        p.bump(ROW_KW);
        if p.nth_contextual(0, ContextualKeyword::Access) {
            p.bump_as(CONTEXTUAL_KEYWORD); // ACCESS
        }
        if p.nth_contextual(0, ContextualKeyword::Policy) {
            p.bump_as(CONTEXTUAL_KEYWORD); // POLICY
        }
    }
}

/// The defensive property/clause region of an object DDL, terminated by an `AS <body>`, `;`, or EOF.
/// Each iteration must make progress: every branch bumps at least one token, and the catch-all bumps
/// the current token into an [`OBJECT_PROPERTY`] so a surprise token can never stall the loop.
fn object_property_region(p: &mut Parser) {
    while !at_create_body(p) && !at_stmt_terminator(p) {
        if p.dialect().supports_semantic_view() && at_semantic_view_clause_start(p) {
            semantic_view_clause(p);
        } else if p.at(ON_KW) {
            stream_source(p);
        } else if p.at(AFTER_KW) {
            task_after(p);
        } else if p.at(WHEN_KW) {
            // CREATE TASK … WHEN <boolean_expr>.
            let m = p.start();
            p.bump(WHEN_KW);
            expr(p);
            m.complete(p, OBJECT_PROPERTY);
        } else {
            object_property(p);
        }
    }
}

/// A top-level `CREATE SEMANTIC VIEW` clause. The Snowflake surface is wide and evolving:
/// `TABLES (...)`, `RELATIONSHIPS (...)`, `FACTS (...)`, `DIMENSIONS (...)`, `METRICS (...)`,
/// AI instruction clauses, `WITH TAG (...)`, and `COPY GRANTS`. We structure the top-level clause
/// and the outer comma-separated list items while keeping each item body lossless.
fn semantic_view_clause(p: &mut Parser) {
    let m = p.start();
    if p.at(WITH_KW) {
        p.bump(WITH_KW);
        if p.nth_contextual(0, ContextualKeyword::Tag) {
            p.bump_as(CONTEXTUAL_KEYWORD);
        }
        if p.at(L_PAREN) {
            semantic_view_paren_list(p);
        }
    } else if p.at(COPY_KW) {
        p.bump(COPY_KW);
        p.eat(GRANTS_KW);
    } else {
        semantic_view_word(p);
        if p.at(L_PAREN) {
            semantic_view_paren_list(p);
        } else if !at_semantic_view_clause_start(p)
            && !p.at(SEMICOLON)
            && !p.at_eof()
            && !at_create_body(p)
        {
            // `AI_SQL_GENERATION '<instruction>'` and
            // `AI_QUESTION_CATEGORIZATION '<instruction>'` carry a single literal value.
            semantic_view_word(p);
        }
    }
    m.complete(p, SEMANTIC_VIEW_CLAUSE);
}

fn at_semantic_view_clause_start(p: &Parser) -> bool {
    p.nth_any_contextual(0, SEMANTIC_VIEW_CLAUSE_CONTEXTUAL_WORDS)
        || (p.at(WITH_KW) && p.nth_contextual(1, ContextualKeyword::Tag))
        || (p.at(COPY_KW) && p.nth_at(1, GRANTS_KW))
}

fn semantic_view_paren_list(p: &mut Parser) {
    p.bump(L_PAREN);
    if !p.at(R_PAREN) {
        semantic_view_item(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            semantic_view_item(p);
        }
    }
    p.expect(R_PAREN);
}

fn semantic_view_item(p: &mut Parser) {
    let m = p.start();
    balanced_token_run_until(p, |p| p.at(COMMA) || p.at(R_PAREN), semantic_view_word);
    m.complete(p, SEMANTIC_VIEW_ITEM);
}

fn semantic_view_word(p: &mut Parser) {
    if is_semantic_view_contextual_word(p) {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else {
        p.bump_any();
    }
}

fn is_semantic_view_contextual_word(p: &Parser) -> bool {
    p.nth_any_contextual(0, SEMANTIC_VIEW_CONTEXTUAL_WORDS)
}

/// One object property: `KEY = value`, `KEY = ( … )`, the unset/no-prefixed flags (`NOORDER`), or a
/// bare flag word. Always consumes at least one token.
fn object_property(p: &mut Parser) {
    let m = p.start();
    let is_allowed_values = p.nth_contextual(0, ContextualKeyword::AllowedValues);
    if is_allowed_values {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else {
        p.bump_any(); // the property name / flag word (kept verbatim; values are case-sensitive)
    }
    if p.eat(EQ) {
        if p.at(L_PAREN) {
            balanced_parens(p); // KEY = ( sub-option = value, … ) — e.g. FILE_FORMAT = (TYPE = 'CSV')
        } else if !at_create_body(p) && !at_stmt_terminator(p) {
            p.bump_any(); // a single literal / bare-word / @stage value
        }
        // Some values carry trailing units as separate words: `START WITH 1`, `INCREMENT BY 1`.
        while at_property_value_tail(p) {
            p.bump_any();
        }
    } else if is_allowed_values {
        while at_allowed_values_tail(p) {
            p.bump_any();
        }
    } else {
        // Bare option words like `START WITH 1` / `INCREMENT BY 1` where the `=` is omitted, or a
        // standalone flag (`NOORDER`). Absorb the immediate value tail so it stays on one line.
        while at_property_value_tail(p) {
            p.bump_any();
        }
    }
    m.complete(p, OBJECT_PROPERTY);
}

/// Continuation of `CREATE TAG ... ALLOWED_VALUES 'a', 'b', ...`.
fn at_allowed_values_tail(p: &Parser) -> bool {
    if p.at(COMMA) {
        return true;
    }
    let starts_new_property = p.nth_at(1, EQ);
    !starts_new_property
        && !p.at(SEMICOLON)
        && !p.at(ON_KW)
        && !p.at(AFTER_KW)
        && !p.at(WHEN_KW)
        && !at_create_body(p)
        && !p.at_eof()
        && (p.at(STRING) || p.at(INT_NUMBER) || p.at(FLOAT_NUMBER))
}

/// A continuation token of the current property's value: a `WITH`/`BY` connector or a literal/word
/// that is not the start of the next property, a clause, or the body. Keeps `START WITH 1` and
/// `INCREMENT BY 1` (the `=`-less sequence forms) together on one line.
fn at_property_value_tail(p: &Parser) -> bool {
    if p.at(WITH_KW) || p.at(BY_KW) {
        return true;
    }
    // A bare value word/literal that is not itself a new `KEY = …` property and not a clause/body.
    let starts_new_property = p.nth_at(1, EQ);
    !starts_new_property
        && !p.at(SEMICOLON)
        && !p.at(ON_KW)
        && !p.at(AFTER_KW)
        && !p.at(WHEN_KW)
        && !at_create_body(p)
        && !p.at_eof()
        && (p.at(INT_NUMBER) || p.at(FLOAT_NUMBER) || p.at(STRING) || p.at(VARIABLE))
}

/// A stream's `ON { TABLE | VIEW | STAGE } <name> [ { AT | BEFORE } ( … ) ]` source clause.
fn stream_source(p: &mut Parser) {
    let m = p.start();
    p.bump(ON_KW);
    // The source object kind word (TABLE / VIEW / STAGE / EXTERNAL TABLE …) then the name.
    if p.at(TABLE_KW) || p.at(VIEW_KW) {
        p.bump_any();
    } else if p.nth_contextual(0, ContextualKeyword::Stage) {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else if p.at_keyword() {
        p.bump_any();
    }
    if p.at_name() {
        name_ref(p);
    }
    // Optional time-travel: `AT ( … )` / `BEFORE ( … )`.
    if p.nth_contextual(0, ContextualKeyword::At) || p.nth_contextual(0, ContextualKeyword::Before)
    {
        p.bump_as(CONTEXTUAL_KEYWORD);
        if p.at(L_PAREN) {
            balanced_parens(p);
        }
    }
    m.complete(p, STREAM_SOURCE);
}

/// A task's `AFTER <pred> [, <pred>]*` predecessor list.
fn task_after(p: &mut Parser) {
    let m = p.start();
    p.bump(AFTER_KW);
    if p.at_name() {
        name_ref(p);
        while p.eat(COMMA) {
            if p.at_name() {
                name_ref(p);
            } else {
                break;
            }
        }
    }
    m.complete(p, TASK_AFTER);
}

/// The `<body>` after `AS` in a body-bearing object DDL: a query (TASK over a SELECT, DYNAMIC TABLE),
/// a DML statement (TASK), a scripting block, or a parenthesized query. Parsed structurally so it
/// lays out like a standalone statement.
fn create_body(p: &mut Parser) {
    if p.at(SELECT_KW)
        || p.at(WITH_KW)
        || p.at(VALUES_KW)
        || (p.at(L_PAREN) && (p.nth_at(1, SELECT_KW) || p.nth_at(1, WITH_KW)))
    {
        query_expr(p);
    } else if p.at(INSERT_KW) {
        dml::insert_stmt(p);
    } else if p.at(UPDATE_KW) {
        dml::update_stmt(p);
    } else if p.at(DELETE_KW) {
        dml::delete_stmt(p);
    } else if p.at(MERGE_KW) {
        dml::merge_stmt(p);
    } else if p.at(CALL_KW) {
        call_stmt(p);
    } else if at_block_start(p) {
        block_stmt(p);
    } else if !at_stmt_terminator(p) {
        // An unmodeled body shape: keep it as an expression statement so it still round-trips.
        let m = p.start();
        expr(p);
        m.complete(p, EXPR_STMT);
    }
}

/// Object kinds without a query body that this rule does not specialize — parsed leniently as a flat
/// token run so they round-trip and get inline spacing (like [`alter_stmt`]).
fn create_other(p: &mut Parser) {
    while !at_stmt_terminator(p) {
        if at_create_body(p) {
            p.error("CREATE ... AS <body> is not yet formatted; left verbatim");
            while !at_stmt_terminator(p) {
                p.bump_any();
            }
            return;
        }
        p.bump_any();
    }
}

// ---- access control: GRANT / REVOKE ----

/// `GRANT <privileges> ON <object> TO [ROLE|USER] <name> [WITH GRANT OPTION]`, plus the role/share
/// grant shapes (`GRANT ROLE r TO …`, `GRANT <role> TO USER u`). The privilege list, the `ON …`
/// securable, and the `TO …` grantee are each their own node so the formatter can stack them; a
/// trailing `WITH GRANT OPTION` (or any unmodeled tail) is kept as inline tokens.
fn grant_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(GRANT_KW);
    priv_list(p, |p| p.at(ON_KW) || at_to(p));
    if p.at(ON_KW) {
        grant_target(p);
    }
    if at_to(p) {
        grantee(p, GranteeIntro::To);
    }
    // `WITH GRANT OPTION`, `COPY CURRENT GRANTS`, etc. — kept inline as tokens.
    grant_tail(p);
    m.complete(p, GRANT_STMT);
}

/// At the contextual `TO` that introduces a grantee.
fn at_to(p: &Parser) -> bool {
    p.nth_contextual(0, ContextualKeyword::To)
}

/// Which keyword introduces the grantee: `TO` for GRANT, `FROM` for REVOKE.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GranteeIntro {
    To,
    From,
}

/// `REVOKE [GRANT OPTION FOR] <privileges> ON <object> FROM [ROLE|USER] <name> [CASCADE|RESTRICT]`.
fn revoke_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(REVOKE_KW);
    // Optional `GRANT OPTION FOR` prefix — `GRANT`/`FOR` are keywords; `OPTION` is contextual.
    if p.at(GRANT_KW) && p.nth_contextual(1, ContextualKeyword::Option) {
        p.bump(GRANT_KW);
        p.bump_as(CONTEXTUAL_KEYWORD); // OPTION
        p.eat(FOR_KW);
    }
    priv_list(p, |p| p.at(ON_KW) || p.at(FROM_KW));
    if p.at(ON_KW) {
        grant_target(p);
    }
    if p.at(FROM_KW) {
        grantee(p, GranteeIntro::From);
    }
    // `CASCADE` / `RESTRICT` and any unmodeled tail — kept inline as tokens.
    grant_tail(p);
    m.complete(p, REVOKE_STMT);
}

/// The comma-separated privilege list before `ON` (`SELECT, INSERT, UPDATE`), the catch-all
/// `ALL [PRIVILEGES]`, or a role/privilege word. `stop` reports the token that ends the list.
fn priv_list(p: &mut Parser, stop: impl Fn(&Parser) -> bool) {
    let m = p.start();
    while !stop(p) && !at_to(p) && !at_stmt_terminator(p) {
        if p.at(COMMA) {
            p.bump(COMMA);
        } else if p.nth_contextual(0, ContextualKeyword::Privileges) {
            p.bump_as(CONTEXTUAL_KEYWORD); // `ALL PRIVILEGES`
        } else {
            // A privilege word/phrase (SELECT, ALL, IMPORTED, …). Up-case keyword-spelled ones
            // (SELECT/INSERT/UPDATE/DELETE/CREATE/…); keep others verbatim.
            p.bump_any();
        }
    }
    m.complete(p, PRIV_LIST);
}

/// The `ON <object_type> <object_name>` securable. The object type and name are kept as inline
/// tokens (`TABLE db.sch.t`, `ALL TABLES IN SCHEMA s`, `FUTURE SCHEMAS IN DATABASE d`) so the wide,
/// open-ended surface round-trips losslessly while still living on its own line.
fn grant_target(p: &mut Parser) {
    let m = p.start();
    p.bump(ON_KW);
    while !at_to(p) && !p.at(FROM_KW) && !at_grant_tail(p) && !at_stmt_terminator(p) {
        if at_object_type_word(p) {
            p.bump_as(CONTEXTUAL_KEYWORD); // SCHEMA / DATABASE / STAGE / SEQUENCE / STREAM
        } else {
            p.bump_any();
        }
    }
    m.complete(p, GRANT_TARGET);
}

/// An object-type word inside a GRANT/REVOKE securable (`ON SCHEMA s`, `ON ALL TABLES IN DATABASE
/// d`). The reserved ones (`TABLE`/`VIEW`/`WAREHOUSE`) are already up-cased by `bump_any`; this
/// reaches the contextual ones so the whole securable reads in canonical case.
fn at_object_type_word(p: &Parser) -> bool {
    p.nth_any_contextual(0, OBJECT_TYPE_CONTEXTUAL_WORDS)
}

/// The `{ TO | FROM } [ROLE|USER|SHARE|DATABASE ROLE] <name>` recipient. A trailing
/// `WITH GRANT OPTION` / `CASCADE` / `RESTRICT` ends the grantee.
fn grantee(p: &mut Parser, intro: GranteeIntro) {
    let m = p.start();
    // The introducer keyword: `FROM` is reserved; `TO` is contextual.
    match intro {
        GranteeIntro::To => {
            if at_to(p) {
                p.bump_as(CONTEXTUAL_KEYWORD);
            }
        }
        GranteeIntro::From => p.expect(FROM_KW),
    }
    // Optional grantee kind (ROLE / USER / SHARE / DATABASE ROLE / APPLICATION ROLE …).
    while at_grantee_kind(p) {
        p.bump_as(CONTEXTUAL_KEYWORD);
    }
    if p.at_name() {
        name_ref(p);
    }
    m.complete(p, GRANTEE);
}

/// A grantee-kind word that precedes the recipient name (`ROLE r`, `USER u`, `SHARE s`).
fn at_grantee_kind(p: &Parser) -> bool {
    p.nth_any_contextual(0, GRANTEE_KIND_CONTEXTUAL_WORDS)
}

/// The optional statement tail after the grantee: `WITH GRANT OPTION`, `COPY CURRENT GRANTS`,
/// `CASCADE`, `RESTRICT`, `GRANTED BY …`. Kept as inline tokens so it round-trips and stays on the
/// grantee's line; the recognized access-control words (`OPTION`/`CASCADE`/`RESTRICT`) are tagged
/// contextual so the formatter up-cases them like keywords.
fn grant_tail(p: &mut Parser) {
    while !at_stmt_terminator(p) {
        if p.nth_any_contextual(0, GRANT_TAIL_CONTEXTUAL_WORDS) {
            p.bump_as(CONTEXTUAL_KEYWORD);
        } else {
            p.bump_any();
        }
    }
}

/// At the start of a grant/revoke statement tail (`WITH GRANT OPTION`, `CASCADE`, `RESTRICT`, …),
/// used so `grant_target` does not swallow it.
fn at_grant_tail(p: &Parser) -> bool {
    p.at(WITH_KW) || p.nth_any_contextual(0, GRANT_TAIL_START_CONTEXTUAL_WORDS)
}

/// At an `AS` that introduces a statement/query body (a task's DML, a dynamic-table query, a
/// procedural block) rather than an inline option like `CREATE DATABASE d AS REPLICA OF …`.
fn at_create_body(p: &Parser) -> bool {
    p.at(AS_KW)
        && (p.nth_at(1, SELECT_KW)
            || p.nth_at(1, WITH_KW)
            || p.nth_at(1, VALUES_KW)
            || p.nth_at(1, INSERT_KW)
            || p.nth_at(1, UPDATE_KW)
            || p.nth_at(1, DELETE_KW)
            || p.nth_at(1, MERGE_KW)
            || p.nth_at(1, CALL_KW)
            || p.nth_at(1, BEGIN_KW)
            || p.nth_at(1, L_PAREN))
}

/// `CREATE ... PROCEDURE/FUNCTION name (params) RETURNS ... <options> AS <body>`.
///
/// Skeleton support (Phase 8): the signature/options are kept leniently as tokens. Delimited bodies
/// (`$$ … $$` or a quoted string) remain a single token, while unquoted Snowflake Scripting bodies
/// (`AS BEGIN … END` / `AS DECLARE … BEGIN … END`) reuse the block parser so inner `;` separators
/// never split the outer routine statement.
fn create_routine(p: &mut Parser) {
    p.bump_any(); // PROCEDURE or FUNCTION
    name_ref(p);
    if p.at(L_PAREN) {
        column_def_list(p); // parameter list, parsed leniently like column defs
    }
    // RETURNS / LANGUAGE / RUNTIME_VERSION / PACKAGES / HANDLER / EXECUTE AS / ... up to `AS <body>`.
    while !at_routine_body(p) && !at_stmt_terminator(p) {
        p.bump_any();
    }
    if at_routine_body(p) {
        p.bump(AS_KW);
        if p.at(DOLLAR_STRING) || p.at(STRING) {
            p.bump_any(); // the delimited body token
        } else if at_block_start(p) {
            block_stmt(p);
        }
    } else {
        p.error("expected a routine body (AS $$ … $$, AS '…', or AS BEGIN … END)");
    }
}

/// At `AS` immediately followed by a routine body (so we don't stop on `EXECUTE AS`).
fn at_routine_body(p: &Parser) -> bool {
    p.at(AS_KW)
        && (p.nth_at(1, DOLLAR_STRING)
            || p.nth_at(1, STRING)
            || p.nth_at(1, DECLARE_KW)
            || (p.nth_at(1, BEGIN_KW)
                && !p.nth_at(2, SEMICOLON)
                && !p.nth_contextual(2, ContextualKeyword::Transaction)
                && !p.nth_contextual(2, ContextualKeyword::Work)))
}

// ---- COPY INTO (Phase 6) ----

/// `COPY INTO <target> FROM <source> <option>*` (both the load and unload shapes). The location
/// operands (`@stage/path`, table names) are captured verbatim — stage paths use `/` which would be
/// mangled by operator spacing — while options are parsed as `name = value` (or `PARTITION BY (...)`)
/// so each can sit on its own line.
fn copy_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(COPY_KW);
    p.expect(INTO_KW);
    copy_operand(p);
    if p.eat(FROM_KW) {
        copy_operand(p);
    }
    while !at_stmt_terminator(p) {
        copy_option(p);
    }
    m.complete(p, COPY_STMT);
}

/// A staged-file reference used where a table can appear: `@[~|%][namespace.]stage[/path...]`.
///
/// The lexer splits this into many tokens (`@`, names, `/`, `.`, `~`, `%`, numbers); we gather a
/// contiguous run into one `STAGE_REF` node. To avoid swallowing a following clause keyword (e.g.
/// `FROM @s WHERE`), additional path segments are only consumed when joined by a `/` or `.`
/// connector — a bare word after whitespace ends the reference. The rule is total and never panics.
fn stage_ref(p: &mut Parser) {
    let m = p.start();
    p.bump(AT); // @
    p.eat(TILDE); // @~ (the user's home stage)
    p.eat(PERCENT); // @%table (a table's internal stage)
    eat_stage_atom(p); // stage / table / namespace name (possibly a quoted identifier)
    while p.at(DOT) || p.at(SLASH) {
        // Further `.namespace` / `/path` segments, only when introduced by a connector.
        p.bump_any(); // . or /
        while eat_stage_atom(p) {}
    }
    m.complete(p, STAGE_REF);
}

/// Consume one atom of a stage path (a name, number, or `~`/`%`), returning whether one was eaten.
/// Anything else (paren, comma, `=`, EOF, a clause keyword) ends the path.
fn eat_stage_atom(p: &mut Parser) -> bool {
    if p.at(IDENT)
        || p.at(QUOTED_IDENT)
        || p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(TILDE)
        || p.at(PERCENT)
    {
        p.bump_any();
        true
    } else {
        false
    }
}

/// A COPY target/source: a parenthesized query, or a location captured as a verbatim token run up
/// to `FROM`, the first option, or the statement end.
fn copy_operand(p: &mut Parser) {
    if p.at(L_PAREN) {
        subquery(p);
        return;
    }
    let m = p.start();
    while !p.at(FROM_KW) && !at_stmt_terminator(p) && !at_copy_option_start(p) {
        p.bump_any();
    }
    m.complete(p, COPY_LOCATION);
}

/// A COPY option starts at `PARTITION BY` or any word immediately followed by `=`.
fn at_copy_option_start(p: &Parser) -> bool {
    p.at(PARTITION_KW) || p.nth_at(1, EQ)
}

fn copy_option(p: &mut Parser) {
    let m = p.start();
    if p.at(PARTITION_KW) {
        p.bump(PARTITION_KW);
        p.expect(BY_KW);
        if p.at(L_PAREN) {
            balanced_parens(p);
        }
    } else {
        p.bump_any(); // option name
        if p.eat(EQ) {
            if p.at(L_PAREN) {
                balanced_parens(p);
            } else if !at_stmt_terminator(p) {
                p.bump_any(); // a single literal / bare word value
            }
        }
    }
    m.complete(p, COPY_OPTION);
}

// ---- session SET / EXECUTE IMMEDIATE ----

/// `SET <name> = <expr>` or `SET (<name>, ...) = (<expr>, ...)` (session variables).
fn set_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(SET_KW);
    if p.at(L_PAREN) {
        column_list(p);
    } else {
        name_ref(p);
    }
    p.expect(EQ);
    if p.at(L_PAREN) {
        // A tuple / subquery right-hand side.
        p.bump(L_PAREN);
        if p.at(SELECT_KW) || p.at(WITH_KW) {
            query_expr(p);
        } else if !p.at(R_PAREN) {
            expr_list(p);
        }
        p.expect(R_PAREN);
    } else {
        expr(p);
    }
    m.complete(p, SET_STMT);
}

/// `EXECUTE IMMEDIATE <string|$$…$$|:var> [USING (<binds>)]`.
fn execute_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(EXECUTE_KW);
    p.expect(IMMEDIATE_KW);
    expr(p);
    if p.eat(USING_KW) {
        if p.at(L_PAREN) {
            p.bump(L_PAREN);
            if !p.at(R_PAREN) {
                expr_list(p);
            }
            p.expect(R_PAREN);
        } else {
            expr(p);
            while p.eat(COMMA) {
                expr(p);
            }
        }
    }
    m.complete(p, EXECUTE_STMT);
}

fn create_view(p: &mut Parser) {
    p.bump(VIEW_KW);
    if_exists_clause(p);
    name_ref(p);
    if p.at(L_PAREN) {
        column_list(p);
    }
    // Tolerate view options (COMMENT = '...', masking policies, ...) up to the defining query,
    // up-casing recognized DDL words so they format like keywords.
    while !p.at(AS_KW) && !p.at(SELECT_KW) && !p.at(WITH_KW) && !at_stmt_terminator(p) {
        bump_ddl_word(p);
    }
    p.eat(AS_KW);
    if p.at(SELECT_KW) || p.at(WITH_KW) || p.at(VALUES_KW) || p.at(L_PAREN) {
        query_expr(p);
    }
}

fn create_table(p: &mut Parser) {
    p.bump(TABLE_KW);
    if_exists_clause(p);
    name_ref(p);
    if p.at(L_PAREN) {
        column_def_list(p);
    }
    // `CREATE TABLE <name> CLONE <source> [<time-travel>]` — no column list, no CTAS.
    if p.nth_contextual(0, ContextualKeyword::Clone) {
        p.bump_as(CONTEXTUAL_KEYWORD); // CLONE
        if p.at_name() {
            name_ref(p);
        }
    }
    // Tolerate table options (CLUSTER BY (...), COMMENT = '...', ...) up to an optional CTAS query,
    // up-casing the recognized DDL words so they format like keywords.
    while !p.at(AS_KW) && !at_stmt_terminator(p) {
        if p.dialect().supports_delta_table_options() && at_databricks_table_option(p) {
            databricks_table_option(p);
        } else {
            bump_ddl_word(p);
        }
    }
    if p.eat(AS_KW) {
        query_expr(p);
    }
}

fn at_databricks_table_option(p: &Parser) -> bool {
    p.at(USING_KW)
        || p.nth_contextual(0, ContextualKeyword::Location)
        || p.nth_contextual(0, ContextualKeyword::Tblproperties)
        || p.nth_contextual(0, ContextualKeyword::Options)
        || (p.nth_contextual(0, ContextualKeyword::Partitioned) && p.nth_at(1, BY_KW))
        || (p.nth_contextual(0, ContextualKeyword::Cluster) && p.nth_at(1, BY_KW))
}

fn databricks_table_option(p: &mut Parser) {
    let m = p.start();
    if p.at(USING_KW) {
        p.bump(USING_KW);
        if p.at_name() {
            name_ref(p);
        } else if !at_databricks_table_option_stop(p) {
            p.bump_any();
        }
    } else if p.nth_contextual(0, ContextualKeyword::Location) {
        p.bump_as(CONTEXTUAL_KEYWORD);
        if !at_databricks_table_option_stop(p) {
            p.bump_any();
        }
    } else if p.nth_contextual(0, ContextualKeyword::Tblproperties)
        || p.nth_contextual(0, ContextualKeyword::Options)
    {
        p.bump_as(CONTEXTUAL_KEYWORD);
        if p.at(L_PAREN) {
            balanced_parens(p);
        } else {
            databricks_table_option_tail(p);
        }
    } else if p.nth_contextual(0, ContextualKeyword::Partitioned)
        || p.nth_contextual(0, ContextualKeyword::Cluster)
    {
        p.bump_as(CONTEXTUAL_KEYWORD);
        p.expect(BY_KW);
        if p.at(L_PAREN) {
            balanced_parens(p);
        } else {
            databricks_table_option_tail(p);
        }
    } else {
        bump_ddl_word(p);
    }
    m.complete(p, OBJECT_PROPERTY);
}

fn databricks_table_option_tail(p: &mut Parser) {
    while !at_databricks_table_option_stop(p) && !at_databricks_table_option(p) {
        if p.at(L_PAREN) {
            balanced_parens(p);
        } else {
            bump_ddl_word(p);
        }
    }
}

fn at_databricks_table_option_stop(p: &Parser) -> bool {
    p.at(AS_KW) || at_stmt_terminator(p)
}

fn column_def_list(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    if !p.at(R_PAREN) {
        column_def(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            column_def(p);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, COLUMN_DEF_LIST);
}

/// A column definition or table constraint, captured leniently as `name type constraints...` up to
/// the next top-level comma or the closing paren (balanced inner parens for `NUMBER(10,2)` etc.).
///
/// The structure stays a flat token run (robust against the vast inline-constraint surface), but the
/// recognized constraint words (`NOT NULL`, `DEFAULT`, `PRIMARY KEY`, `UNIQUE`, `FOREIGN KEY`,
/// `REFERENCES`, `CONSTRAINT`, `CHECK`, `COLLATE`, `COMMENT`) are tagged so they up-case like
/// keywords. They remain contextual, so a column literally named `default`/`comment`/`key` still
/// round-trips (it is only ever up-cased, never reparsed differently).
fn column_def(p: &mut Parser) {
    let m = p.start();
    // An out-of-line constraint begins with CONSTRAINT / PRIMARY / UNIQUE / FOREIGN / CHECK. A plain
    // column begins with its name (an identifier — possibly a word that merely *looks* like a DDL
    // option, e.g. a column named `comment`), so that leading token is taken verbatim, never
    // up-cased, before the constraint words that follow it are tagged.
    let starts_with_constraint = p.nth_any_contextual(0, DDL_CONSTRAINT_START_CONTEXTUAL_WORDS);
    if !starts_with_constraint && !p.at(COMMA) && !p.at(R_PAREN) && !p.at_eof() {
        p.bump_any(); // the column name (verbatim, even if it spells a contextual word)
    }
    balanced_token_run_until(p, |p| p.at(COMMA) || p.at(R_PAREN), bump_ddl_word);
    m.complete(p, COLUMN_DEF);
}

/// Consume one token in a lenient DDL run, tagging a recognized constraint/option word as a
/// contextual keyword (so the formatter up-cases it) and everything else verbatim. Real reserved
/// keywords (`NOT`, `NULL`, `BY`, `IN`, …) already up-case via [`Parser::bump_any`].
fn bump_ddl_word(p: &mut Parser) {
    if p.at_keyword() {
        p.bump_any(); // a reserved keyword bumps as itself (already up-cased)
    } else if is_ddl_contextual_word(p) {
        p.bump_as(CONTEXTUAL_KEYWORD); // a contextual DDL word is tagged for up-casing
    } else {
        p.bump_any();
    }
}

/// Whether the current token is a recognized (non-reserved) DDL constraint/option word.
fn is_ddl_contextual_word(p: &Parser) -> bool {
    p.nth_any_contextual(0, DDL_CONTEXTUAL_WORDS)
}

fn drop_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(DROP_KW);
    // Object kind (TABLE / VIEW / SCHEMA / ...).
    if !at_stmt_terminator(p) {
        p.bump_any();
    }
    if_exists_clause(p);
    if p.at_name() {
        name_ref(p);
    }
    // Tolerate trailing options (CASCADE / RESTRICT / ...), up-casing the recognized DDL words.
    while !at_stmt_terminator(p) {
        bump_ddl_word(p);
    }
    m.complete(p, DROP_STMT);
}

/// `ALTER` has enormous surface; parse it leniently as a flat token run so it round-trips and gets
/// inline formatting rather than erroring the whole file.
fn alter_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(ALTER_KW);
    while !at_stmt_terminator(p) {
        p.bump_any();
    }
    m.complete(p, ALTER_STMT);
}

/// Parse the leading keyword and the rest of the statement as a flat token run, completing it as
/// `node`. Used for statements whose surface is large/evolving (GRANT, REVOKE) or simple enough that
/// inline token rendering is all the formatter needs (USE, SHOW, DESCRIBE, TRUNCATE): the result
/// round-trips losslessly and gets inline spacing normalization rather than erroring the file.
fn lenient_stmt(p: &mut Parser, node: SyntaxKind) {
    let m = p.start();
    p.bump_any(); // the leading statement keyword
    while !at_stmt_terminator(p) {
        p.bump_any();
    }
    m.complete(p, node);
}

/// At a `COMMENT ON …` statement. `comment` is a contextual keyword recognized only before `ON`, so
/// the very common `comment` column/identifier is never mistaken for this statement.
fn at_comment_stmt(p: &Parser) -> bool {
    p.nth_contextual(0, ContextualKeyword::Comment) && p.nth_at(1, ON_KW)
}

/// At a transaction-starting `BEGIN` (`BEGIN;`, `BEGIN TRANSACTION …`, `BEGIN WORK`) — as opposed to
/// a Snowflake Scripting block (`BEGIN <stmt>; … END`). Only the transaction form is recognized (so
/// it formats inline); a scripting block is left to pass through verbatim, its inner `;`-separated
/// statements never mis-split. `BEGIN NAME …` is intentionally not matched (rarer, and `name` is a
/// common identifier).
fn at_begin_transaction(p: &Parser) -> bool {
    p.at(BEGIN_KW)
        && (p.nth_at(1, SEMICOLON)
            || p.nth_contextual(1, ContextualKeyword::Transaction)
            || p.nth_contextual(1, ContextualKeyword::Work))
}

/// `COMMENT ON <object> IS '<text>'` (or `COMMENT IF EXISTS …`). Parsed leniently as a flat token
/// run after up-casing the contextual `COMMENT`, so it round-trips and formats inline.
fn comment_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // COMMENT
    while !at_stmt_terminator(p) {
        p.bump_any();
    }
    m.complete(p, COMMENT_STMT);
}

/// `CALL proc(args)` — invoke a stored procedure. The invocation is an ordinary call expression, so
/// its argument list is formatted like any other (one-per-line when it overflows). A trailing tail
/// (e.g. `INTO :result`) is kept leniently as tokens so it round-trips.
fn call_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(CALL_KW);
    expr(p); // the procedure-call expression: name(args)
    while !at_stmt_terminator(p) {
        p.bump_any();
    }
    m.complete(p, CALL_STMT);
}

// ---- Snowflake Scripting blocks (Phase 8) ----

/// A scripting block starts at `DECLARE`, or at a `BEGIN` that is not a transaction start.
fn at_block_start(p: &Parser) -> bool {
    p.at(DECLARE_KW) || (p.at(BEGIN_KW) && !at_begin_transaction(p))
}

/// `[DECLARE <decls>] BEGIN <body> [EXCEPTION <handlers>] END [<label>]` — a Snowflake Scripting
/// block. The body and handler bodies are statement sequences (`STMT_LIST`); control-flow statements
/// (IF / loops) are structured and everything else is kept as a lenient inline statement, so the
/// block round-trips losslessly even where a construct is not modeled in detail.
fn block_stmt(p: &mut Parser) {
    let m = p.start();
    if p.at(DECLARE_KW) {
        declare_section(p);
    }
    p.expect(BEGIN_KW);
    stmt_list(p, |p| p.at(END_KW) || p.at(EXCEPTION_KW));
    if p.at(EXCEPTION_KW) {
        exception_section(p);
    }
    p.expect(END_KW);
    if p.at_name() {
        name_ref(p); // optional label after END
    }
    m.complete(p, BLOCK_STMT);
}

/// `DECLARE <decl>; <decl>; …` — each declaration kept leniently as a token run up to its `;` (a
/// cursor/resultset declaration's inner query has no top-level `;`, so this is safe).
fn declare_section(p: &mut Parser) {
    let m = p.start();
    p.bump(DECLARE_KW);
    while !p.at(BEGIN_KW) && !p.at_eof() {
        if p.eat(SEMICOLON) {
            continue;
        }
        declare_item(p);
        p.eat(SEMICOLON);
    }
    m.complete(p, DECLARE_SECTION);
}

fn declare_item(p: &mut Parser) {
    let m = p.start();
    let mut first = true;
    while !p.at(SEMICOLON) && !p.at(BEGIN_KW) && !p.at_eof() {
        // `<name> [<type>] DEFAULT <expr>` / cursor `… FOR <query>`: up-case the `DEFAULT` value
        // word (not reserved — `default` is a common identifier, so never the declared name in the
        // first position) and keep the rest verbatim.
        if !first && p.nth_contextual(0, ContextualKeyword::Default) {
            p.bump_as(CONTEXTUAL_KEYWORD);
        } else {
            p.bump_any();
        }
        first = false;
    }
    m.complete(p, DECLARE_ITEM);
}

/// A sequence of scripting statements, each terminated by `;`, until `is_end` holds. Wrapped in a
/// `STMT_LIST` so the formatter can indent the whole body as one unit.
fn stmt_list(p: &mut Parser, is_end: impl Fn(&Parser) -> bool) {
    let m = p.start();
    while !is_end(p) && !p.at_eof() {
        if p.eat(SEMICOLON) {
            continue; // a stray/empty `;`
        }
        block_statement(p);
        p.eat(SEMICOLON);
    }
    m.complete(p, STMT_LIST);
}

/// One statement inside a scripting block: a structured control-flow construct, a nested block, or a
/// lenient inline statement (LET / RETURN / assignment / a SQL statement / anything else up to `;`).
fn block_statement(p: &mut Parser) {
    if p.at(IF_KW) {
        if_stmt(p);
    } else if p.at(FOR_KW) || p.at(WHILE_KW) || p.at(LOOP_KW) || p.at(REPEAT_KW) {
        loop_stmt(p);
    } else if at_block_start(p) {
        block_stmt(p); // nested DECLARE…/BEGIN…END
    } else if p.at(CASE_KW) {
        case_stmt(p);
    } else if p.at(LET_KW) {
        simple_script_stmt(p, LET_STMT);
    } else if p.at(RETURN_KW) {
        simple_script_stmt(p, RETURN_STMT);
    } else if stmt::at_sql_stmt_start(p) {
        stmt::statement(p);
    } else if p.at_name() && p.nth_at(1, ASSIGN) {
        simple_script_stmt(p, ASSIGN_STMT);
    } else {
        simple_script_stmt(p, SCRIPT_STMT);
    }
}

/// A lenient scripting statement: consume tokens up to (but not including) the terminating `;`. Every
/// Snowflake Scripting statement ends with `;`, so this captures the whole statement — including an
/// expression `CASE … END` on the right of a `LET`/assignment — without mis-splitting.
fn simple_script_stmt(p: &mut Parser, node: SyntaxKind) {
    let m = p.start();
    let mut first = true;
    while !p.at(SEMICOLON) && !p.at_eof() {
        // Up-case the scripting structural words that are not reserved, but only in a position where
        // the word cannot be an identifier — so a variable literally named `default`/`break` is left
        // alone. `DEFAULT` introduces a value and never starts a statement; `BREAK`/`CONTINUE` are
        // whole statements (the only/first token). Everything else is kept verbatim.
        let up = if first {
            p.nth_contextual(0, ContextualKeyword::Break)
                || p.nth_contextual(0, ContextualKeyword::Continue)
        } else {
            p.nth_contextual(0, ContextualKeyword::Default) && !p.nth_at(1, ASSIGN)
        };
        if up {
            p.bump_as(CONTEXTUAL_KEYWORD);
        } else {
            p.bump_any();
        }
        first = false;
    }
    m.complete(p, node);
}

/// `IF <cond> THEN <body> [ELSEIF <cond> THEN <body>]… [ELSE <body>] END IF`.
fn if_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(IF_KW);
    expr(p); // condition (parenthesized or bare)
    p.expect(THEN_KW);
    stmt_list(p, |p| p.at(ELSEIF_KW) || p.at(ELSE_KW) || p.at(END_KW));
    while p.at(ELSEIF_KW) {
        p.bump(ELSEIF_KW);
        expr(p);
        p.expect(THEN_KW);
        stmt_list(p, |p| p.at(ELSEIF_KW) || p.at(ELSE_KW) || p.at(END_KW));
    }
    if p.eat(ELSE_KW) {
        stmt_list(p, |p| p.at(END_KW));
    }
    p.expect(END_KW);
    p.expect(IF_KW);
    m.complete(p, IF_STMT);
}

/// A procedural `CASE` statement, in both documented forms: searched
/// `CASE WHEN <cond> THEN <stmts> ... END [CASE]` and simple
/// `CASE <operand> WHEN <value> THEN <stmts> ... END [CASE]`.
fn case_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(CASE_KW);
    if !p.at(WHEN_KW) {
        expr(p); // simple CASE operand
    }
    while p.at(WHEN_KW) {
        let arm = p.start();
        p.bump(WHEN_KW);
        expr(p);
        p.expect(THEN_KW);
        stmt_list(p, |p| p.at(WHEN_KW) || p.at(ELSE_KW) || p.at(END_KW));
        arm.complete(p, CASE_STMT_WHEN);
    }
    if p.eat(ELSE_KW) {
        stmt_list(p, |p| p.at(END_KW));
    }
    p.expect(END_KW);
    p.eat(CASE_KW); // optional trailing CASE in `END CASE`
    m.complete(p, CASE_STMT);
}

/// `FOR …/WHILE … DO <body> END FOR/WHILE`, `LOOP <body> END LOOP`, and
/// `REPEAT <body> UNTIL <cond> END REPEAT` — unified as one loop node.
fn loop_stmt(p: &mut Parser) {
    let m = p.start();
    if p.at(FOR_KW) || p.at(WHILE_KW) {
        p.bump_any(); // FOR / WHILE
        while !p.at(DO_KW) && !p.at(SEMICOLON) && !p.at(END_KW) && !p.at_eof() {
            // The counter-loop range words `REVERSE`/`TO` up-case in this position; everything else
            // (the counter name, bounds, cursor name, `USING (…)`, the condition) is kept verbatim.
            if p.nth_contextual(0, ContextualKeyword::Reverse)
                || p.nth_contextual(0, ContextualKeyword::To)
            {
                p.bump_as(CONTEXTUAL_KEYWORD);
            } else {
                p.bump_any(); // loop header (counter/range or cursor; condition)
            }
        }
        p.expect(DO_KW);
        stmt_list(p, |p| p.at(END_KW));
    } else if p.at(LOOP_KW) {
        p.bump(LOOP_KW);
        stmt_list(p, |p| p.at(END_KW));
    } else {
        p.bump(REPEAT_KW);
        stmt_list(p, |p| p.at(UNTIL_KW) || p.at(END_KW));
        if p.eat(UNTIL_KW) {
            expr(p); // loop condition
            while !p.at(END_KW) && !p.at(SEMICOLON) && !p.at_eof() {
                p.bump_any();
            }
        }
    }
    p.expect(END_KW);
    // The matching trailer keyword: END FOR / WHILE / LOOP / REPEAT.
    if p.at(FOR_KW) || p.at(WHILE_KW) || p.at(LOOP_KW) || p.at(REPEAT_KW) {
        p.bump_any();
    }
    m.complete(p, LOOP_STMT);
}

/// `EXCEPTION WHEN <exc> [OR <exc>]… THEN <body> …` inside a block.
fn exception_section(p: &mut Parser) {
    let m = p.start();
    p.bump(EXCEPTION_KW);
    while p.at(WHEN_KW) {
        exception_when(p);
    }
    m.complete(p, EXCEPTION_SECTION);
}

fn exception_when(p: &mut Parser) {
    let m = p.start();
    p.bump(WHEN_KW);
    while !p.at(THEN_KW) && !p.at(WHEN_KW) && !p.at(END_KW) && !p.at_eof() {
        p.bump_any(); // exception name(s) / OTHER
    }
    p.expect(THEN_KW);
    stmt_list(p, |p| p.at(WHEN_KW) || p.at(END_KW));
    m.complete(p, EXCEPTION_WHEN);
}

// ---- queries ----

fn with_query(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    with_clause(p);
    query_expr(p);
    m.complete(p, WITH_QUERY)
}

fn with_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(WITH_KW);
    p.eat(RECURSIVE_KW);
    cte(p);
    while p.eat(COMMA) {
        cte(p);
    }
    m.complete(p, WITH_CLAUSE);
}

fn cte(p: &mut Parser) {
    let m = p.start();
    name(p);
    if p.at(L_PAREN) {
        column_list(p);
    }
    p.expect(AS_KW);
    subquery(p);
    m.complete(p, CTE);
}

/// A query expression: query primaries combined by left-associative set operations.
fn query_expr(p: &mut Parser) -> Option<CompletedMarker> {
    let mut lhs = query_primary(p)?;
    while p.at(UNION_KW) || p.at(EXCEPT_KW) || p.at(INTERSECT_KW) || p.at(MINUS_KW) {
        let m = lhs.precede(p);
        p.bump_any(); // the set operator
        if p.at(ALL_KW) {
            p.bump(ALL_KW);
        } else if p.at(DISTINCT_KW) {
            p.bump(DISTINCT_KW);
        }
        query_primary(p);
        lhs = m.complete(p, SET_OP);
    }
    Some(lhs)
}

fn query_primary(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at(L_PAREN) {
        Some(subquery(p))
    } else if p.at(SELECT_KW) {
        Some(select_core(p))
    } else if p.at(WITH_KW) {
        Some(with_query(p)) // a CTE query is a valid (sub)query: `(WITH ... SELECT ...)`, `AS WITH ...`
    } else if p.at(VALUES_KW) {
        Some(values_clause(p))
    } else {
        p.error("expected a query (SELECT, VALUES, WITH, or a parenthesized subquery)");
        None
    }
}

fn subquery(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect(L_PAREN);
    query_expr(p);
    p.expect(R_PAREN);
    m.complete(p, SUBQUERY)
}

fn select_core(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    if p.at(SELECT_KW) {
        p.bump(SELECT_KW);
    } else {
        p.error("expected SELECT");
    }
    if p.at(DISTINCT_KW) {
        p.bump(DISTINCT_KW);
    } else if p.at(ALL_KW) {
        p.bump(ALL_KW);
    }
    if p.at(TOP_KW) {
        top_clause(p);
    }
    select_list(p);
    if p.at(FROM_KW) {
        from_clause(p);
    }
    if p.at(WHERE_KW) {
        where_clause(p);
    }
    // Hierarchical queries: `START WITH` / `CONNECT BY` in either order.
    while p.at(START_KW) || p.at(CONNECT_KW) {
        if p.at(START_KW) {
            start_with_clause(p);
        } else {
            connect_by_clause(p);
        }
    }
    if p.at(GROUP_KW) {
        group_by_clause(p);
    }
    if p.at(HAVING_KW) {
        having_clause(p);
    }
    if p.at(QUALIFY_KW) {
        qualify_clause(p);
    }
    if p.at(WINDOW_KW) {
        window_clause(p);
    }
    if p.at(ORDER_KW) {
        order_by_clause(p);
    }
    if p.at(LIMIT_KW) {
        limit_clause(p);
    }
    if p.at(OFFSET_KW) {
        offset_clause(p);
    }
    if p.at(FETCH_KW) {
        fetch_clause(p);
    }
    m.complete(p, SELECT_STMT)
}

/// Snowflake `SELECT TOP <n>` header. Keep the count as direct header tokens instead of an
/// expression node so the SELECT-list formatter sees `TOP n` as part of the header, not as a child
/// to ignore. Parenthesized counts are accepted as a balanced token run.
fn top_clause(p: &mut Parser) {
    p.bump(TOP_KW);
    if p.at(L_PAREN) {
        balanced_parens(p);
    } else if p.at(INT_NUMBER) || p.at(FLOAT_NUMBER) || p.at(VARIABLE) || p.at_name() {
        p.bump_any();
    } else {
        p.error("expected a row count after TOP");
    }
}

// ---- SELECT list ----

fn select_list(p: &mut Parser) {
    let m = p.start();
    select_item(p);
    while p.eat(COMMA) {
        if p.at_eof() {
            p.error("expected a select item after ','");
            break;
        }
        if at_clause_end(p) {
            break; // tolerate a trailing comma
        }
        select_item(p);
    }
    m.complete(p, SELECT_LIST);
}

/// Keywords that end the select list (so a stray trailing comma doesn't eat them).
fn at_clause_end(p: &Parser) -> bool {
    p.at(FROM_KW)
        || p.at(WHERE_KW)
        || p.at(GROUP_KW)
        || p.at(HAVING_KW)
        || p.at(QUALIFY_KW)
        || p.at(WINDOW_KW)
        || p.at(ORDER_KW)
        || p.at(LIMIT_KW)
        || p.at(OFFSET_KW)
        || p.at(FETCH_KW)
}

fn select_item(p: &mut Parser) {
    let m = p.start();
    if p.at(STAR) || at_qualified_star(p) {
        star_select_expr(p);
    } else if at_expr_start(p) {
        expr(p);
        let explicit_alias = p.eat(AS_KW);
        if explicit_alias || p.at_name() {
            name(p); // implicit alias: SELECT a alias
        }
    } else {
        p.error("expected a select item");
    }
    m.complete(p, SELECT_ITEM);
}

fn at_qualified_star(p: &Parser) -> bool {
    if !p.at_name() {
        return false;
    }
    let mut i = 1;
    while i < 32 {
        if !p.nth_at(i, DOT) {
            return false;
        }
        if p.nth_at(i + 1, STAR) {
            return true;
        }
        if !(p.nth_at(i + 1, IDENT) || p.nth_at(i + 1, QUOTED_IDENT)) {
            return false;
        }
        i += 2;
    }
    false
}

fn star_select_expr(p: &mut Parser) {
    let s = p.start();
    if p.at(STAR) {
        p.bump(STAR);
    } else {
        // Qualified star: `t.*`, `"db"."schema".t.*`.
        name_ref(p);
    }
    while at_star_modifier(p) {
        star_modifier(p);
    }
    s.complete(p, STAR_EXPR);
}

fn at_star_modifier(p: &Parser) -> bool {
    p.at(ILIKE_KW)
        || p.at(REPLACE_KW)
        || (p.dialect().supports_delta_commands() && p.at(EXCEPT_KW))
        || (p.dialect().supports_semantic_view() && p.at_name())
}

fn star_modifier(p: &mut Parser) {
    if p.at(ILIKE_KW) {
        p.bump(ILIKE_KW);
        expr_bp(p, BP_CMP.1);
    } else if p.at(REPLACE_KW) {
        p.bump(REPLACE_KW);
        star_modifier_parens(p);
    } else if p.at(EXCEPT_KW) {
        p.bump(EXCEPT_KW);
        star_modifier_parens(p);
    } else if p.at_name() {
        // Snowflake's `EXCLUDE` and `RENAME` are contextual here. The parser does not reserve those
        // words globally, so recognize the modifier by position after `*`.
        p.bump_as(CONTEXTUAL_KEYWORD);
        if p.at(L_PAREN) {
            star_modifier_parens(p);
        } else if p.at_name() {
            name_ref(p);
            if p.eat(AS_KW) && p.at_name() {
                name(p);
            }
        }
    }
}

fn star_modifier_parens(p: &mut Parser) {
    p.expect(L_PAREN);
    if !p.at(R_PAREN) {
        star_modifier_item(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            star_modifier_item(p);
        }
    }
    p.expect(R_PAREN);
}

fn star_modifier_item(p: &mut Parser) {
    if at_expr_start(p) {
        expr(p);
    } else if p.at(STAR) {
        p.bump(STAR);
    } else {
        p.error("expected a star modifier item");
        return;
    }
    if p.eat(AS_KW) && p.at_name() {
        name(p);
    }
}

// ---- FROM / JOIN ----

fn from_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(FROM_KW);
    table_ref(p);
    loop {
        if p.dialect().supports_lateral_view() && at_lateral_view(p) {
            lateral_view(p);
        } else if at_join_start(p) {
            join(p);
        } else if p.eat(COMMA) {
            table_ref(p);
        } else {
            break;
        }
    }
    m.complete(p, FROM_CLAUSE);
}

fn table_ref(p: &mut Parser) {
    let m = p.start();
    p.eat(LATERAL_KW); // LATERAL FLATTEN(...) / LATERAL (subquery)
    if p.at(L_PAREN) {
        subquery(p); // derived table
    } else if p.at(VALUES_KW) {
        values_clause(p); // FROM VALUES (...), (...) [AS t(c1, c2)]
    } else if p.at(FLATTEN_KW) || p.at(TABLE_KW) {
        // Keyword-named table function: FLATTEN(...) / TABLE(...).
        p.bump_any();
        if p.at(L_PAREN) {
            arg_list(p);
        } else {
            p.error("expected '(' after table function");
        }
    } else if p.at(VARIABLE) {
        // Flow-operator back-reference: `FROM $1` points at a previous statement in the chain.
        let r = p.start();
        p.bump(VARIABLE);
        r.complete(p, NAME_REF);
    } else if p.at(AT) {
        // A staged-file source: `FROM @stage[/path] [( FILE_FORMAT => ... )]` (data-load transform).
        stage_ref(p);
        if p.at(L_PAREN) {
            arg_list(p); // FROM @s ( FILE_FORMAT => my_ff, PATTERN => '...' )
        }
    } else if p.at_name() {
        name_ref(p);
        if p.at(L_PAREN) {
            arg_list(p); // table function: my_udtf(args)
        }
    } else {
        p.error("expected a table reference");
    }
    // Change-tracking, time travel, SAMPLE / TABLESAMPLE, MATCH_RECOGNIZE, and PIVOT / UNPIVOT all
    // attach to the table before its alias.
    if p.nth_contextual(0, ContextualKeyword::Changes) {
        // `CHANGES ( INFORMATION => ... )` then AT|BEFORE (below) and an optional END ( ... ).
        p.bump_as(CONTEXTUAL_KEYWORD); // CHANGES
        if p.at(L_PAREN) {
            balanced_parens(p);
        }
    }
    if at_time_travel(p) {
        time_travel(p);
    }
    if p.dialect().supports_as_of_travel() && at_databricks_as_of_travel(p) {
        databricks_as_of_travel(p);
    }
    if p.at(END_KW) && p.nth_at(1, L_PAREN) {
        p.bump(END_KW); // CHANGES ... END ( TIMESTAMP => ... )
        balanced_parens(p);
    }
    if p.at(SAMPLE_KW) || p.at(TABLESAMPLE_KW) {
        sample_clause(p);
    }
    if p.nth_contextual(0, ContextualKeyword::MatchRecognize) {
        match_recognize(p);
    }
    while p.at(PIVOT_KW) || p.at(UNPIVOT_KW) {
        pivot_clause(p);
    }
    table_alias(p);
    m.complete(p, TABLE_REF);
}

fn at_lateral_view(p: &Parser) -> bool {
    p.at(LATERAL_KW) && p.nth_at(1, VIEW_KW)
}

/// Databricks/Spark `LATERAL VIEW [OUTER] generator(...) [table_alias] AS col [, ...]`.
fn lateral_view(p: &mut Parser) {
    let m = p.start();
    p.bump(LATERAL_KW);
    p.expect(VIEW_KW);
    p.eat(OUTER_KW);
    if at_expr_start(p) {
        expr(p);
    } else {
        p.error("expected a generator expression after LATERAL VIEW");
    }
    if p.at_name() {
        name(p);
    }
    p.eat(AS_KW);
    if p.at_name() {
        name(p);
        while p.eat(COMMA) {
            if p.at_name() {
                name(p);
            } else {
                p.error("expected a column alias after ','");
                break;
            }
        }
    }
    m.complete(p, LATERAL_VIEW);
}

/// Time-travel: `AT ( ... )` / `BEFORE ( ... )` (`at`/`before` are contextual keywords).
fn at_time_travel(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::At) || p.nth_contextual(0, ContextualKeyword::Before))
        && p.nth_at(1, L_PAREN)
}

/// `<table> {AT|BEFORE} ( TIMESTAMP|OFFSET|STATEMENT => ... )`, captured leniently.
fn time_travel(p: &mut Parser) {
    p.bump_as(CONTEXTUAL_KEYWORD); // AT / BEFORE (contextual keyword)
    if p.at(L_PAREN) {
        balanced_parens(p);
    }
}

fn at_databricks_as_of_travel(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::Version)
        || p.nth_contextual(0, ContextualKeyword::Timestamp))
        && p.nth_at(1, AS_KW)
        && p.nth_contextual(2, ContextualKeyword::Of)
}

/// Databricks table time travel: `VERSION AS OF <expr>` / `TIMESTAMP AS OF <expr>`.
fn databricks_as_of_travel(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // VERSION / TIMESTAMP
    p.expect(AS_KW);
    if p.nth_contextual(0, ContextualKeyword::Of) {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else {
        p.error("expected OF in time travel clause");
    }
    if at_expr_start(p) {
        expr(p);
    } else {
        p.error("expected a time travel value");
    }
    m.complete(p, AS_OF_TRAVEL);
}

/// `<table> {SAMPLE|TABLESAMPLE} [method] ( n [ROWS] ) [REPEATABLE|SEED ( seed )]`. The fraction
/// and any method/seed are captured leniently (balanced parens) for inline formatting.
fn sample_clause(p: &mut Parser) {
    p.bump_any(); // SAMPLE / TABLESAMPLE
                  // Sampling method: BERNOULLI / SYSTEM / BLOCK are plain words; ROW is the reserved keyword
                  // `ROW_KW` (so `p.at_name()` is false for it) — accept it explicitly. Guard the `(` so a bare
                  // `SAMPLE (10)` (no method) and a `ROW`-without-parens both stay total.
    if p.at(ROW_KW) && p.nth_at(1, L_PAREN) {
        p.bump(ROW_KW);
    } else if p.at_name() {
        name_ref(p);
    }
    if p.at(L_PAREN) {
        balanced_parens(p);
    }
    // Optional REPEATABLE(seed) / SEED(seed).
    if p.at_name() && p.nth_at(1, L_PAREN) {
        name_ref(p);
        balanced_parens(p);
    }
}

/// `<table> MATCH_RECOGNIZE ( <body> )`. The body's clauses appear in a fixed order
/// (PARTITION BY / ORDER BY / MEASURES / {ONE ROW|ALL ROWS} PER MATCH / AFTER MATCH SKIP /
/// PATTERN / SUBSET / DEFINE) but are parsed resiliently: dispatch on the clause-introducing word
/// and, for anything unrecognized, consume one token so the rule stays total and lossless. The
/// `MATCH_RECOGNIZE` word and the body keywords (MEASURES/PATTERN/DEFINE/…) are contextual.
fn match_recognize(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // MATCH_RECOGNIZE (contextual keyword)
    if !p.eat(L_PAREN) {
        p.error("expected '(' after MATCH_RECOGNIZE");
        m.complete(p, MATCH_RECOGNIZE);
        return;
    }
    while !p.at(R_PAREN) && !p.at_eof() {
        if p.at(PARTITION_KW) {
            partition_by_clause(p);
        } else if p.at(ORDER_KW) {
            order_by_clause(p);
        } else if p.nth_contextual(0, ContextualKeyword::Measures) {
            measures_clause(p);
        } else if p.nth_contextual(0, ContextualKeyword::Pattern) {
            pattern_clause(p);
        } else if p.nth_contextual(0, ContextualKeyword::Define) {
            define_clause(p);
        } else if p.nth_contextual(0, ContextualKeyword::Subset) {
            subset_clause(p);
        } else if at_row_match_clause(p) {
            row_match_clause(p);
        } else if p.at(AFTER_KW) {
            after_match_clause(p);
        } else {
            p.bump_any(); // lenient: never stall on unmodelled syntax
        }
    }
    p.expect(R_PAREN);
    m.complete(p, MATCH_RECOGNIZE);
}

/// `MEASURES <expr> [AS] <alias> [, ...]` (reusing the select-item shape: expression + optional
/// alias). `FINAL`/`RUNNING` measure prefixes are not modelled yet; they parse leniently.
fn measures_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // MEASURES
    select_item(p);
    while p.eat(COMMA) {
        select_item(p);
    }
    m.complete(p, MEASURES_CLAUSE);
}

/// `PATTERN ( <row pattern> )`. The pattern is a regex-like sub-language (`A+ B* (C | D){1,3}`)
/// where `+`/`*`/`?` are postfix quantifiers, not operators — capture it as a [`PATTERN_BODY`] node
/// so the formatter can emit it verbatim instead of re-spacing it.
fn pattern_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // PATTERN
    if p.at(L_PAREN) {
        let b = p.start();
        balanced_parens(p);
        b.complete(p, PATTERN_BODY);
    } else {
        p.error("expected '(' after PATTERN");
    }
    m.complete(p, PATTERN_CLAUSE);
}

/// `DEFINE <symbol> AS <predicate> [, ...]`.
fn define_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // DEFINE
    define_item(p);
    while p.eat(COMMA) {
        define_item(p);
    }
    m.complete(p, DEFINE_CLAUSE);
}

fn define_item(p: &mut Parser) {
    let m = p.start();
    name_ref(p); // pattern variable (symbol)
    p.expect(AS_KW);
    expr(p); // predicate
    m.complete(p, DEFINE_ITEM);
}

/// `SUBSET <name> = ( <symbol>, ... ) [, ...]`.
fn subset_clause(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // SUBSET
    loop {
        name_ref(p);
        p.expect(EQ);
        if p.at(L_PAREN) {
            column_list(p);
        }
        if !p.eat(COMMA) {
            break;
        }
    }
    m.complete(p, SUBSET_CLAUSE);
}

/// `{ ONE ROW | ALL ROWS } PER MATCH` — recognized by the leading `ONE`/`ALL` plus `ROW(S)`.
fn at_row_match_clause(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::One) || p.at(ALL_KW))
        && (p.nth_at(1, ROW_KW) || p.nth_at(1, ROWS_KW))
}

/// `{ ONE ROW | ALL ROWS } PER MATCH [ SHOW EMPTY MATCHES | OMIT EMPTY MATCHES | WITH UNMATCHED
/// ROWS ]`. After the leading `ONE ROW`/`ALL ROWS`, the remaining words are all structural (no
/// pattern symbols), so consume them as soft keywords until the next clause or the closing paren.
fn row_match_clause(p: &mut Parser) {
    let m = p.start();
    if p.at(ALL_KW) {
        p.bump(ALL_KW);
    } else {
        p.bump_as(CONTEXTUAL_KEYWORD); // ONE
    }
    while !p.at(R_PAREN) && !p.at_eof() && !at_mr_clause_start(p) {
        soft_keyword_word(p);
    }
    m.complete(p, ROW_MATCH_CLAUSE);
}

/// `AFTER MATCH SKIP { PAST LAST ROW | TO NEXT ROW | TO [ FIRST | LAST ] <symbol> }`. The trailing
/// `<symbol>` keeps its case (it is a pattern variable), so it is parsed as a name, not up-cased.
fn after_match_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(AFTER_KW);
    if p.nth_contextual(0, ContextualKeyword::Match) {
        p.bump_as(CONTEXTUAL_KEYWORD); // MATCH
    }
    if p.nth_contextual(0, ContextualKeyword::Skip) {
        p.bump_as(CONTEXTUAL_KEYWORD); // SKIP
    }
    if p.nth_contextual(0, ContextualKeyword::Past) {
        p.bump_as(CONTEXTUAL_KEYWORD); // PAST
        p.eat(LAST_KW);
        p.eat(ROW_KW);
    } else if p.nth_contextual(0, ContextualKeyword::To) {
        p.bump_as(CONTEXTUAL_KEYWORD); // TO
        if p.nth_contextual(0, ContextualKeyword::Next) {
            p.bump_as(CONTEXTUAL_KEYWORD); // NEXT
            p.eat(ROW_KW);
        } else {
            if p.at(FIRST_KW) || p.at(LAST_KW) {
                p.bump_any();
            }
            if p.at_name() {
                name_ref(p); // <symbol>
            }
        }
    }
    m.complete(p, AFTER_MATCH_CLAUSE);
}

/// True at the start of any MATCH_RECOGNIZE body clause — used to bound the lenient word runs.
fn at_mr_clause_start(p: &Parser) -> bool {
    p.at(PARTITION_KW)
        || p.at(ORDER_KW)
        || p.at(AFTER_KW)
        || p.nth_contextual(0, ContextualKeyword::Measures)
        || p.nth_contextual(0, ContextualKeyword::Pattern)
        || p.nth_contextual(0, ContextualKeyword::Define)
        || p.nth_contextual(0, ContextualKeyword::Subset)
        || at_row_match_clause(p)
}

/// Consume one word of a structural option run (`PER MATCH`, `SHOW EMPTY MATCHES`, …). These runs
/// hold no pattern symbols, so every word up-cases: tag identifier-like tokens as soft keywords and
/// leave any punctuation untouched.
fn soft_keyword_word(p: &mut Parser) {
    if p.at_ident_like() {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else {
        p.bump_any();
    }
}

/// Consume a balanced `( ... )` token run, tracking only token-level parentheses (string and
/// `$$ … $$` tokens are opaque, so their inner parens don't count).
fn balanced_parens(p: &mut Parser) {
    p.bump(L_PAREN);
    let mut depth = 1u32;
    while depth > 0 && !p.at_eof() {
        if p.at(L_PAREN) {
            depth += 1;
        } else if p.at(R_PAREN) {
            depth -= 1;
        }
        p.bump_any();
    }
}

/// Consume a token run that may contain nested parentheses, stopping when `stop` matches at the
/// top level. Callers supply the non-paren token bumping rule so contextual-word tagging remains
/// local to the grammar region being scanned.
fn balanced_token_run_until(
    p: &mut Parser,
    stop: impl Fn(&Parser) -> bool,
    mut bump_word: impl FnMut(&mut Parser),
) {
    let mut depth = 0u32;
    while !p.at_eof() {
        if depth == 0 && stop(p) {
            break;
        }
        if p.at(L_PAREN) {
            depth += 1;
            p.bump_any();
        } else if p.at(R_PAREN) && depth > 0 {
            depth -= 1;
            p.bump_any();
        } else {
            bump_word(p);
        }
    }
}

fn pivot_clause(p: &mut Parser) {
    let m = p.start();
    let is_pivot = p.at(PIVOT_KW);
    p.bump_any(); // PIVOT or UNPIVOT
    p.expect(L_PAREN);
    if is_pivot {
        // PIVOT ( <agg>(col) FOR col IN ( value, ... ) )
        expr(p);
    } else {
        // UNPIVOT ( value_col FOR name_col IN ( col, ... ) )
        name_ref(p);
    }
    p.expect(FOR_KW);
    name_ref(p);
    p.expect(IN_KW);
    p.expect(L_PAREN);
    if !p.at(R_PAREN) {
        pivot_value(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            pivot_value(p);
        }
    }
    p.expect(R_PAREN);
    p.expect(R_PAREN);
    m.complete(p, PIVOT_CLAUSE);
}

/// A PIVOT `IN` value: an expression with an optional alias (`1 AS JAN`, `'jan' AS January`).
fn pivot_value(p: &mut Parser) {
    expr(p);
    if p.eat(AS_KW) || p.at_name() {
        name(p);
    }
}

fn table_alias(p: &mut Parser) {
    let explicit_alias = p.eat(AS_KW);
    if explicit_alias || (p.at_name() && !at_alias_blocker(p)) {
        name(p);
        if p.at(L_PAREN) {
            column_list(p); // derived-table column aliases: (c1, c2, ...)
        }
    }
}

/// A contextual word that follows a table but introduces a clause rather than being its alias:
/// `ASOF JOIN`, `MATCH_CONDITION (...)`. (`MATCH_RECOGNIZE` is consumed before the alias already.)
fn at_alias_blocker(p: &Parser) -> bool {
    (p.nth_contextual(0, ContextualKeyword::Asof) && p.nth_at(1, JOIN_KW))
        || (p.nth_contextual(0, ContextualKeyword::MatchCondition) && p.nth_at(1, L_PAREN))
        || at_time_travel(p)
        || (p.dialect().supports_as_of_travel() && at_databricks_as_of_travel(p))
        || (p.dialect().supports_lateral_view() && at_lateral_view(p))
}

fn at_join_start(p: &Parser) -> bool {
    p.at(JOIN_KW)
        || p.at(INNER_KW)
        || p.at(LEFT_KW)
        || p.at(RIGHT_KW)
        || p.at(FULL_KW)
        || p.at(CROSS_KW)
        || p.at(NATURAL_KW)
        || (p.nth_contextual(0, ContextualKeyword::Asof) && p.nth_at(1, JOIN_KW))
}

fn join(p: &mut Parser) {
    let m = p.start();
    p.eat(NATURAL_KW);
    if p.nth_contextual(0, ContextualKeyword::Asof) {
        p.bump_as(CONTEXTUAL_KEYWORD); // ASOF (contextual keyword)
    } else if p.at(INNER_KW) {
        p.bump(INNER_KW);
    } else if p.at(LEFT_KW) || p.at(RIGHT_KW) || p.at(FULL_KW) {
        p.bump_any();
        p.eat(OUTER_KW);
    } else if p.at(CROSS_KW) {
        p.bump(CROSS_KW);
    }
    p.expect(JOIN_KW);
    table_ref(p);
    // ASOF joins carry a MATCH_CONDITION ( <predicate> ) before any ON.
    if p.nth_contextual(0, ContextualKeyword::MatchCondition) {
        p.bump_as(CONTEXTUAL_KEYWORD); // MATCH_CONDITION (contextual keyword)
        p.expect(L_PAREN);
        expr(p);
        p.expect(R_PAREN);
    }
    if p.eat(ON_KW) {
        expr(p);
    } else if p.eat(USING_KW) {
        column_list(p);
    }
    m.complete(p, JOIN);
}

// ---- other clauses ----

fn where_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(WHERE_KW);
    expr(p);
    m.complete(p, WHERE_CLAUSE);
}

/// `START WITH <predicate>` — the seed of a hierarchical (`CONNECT BY`) query.
fn start_with_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(START_KW);
    p.expect(WITH_KW);
    expr(p);
    m.complete(p, START_WITH_CLAUSE);
}

/// `CONNECT BY [NOCYCLE] <predicate>` (the predicate uses the `PRIOR` prefix to refer to the parent
/// row).
fn connect_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(CONNECT_KW);
    p.expect(BY_KW);
    if p.nth_contextual(0, ContextualKeyword::NoCycle) {
        p.bump_as(CONTEXTUAL_KEYWORD); // NOCYCLE
    }
    expr(p);
    m.complete(p, CONNECT_BY_CLAUSE);
}

fn group_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(GROUP_KW);
    p.expect(BY_KW);
    if p.at(ALL_KW) {
        p.bump(ALL_KW);
    } else {
        grouping_element(p);
        while p.eat(COMMA) {
            grouping_element(p);
        }
    }
    m.complete(p, GROUP_BY_CLAUSE);
}

/// A `GROUP BY` element: `GROUPING SETS (...)`, or an ordinary expression (which already covers
/// `CUBE(...)` / `ROLLUP(...)`, parsed as function calls).
fn grouping_element(p: &mut Parser) {
    if p.nth_contextual(0, ContextualKeyword::Grouping)
        && p.nth_contextual(1, ContextualKeyword::Sets)
    {
        let m = p.start();
        p.bump_as(CONTEXTUAL_KEYWORD); // GROUPING (contextual keyword)
        p.bump_as(CONTEXTUAL_KEYWORD); // SETS (contextual keyword)
        p.expect(L_PAREN);
        if !p.at(R_PAREN) {
            grouping_set(p);
            while p.eat(COMMA) {
                if p.at(R_PAREN) {
                    break;
                }
                grouping_set(p);
            }
        }
        p.expect(R_PAREN);
        m.complete(p, GROUPING_SETS);
    } else {
        expr(p);
    }
}

/// One set inside `GROUPING SETS`: a parenthesized (possibly empty) tuple of expressions, or a
/// single bare expression.
fn grouping_set(p: &mut Parser) {
    if p.at(L_PAREN) {
        p.bump(L_PAREN);
        if !p.at(R_PAREN) {
            expr(p);
            while p.eat(COMMA) {
                if p.at(R_PAREN) {
                    break;
                }
                expr(p);
            }
        }
        p.expect(R_PAREN);
    } else {
        expr(p);
    }
}

fn having_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(HAVING_KW);
    expr(p);
    m.complete(p, HAVING_CLAUSE);
}

fn qualify_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(QUALIFY_KW);
    expr(p);
    m.complete(p, QUALIFY_CLAUSE);
}

/// SQL named window definitions: `WINDOW w AS (...), w2 AS (w ORDER BY ts)`.
///
/// There is no dedicated `WINDOW_CLAUSE` node yet, so this reuses the generic select-clause
/// formatting path through `QUALIFY_CLAUSE`; the contained definitions still use `WINDOW_SPEC`.
fn window_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(WINDOW_KW);
    window_definition(p);
    while p.eat(COMMA) {
        if at_clause_end(p) {
            break;
        }
        window_definition(p);
    }
    m.complete(p, QUALIFY_CLAUSE);
}

fn window_definition(p: &mut Parser) {
    if p.at_name() {
        name(p);
    } else {
        p.error("expected a window name");
    }
    p.expect(AS_KW);
    if p.at(L_PAREN) {
        window_spec(p);
    } else {
        p.error("expected a window specification");
    }
}

fn order_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(ORDER_KW);
    p.expect(BY_KW);
    order_by_item(p);
    while p.eat(COMMA) {
        order_by_item(p);
    }
    m.complete(p, ORDER_BY_CLAUSE);
}

fn order_by_item(p: &mut Parser) {
    let m = p.start();
    expr(p);
    if p.at(ASC_KW) || p.at(DESC_KW) {
        p.bump_any();
    }
    if p.at(NULLS_KW) {
        p.bump(NULLS_KW);
        if p.at(FIRST_KW) || p.at(LAST_KW) {
            p.bump_any();
        } else {
            p.error("expected FIRST or LAST after NULLS");
        }
    }
    m.complete(p, ORDER_BY_ITEM);
}

fn limit_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(LIMIT_KW);
    expr(p);
    m.complete(p, LIMIT_CLAUSE);
}

fn offset_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(OFFSET_KW);
    expr(p);
    m.complete(p, OFFSET_CLAUSE);
}

fn fetch_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(FETCH_KW);
    p.eat(FIRST_KW);
    expr(p);
    if p.at(ROW_KW) || p.at(ROWS_KW) {
        p.bump_any();
    } else {
        p.error("expected ROW or ROWS after FETCH count");
    }
    // `ONLY` is contextual and intentionally not reserved globally.
    if p.at_name() {
        p.bump_as(CONTEXTUAL_KEYWORD);
    } else {
        p.error("expected ONLY after FETCH ... ROWS");
    }
    m.complete(p, LIMIT_CLAUSE);
}

// ---- names ----

fn name(p: &mut Parser) {
    let m = p.start();
    if p.at_name() {
        p.bump_any();
    } else {
        p.error("expected a name");
    }
    m.complete(p, NAME);
}

fn name_ref(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    if p.at_name() {
        p.bump_any();
        while p.at(DOT) {
            p.bump(DOT);
            if p.at_name() {
                p.bump_any();
            } else if p.at(STAR) {
                p.bump(STAR); // qualified star: t.*
                break;
            } else {
                p.error("expected a name after '.'");
                break;
            }
        }
    } else {
        p.error("expected a name");
    }
    m.complete(p, NAME_REF)
}

fn column_list(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    if p.at_name() {
        name(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            name(p);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, COLUMN_LIST);
}

// ---- expressions (Pratt) ----

fn at_expr_start(p: &Parser) -> bool {
    p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(STRING)
        || p.at(DOLLAR_STRING)
        || p.at(TRUE_KW)
        || p.at(FALSE_KW)
        || p.at(NULL_KW)
        || p.at(VARIABLE)
        || p.at(QUESTION)
        || p.at(COLON)
        || p.at(L_BRACKET)
        || p.at(L_BRACE)
        || p.at(L_PAREN)
        || p.at(MINUS)
        || p.at(PLUS)
        || p.at(NOT_KW)
        || (p.at(EXISTS_KW) && p.nth_at(1, L_PAREN))
        || p.at(CASE_KW)
        || p.at(CAST_KW)
        || p.at(TRY_CAST_KW)
        || p.at(FLATTEN_KW)
        || at_interval_literal_start(p)
        || p.at_name()
        || at_keyword_call_name(p) // a keyword used as a function name: first(x)
}

pub(crate) fn expr(p: &mut Parser) {
    expr_bp(p, 0);
}

fn expr_bp(p: &mut Parser, min_bp: u8) -> Option<CompletedMarker> {
    let mut lhs = lhs(p)?;
    loop {
        // Postfix operators bind tightest and always apply when present.
        if p.at(L_PAREN) {
            let m = lhs.precede(p);
            arg_list(p);
            lhs = m.complete(p, CALL_EXPR);
            continue;
        }
        if p.at(L_BRACKET) {
            let m = lhs.precede(p);
            p.bump(L_BRACKET);
            expr(p);
            p.expect(R_BRACKET);
            lhs = m.complete(p, INDEX_EXPR);
            continue;
        }
        if p.at(COLON2) {
            let m = lhs.precede(p);
            p.bump(COLON2);
            type_name(p);
            lhs = m.complete(p, CAST_EXPR);
            continue;
        }
        // Semi-structured / VARIANT path access: `col:path.to.field`, `col:a[0]:b`.
        if p.at(COLON) {
            let m = lhs.precede(p);
            json_path(p);
            lhs = m.complete(p, JSON_ACCESS);
            continue;
        }
        // Window functions: `f(...) OVER (...)` or `f(...) OVER window_name`.
        if p.at(OVER_KW) {
            let m = lhs.precede(p);
            p.bump(OVER_KW);
            if p.at(L_PAREN) {
                window_spec(p);
            } else if p.at_name() {
                name_ref(p);
            } else {
                p.error("expected a window specification");
            }
            lhs = m.complete(p, WINDOW_EXPR);
            continue;
        }
        // Ordered-set aggregates: `LISTAGG(x, ',') WITHIN GROUP (ORDER BY x)`.
        if p.at(WITHIN_KW) {
            let m = lhs.precede(p);
            p.bump(WITHIN_KW);
            p.expect(GROUP_KW);
            p.expect(L_PAREN);
            if p.at(ORDER_KW) {
                order_by_clause(p);
            }
            p.expect(R_PAREN);
            lhs = m.complete(p, WITHIN_GROUP);
            continue;
        }

        // Databricks higher-order-function lambda: `x -> expr` / `(x, y) -> expr`.
        if p.dialect().supports_lambda_expr() && p.at(ARROW) {
            if 0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.bump(ARROW);
            expr_bp(p, 1);
            lhs = m.complete(p, LAMBDA_EXPR);
            continue;
        }

        // Compound predicates, all at comparison precedence.
        if p.at(IS_KW) {
            if BP_CMP.0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.bump(IS_KW);
            p.eat(NOT_KW);
            is_rhs(p);
            lhs = m.complete(p, IS_EXPR);
            continue;
        }
        let neg = p.at(NOT_KW);
        if p.at(BETWEEN_KW) || (neg && p.nth_at(1, BETWEEN_KW)) {
            if BP_CMP.0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.eat(NOT_KW);
            p.bump(BETWEEN_KW);
            expr_bp(p, BP_CMP.1); // first bound (won't swallow the AND)
            p.expect(AND_KW);
            expr_bp(p, BP_CMP.1); // second bound
            lhs = m.complete(p, BETWEEN_EXPR);
            continue;
        }
        if p.at(IN_KW) || (neg && p.nth_at(1, IN_KW)) {
            if BP_CMP.0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.eat(NOT_KW);
            p.bump(IN_KW);
            in_rhs(p);
            lhs = m.complete(p, IN_EXPR);
            continue;
        }
        if at_like_predicate(p) || (neg && at_like_predicate_after_not(p)) {
            if BP_CMP.0 < min_bp {
                break;
            }
            let m = lhs.precede(p);
            p.eat(NOT_KW);
            p.bump_any(); // LIKE / ILIKE / RLIKE / REGEXP
            like_rhs(p);
            lhs = m.complete(p, BIN_EXPR);
            continue;
        }

        // Generic infix binary operators.
        let (lbp, rbp) = match infix_bp(p) {
            Some(bp) => bp,
            None => break,
        };
        if lbp < min_bp {
            break;
        }
        let m = lhs.precede(p);
        p.bump_any(); // the operator
        expr_bp(p, rbp);
        lhs = m.complete(p, BIN_EXPR);
    }
    Some(lhs)
}

fn at_like_predicate(p: &Parser) -> bool {
    p.at(LIKE_KW) || p.at(ILIKE_KW) || p.at(RLIKE_KW) || p.at(REGEXP_KW)
}

fn at_like_predicate_after_not(p: &Parser) -> bool {
    p.nth_at(1, LIKE_KW) || p.nth_at(1, ILIKE_KW) || p.nth_at(1, RLIKE_KW) || p.nth_at(1, REGEXP_KW)
}

fn like_rhs(p: &mut Parser) {
    if p.at(ANY_KW) || p.at(ALL_KW) {
        p.bump_any();
        p.expect(L_PAREN);
        if !p.at(R_PAREN) {
            expr_list(p);
        }
        p.expect(R_PAREN);
    } else {
        expr_bp(p, BP_CMP.1);
    }
}

fn lhs(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at(NOT_KW) {
        let m = p.start();
        p.bump(NOT_KW);
        expr_bp(p, BP_PREFIX_NOT);
        return Some(m.complete(p, PREFIX_EXPR));
    }
    if p.at(PRIOR_KW) {
        // `CONNECT BY PRIOR <col> = <col>`: PRIOR is a tight unary prefix on a value.
        let m = p.start();
        p.bump(PRIOR_KW);
        expr_bp(p, BP_PREFIX_NEG);
        return Some(m.complete(p, PREFIX_EXPR));
    }
    if p.at(MINUS) || p.at(PLUS) {
        let m = p.start();
        p.bump_any();
        expr_bp(p, BP_PREFIX_NEG);
        return Some(m.complete(p, PREFIX_EXPR));
    }
    primary(p)
}

fn primary(p: &mut Parser) -> Option<CompletedMarker> {
    let cm = if p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(STRING)
        || p.at(DOLLAR_STRING)
        || p.at(TRUE_KW)
        || p.at(FALSE_KW)
        || p.at(NULL_KW)
        || p.at(VARIABLE)
    {
        let m = p.start();
        p.bump_any();
        m.complete(p, LITERAL)
    } else if p.at(QUESTION) || p.at(COLON) {
        bind_marker(p)
    } else if at_interval_literal_start(p) {
        interval_literal(p)
    } else if p.at(EXISTS_KW) && p.nth_at(1, L_PAREN) {
        let m = p.start();
        p.bump(EXISTS_KW);
        subquery(p);
        m.complete(p, EXISTS_EXPR)
    } else if p.at(L_PAREN) && (p.nth_at(1, SELECT_KW) || p.nth_at(1, WITH_KW)) {
        subquery(p) // scalar subquery
    } else if p.dialect().supports_lambda_expr() && at_parenthesized_lambda_params(p) {
        lambda_params(p)
    } else if p.at(L_PAREN) {
        let m = p.start();
        p.bump(L_PAREN);
        expr(p);
        p.expect(R_PAREN);
        m.complete(p, PAREN_EXPR)
    } else if p.at(L_BRACKET) {
        array_literal(p)
    } else if p.at(L_BRACE) {
        object_literal(p)
    } else if p.at(CASE_KW) {
        case_expr(p)
    } else if p.at(CAST_KW) || p.at(TRY_CAST_KW) {
        cast_fn_expr(p)
    } else if p.at(FLATTEN_KW) {
        // FLATTEN is a keyword but acts as a table/regular function; treat it as a callable name.
        let m = p.start();
        p.bump(FLATTEN_KW);
        m.complete(p, NAME_REF)
    } else if at_keyword_call_name(p) {
        // A keyword-spelled word used as a function name (`first(x)`, `last(x)`, `left(s, 2)`):
        // tag it as a plain name so the postfix `(` makes it a CALL_EXPR and it formats like any
        // other call (lower-case, hugging its parens).
        let m = p.start();
        p.bump_as(IDENT);
        m.complete(p, NAME_REF)
    } else if p.at_name() {
        name_ref(p)
    } else {
        p.err_and_bump("expected an expression");
        return None;
    };
    Some(cm)
}

fn bind_marker(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    if p.at(QUESTION) {
        p.bump(QUESTION);
    } else {
        p.bump(COLON);
        if p.at_name() {
            name(p);
        } else {
            p.error("expected a bind variable name after ':'");
        }
    }
    m.complete(p, BIND_MARKER)
}

fn interval_literal(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // INTERVAL
    if p.at(STRING) {
        p.bump(STRING);
        interval_unit_range(p);
    } else if at_interval_component_start(p) {
        interval_component(p);
        while at_interval_component_start(p) {
            interval_component(p);
        }
    } else {
        p.error("expected an interval literal value");
    }
    m.complete(p, INTERVAL_LITERAL)
}

fn at_interval_literal_start(p: &Parser) -> bool {
    p.nth_contextual(0, ContextualKeyword::Interval)
        && (p.nth_at(1, STRING)
            || p.nth_at(1, INT_NUMBER)
            || p.nth_at(1, FLOAT_NUMBER)
            || ((p.nth_at(1, PLUS) || p.nth_at(1, MINUS))
                && (p.nth_at(2, INT_NUMBER) || p.nth_at(2, FLOAT_NUMBER) || p.nth_at(2, STRING))))
}

fn at_interval_component_start(p: &Parser) -> bool {
    p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(STRING)
        || ((p.at(PLUS) || p.at(MINUS))
            && (p.nth_at(1, INT_NUMBER) || p.nth_at(1, FLOAT_NUMBER) || p.nth_at(1, STRING)))
}

fn interval_component(p: &mut Parser) {
    if !p.eat(PLUS) {
        p.eat(MINUS);
    }
    if p.at(INT_NUMBER) || p.at(FLOAT_NUMBER) || p.at(STRING) {
        p.bump_any();
    } else {
        p.error("expected an interval literal value");
        return;
    }
    interval_unit_range(p);
}

fn interval_unit_range(p: &mut Parser) {
    if at_interval_unit(p) {
        p.bump_as(CONTEXTUAL_KEYWORD);
        if p.nth_contextual(0, ContextualKeyword::To) {
            p.bump_as(CONTEXTUAL_KEYWORD);
            if at_interval_unit(p) {
                p.bump_as(CONTEXTUAL_KEYWORD);
            } else {
                p.error("expected an interval unit after TO");
            }
        }
    }
}

fn at_interval_unit(p: &Parser) -> bool {
    p.nth_any_contextual(0, INTERVAL_UNIT_CONTEXTUAL_WORDS)
}

fn array_literal(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(L_BRACKET);
    if !p.at(R_BRACKET) {
        expr(p);
        while p.eat(COMMA) {
            if p.at(R_BRACKET) {
                break;
            }
            expr(p);
        }
    }
    p.expect(R_BRACKET);
    m.complete(p, ARRAY_LITERAL)
}

fn object_literal(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(L_BRACE);
    if !p.at(R_BRACE) {
        object_field(p);
        while p.eat(COMMA) {
            if p.at(R_BRACE) {
                break;
            }
            object_field(p);
        }
    }
    p.expect(R_BRACE);
    m.complete(p, OBJECT_LITERAL)
}

fn object_field(p: &mut Parser) {
    let m = p.start();
    object_key(p);
    p.expect(COLON);
    if at_expr_start(p) {
        expr(p);
    } else {
        p.error("expected an object literal value");
    }
    m.complete(p, OBJECT_FIELD);
}

fn object_key(p: &mut Parser) {
    if p.at(STRING)
        || p.at(INT_NUMBER)
        || p.at(FLOAT_NUMBER)
        || p.at(TRUE_KW)
        || p.at(FALSE_KW)
        || p.at(NULL_KW)
    {
        let m = p.start();
        p.bump_any();
        m.complete(p, LITERAL);
    } else if p.at_name() {
        name_ref(p);
    } else {
        p.error("expected an object literal key");
    }
}

fn at_parenthesized_lambda_params(p: &Parser) -> bool {
    if !p.at(L_PAREN) {
        return false;
    }
    let mut depth = 0u32;
    for i in 0..48 {
        if p.nth_at(i, EOF) {
            return false;
        }
        if p.nth_at(i, L_PAREN) {
            depth += 1;
        } else if p.nth_at(i, R_PAREN) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return p.nth_at(i + 1, ARROW);
            }
        }
    }
    false
}

fn lambda_params(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(L_PAREN);
    if !p.at(R_PAREN) {
        if p.at_name() {
            name(p);
        } else {
            p.error("expected a lambda parameter");
        }
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            name(p);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, LAMBDA_PARAMS)
}

fn at_keyword_call_name(p: &Parser) -> bool {
    (p.at(FIRST_KW) || p.at(LAST_KW) || p.at(LEFT_KW) || p.at(RIGHT_KW)) && p.nth_at(1, L_PAREN)
}

fn is_rhs(p: &mut Parser) {
    if p.at(DISTINCT_KW) {
        // `a IS [NOT] DISTINCT FROM b`
        p.bump(DISTINCT_KW);
        p.expect(FROM_KW);
        expr_bp(p, BP_CMP.1);
    } else if p.at(NULL_KW) || p.at(TRUE_KW) || p.at(FALSE_KW) {
        let m = p.start();
        p.bump_any();
        m.complete(p, LITERAL);
    } else {
        expr_bp(p, BP_CMP.1);
    }
}

fn in_rhs(p: &mut Parser) {
    if !p.eat(L_PAREN) {
        p.error("expected '(' after IN");
        return;
    }
    if p.at(SELECT_KW) || p.at(WITH_KW) {
        query_expr(p);
    } else if !p.at(R_PAREN) {
        expr_list(p);
    }
    p.expect(R_PAREN);
}

fn expr_list(p: &mut Parser) {
    let m = p.start();
    expr(p);
    while p.eat(COMMA) {
        if p.at(R_PAREN) {
            break;
        }
        expr(p);
    }
    m.complete(p, EXPR_LIST);
}

fn arg_list(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    // Aggregate quantifier applying to the whole argument list: COUNT(DISTINCT x), ARRAY_AGG(ALL x).
    if p.at(DISTINCT_KW) || p.at(ALL_KW) {
        p.bump_any();
    }
    if !p.at(R_PAREN) {
        arg(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            arg(p);
        }
    }
    if !p.eat(R_PAREN) {
        p.err_and_bump(format!("expected {}", R_PAREN.describe()));
    }
    m.complete(p, ARG_LIST);
}

fn arg(p: &mut Parser) {
    if p.at(STAR) {
        let m = p.start();
        p.bump(STAR);
        m.complete(p, STAR_EXPR);
    } else if p.at_ident_like() && p.nth_at(1, FAT_ARROW) {
        // Named argument: `name => value` (e.g. FLATTEN(INPUT => col, OUTER => TRUE)).
        let m = p.start();
        p.bump_any(); // the argument name
        p.bump(FAT_ARROW);
        expr(p);
        m.complete(p, NAMED_ARG);
    } else if at_expr_start(p) {
        expr(p);
    } else {
        p.error("expected an argument");
    }
}

fn type_name(p: &mut Parser) {
    let m = p.start();
    if p.at_name() {
        p.bump_any();
        if p.eat(L_PAREN) {
            while !p.at(R_PAREN) && !p.at_eof() {
                p.bump_any();
            }
            p.expect(R_PAREN);
        }
    } else {
        p.error("expected a type name");
    }
    m.complete(p, TYPE_NAME);
}

fn window_spec(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    if p.at_name() {
        name_ref(p); // base window name in `WINDOW w AS (base ORDER BY ts)`
    }
    if p.at(PARTITION_KW) {
        partition_by_clause(p);
    }
    if p.at(ORDER_KW) {
        order_by_clause(p);
    }
    if p.at(ROWS_KW) || p.at(RANGE_KW) {
        window_frame(p);
    }
    p.expect(R_PAREN);
    m.complete(p, WINDOW_SPEC);
}

fn partition_by_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(PARTITION_KW);
    p.expect(BY_KW);
    expr(p);
    while p.eat(COMMA) {
        expr(p);
    }
    m.complete(p, PARTITION_BY_CLAUSE);
}

fn window_frame(p: &mut Parser) {
    let m = p.start();
    p.bump_any(); // ROWS or RANGE
    if p.eat(BETWEEN_KW) {
        frame_bound(p);
        p.expect(AND_KW);
        frame_bound(p);
    } else {
        frame_bound(p);
    }
    m.complete(p, WINDOW_FRAME);
}

fn frame_bound(p: &mut Parser) {
    if p.at(UNBOUNDED_KW) {
        p.bump(UNBOUNDED_KW);
        if p.at(PRECEDING_KW) || p.at(FOLLOWING_KW) {
            p.bump_any();
        } else {
            p.error("expected PRECEDING or FOLLOWING");
        }
    } else if p.at(CURRENT_KW) {
        p.bump(CURRENT_KW);
        p.expect(ROW_KW);
    } else {
        expr(p);
        if p.at(PRECEDING_KW) || p.at(FOLLOWING_KW) {
            p.bump_any();
        } else {
            p.error("expected PRECEDING or FOLLOWING");
        }
    }
}

fn case_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(CASE_KW);
    if !p.at(WHEN_KW) {
        expr(p); // simple CASE: the operand before the first WHEN
    }
    while p.at(WHEN_KW) {
        let arm = p.start();
        p.bump(WHEN_KW);
        expr(p);
        p.expect(THEN_KW);
        expr(p);
        arm.complete(p, CASE_WHEN);
    }
    if p.eat(ELSE_KW) {
        expr(p);
    }
    p.expect(END_KW);
    m.complete(p, CASE_EXPR)
}

fn cast_fn_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump_any(); // CAST or TRY_CAST
    p.expect(L_PAREN);
    expr(p);
    p.expect(AS_KW);
    type_name(p);
    p.expect(R_PAREN);
    m.complete(p, CAST_EXPR)
}

/// Semi-structured path tail after a leading `:` (already on the `:` token).
fn json_path(p: &mut Parser) {
    p.bump(COLON);
    json_path_segment(p);
    loop {
        if p.at(DOT) {
            p.bump(DOT);
            json_path_segment(p);
        } else if p.at(COLON) {
            p.bump(COLON);
            json_path_segment(p);
        } else if p.at(L_BRACKET) {
            p.bump(L_BRACKET);
            expr(p);
            p.expect(R_BRACKET);
        } else {
            break;
        }
    }
}

fn json_path_segment(p: &mut Parser) {
    // A path key may be any bare word, including one that spells a keyword (`payload:order`). Tag it
    // as a plain IDENT so its case is preserved — semi-structured keys are case-sensitive.
    if p.at_ident_like() {
        p.bump_as(IDENT);
    } else if p.at(STRING) {
        p.bump_any();
    } else {
        p.error("expected a path segment after ':'");
    }
}

fn values_clause(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(VALUES_KW);
    values_row(p);
    while p.eat(COMMA) {
        values_row(p);
    }
    m.complete(p, VALUES_CLAUSE)
}

fn values_row(p: &mut Parser) {
    let m = p.start();
    p.expect(L_PAREN);
    if !p.at(R_PAREN) {
        expr(p);
        while p.eat(COMMA) {
            if p.at(R_PAREN) {
                break;
            }
            expr(p);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, VALUES_ROW);
}

fn infix_bp(p: &Parser) -> Option<(u8, u8)> {
    let bp = if p.at(OR_KW) {
        BP_OR
    } else if p.at(AND_KW) {
        BP_AND
    } else if p.at(EQ)
        || p.at(NEQ)
        || p.at(LT)
        || p.at(LTE)
        || p.at(GT)
        || p.at(GTE)
        || p.at(LIKE_KW)
        || p.at(ILIKE_KW)
    {
        BP_CMP
    } else if p.at(CONCAT) {
        BP_CONCAT
    } else if p.at(PLUS) || p.at(MINUS) {
        BP_ADD
    } else if p.at(STAR) || p.at(SLASH) || p.at(PERCENT) {
        BP_MUL
    } else {
        return None;
    };
    Some(bp)
}
