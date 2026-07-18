//! DDL statements: `CREATE` (tables, views, routines, policies, and the named-object kinds),
//! `DROP`, `ALTER`, and `COMMENT ON`.

use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{ContextualKeyword, Parser};

use super::{
    at_block_start, at_stmt_terminator, balanced_parens, balanced_token_run_until, block_stmt,
    call_stmt, column_list, dml, expr, name_ref, query_expr, type_name,
};

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

// ---- DDL (Phase 7) ----

/// `IF [NOT] EXISTS`, tolerated wherever Snowflake allows it.
fn if_exists_clause(p: &mut Parser) {
    if p.at(IF_KW) {
        p.bump(IF_KW);
        p.eat(NOT_KW);
        p.eat(EXISTS_KW);
    }
}

pub(super) fn create_stmt(p: &mut Parser) {
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

/// Object kinds this rule does not specialize. Their open-ended header stays a lenient token run,
/// but an unambiguous `AS <query>` tail is parsed structurally so new/preview CTAS-like objects get
/// the same query formatting as the object kinds known to this parser. Other `AS` surfaces (for
/// example a policy's `AS (<args>) RETURNS ...`) remain verbatim and are not mistaken for CTAS.
fn create_other(p: &mut Parser) {
    while !at_stmt_terminator(p) {
        if at_create_query_body(p) {
            p.bump(AS_KW);
            query_expr(p);
            return;
        }
        p.bump_any();
    }
}

/// An `AS` whose next token(s) unambiguously start a query. Keeping this narrower than
/// [`at_create_body`] prevents an unknown policy/object signature beginning `AS (...)` from being
/// parsed as a parenthesized query.
fn at_create_query_body(p: &Parser) -> bool {
    p.at(AS_KW)
        && (p.nth_at(1, SELECT_KW)
            || p.nth_at(1, WITH_KW)
            || p.nth_at(1, VALUES_KW)
            || (p.nth_at(1, L_PAREN)
                && (p.nth_at(2, SELECT_KW) || p.nth_at(2, WITH_KW) || p.nth_at(2, VALUES_KW))))
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
/// `RETURNS <type>` and `LANGUAGE <language>` are structured signature clauses; the remaining
/// open-ended option surface stays lenient. Delimited bodies (`$$ … $$` or a quoted string) remain
/// a single token, while unquoted Snowflake Scripting bodies (`AS BEGIN … END` /
/// `AS DECLARE … BEGIN … END`) reuse the block parser so inner `;` separators never split the
/// outer routine statement.
fn create_routine(p: &mut Parser) {
    p.bump_any(); // PROCEDURE or FUNCTION
    name_ref(p);
    if p.at(L_PAREN) {
        column_def_list(p); // parameter list, parsed leniently like column defs
    }
    // Structure the first RETURNS clause (the return type) while leaving a later
    // `RETURNS NULL ON NULL INPUT` behavior phrase in the lenient option run.
    let mut seen_returns = false;
    while !at_routine_body(p) && !at_stmt_terminator(p) {
        if !seen_returns && p.at(RETURNS_KW) {
            routine_returns_clause(p);
            seen_returns = true;
        } else if p.at(LANGUAGE_KW) {
            routine_language_clause(p);
        } else {
            p.bump_any();
        }
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

fn routine_returns_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(RETURNS_KW);
    if p.at(TABLE_KW) {
        p.bump(TABLE_KW);
        if p.at(L_PAREN) {
            balanced_parens(p);
        }
    } else if !p.at(LANGUAGE_KW) && p.at_name() {
        type_name(p);
    } else {
        p.error("expected a routine return type");
    }
    if p.eat(NOT_KW) {
        p.expect(NULL_KW);
    }
    m.complete(p, ROUTINE_RETURNS_CLAUSE);
}

fn routine_language_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(LANGUAGE_KW);
    if p.at_name() || p.at_keyword() {
        p.bump_any();
    } else {
        p.error("expected a routine language");
    }
    m.complete(p, ROUTINE_LANGUAGE_CLAUSE);
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
    // `CREATE TABLE <name> [SHALLOW|DEEP] CLONE <source> [<time-travel>]` — no CTAS.
    if p.dialect().supports_delta_table_options()
        && (p.nth_contextual(0, ContextualKeyword::Shallow)
            || p.nth_contextual(0, ContextualKeyword::Deep))
        && p.nth_contextual(1, ContextualKeyword::Clone)
    {
        p.bump_as(CONTEXTUAL_KEYWORD);
    }
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

pub(super) fn drop_stmt(p: &mut Parser) {
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
pub(super) fn alter_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(ALTER_KW);
    while !at_stmt_terminator(p) {
        p.bump_any();
    }
    m.complete(p, ALTER_STMT);
}

/// At a `COMMENT ON …` statement. `comment` is a contextual keyword recognized only before `ON`, so
/// the very common `comment` column/identifier is never mistaken for this statement.
pub(super) fn at_comment_stmt(p: &Parser) -> bool {
    p.nth_contextual(0, ContextualKeyword::Comment) && p.nth_at(1, ON_KW)
}

/// `COMMENT ON <object> IS '<text>'` (or `COMMENT IF EXISTS …`). Parsed leniently as a flat token
/// run after up-casing the contextual `COMMENT`, so it round-trips and formats inline.
pub(super) fn comment_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump_as(CONTEXTUAL_KEYWORD); // COMMENT
    while !at_stmt_terminator(p) {
        p.bump_any();
    }
    m.complete(p, COMMENT_STMT);
}
