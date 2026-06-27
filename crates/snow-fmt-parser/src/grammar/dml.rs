//! DML grammar rules (`INSERT`, `UPDATE`, `DELETE`, `MERGE`).

use snow_fmt_syntax::SyntaxKind::*;

use crate::parser::{ContextualKeyword, Parser};

pub(super) fn insert_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(INSERT_KW);
    let overwrite = p.eat(OVERWRITE_KW);
    if p.at(ALL_KW) || p.at(FIRST_KW) {
        multi_table_insert(p);
    } else if overwrite
        && p.dialect().supports_delta_commands()
        && !p.at(INTO_KW)
        && (p.at(TABLE_KW) || p.at_name())
    {
        // Databricks `INSERT OVERWRITE [TABLE] t [PARTITION (...)] { <query> | VALUES ... }` — the
        // `INTO` keyword is absent and an optional `TABLE` word and `PARTITION` spec precede the
        // source. Snowflake's `INSERT OVERWRITE INTO …` keeps the `INTO` path below, unchanged.
        databricks_insert_overwrite(p);
    } else {
        // Single-table: INSERT [OVERWRITE] INTO t [(cols)] VALUES/<query>.
        p.expect(INTO_KW);
        super::name_ref(p);
        if p.at(L_PAREN) {
            super::column_list(p);
        }
        if p.at(VALUES_KW) {
            super::values_clause(p);
        } else {
            super::query_expr(p);
        }
    }
    m.complete(p, INSERT_STMT);
}

/// `INSERT OVERWRITE [TABLE] t [PARTITION ( col [= val] [, ...] )] [(cols)] { <query> | VALUES ... }`
/// (Databricks/Spark). The `TABLE` keyword and `PARTITION` spec are both optional.
fn databricks_insert_overwrite(p: &mut Parser) {
    p.eat(TABLE_KW);
    super::name_ref(p);
    if p.at(PARTITION_KW) {
        p.bump(PARTITION_KW);
        if p.at(L_PAREN) {
            super::balanced_parens(p);
        } else {
            p.error("expected '(' after PARTITION");
        }
    }
    if p.at(L_PAREN) {
        super::column_list(p);
    }
    if p.at(VALUES_KW) {
        super::values_clause(p);
    } else {
        super::query_expr(p);
    }
}

/// `INSERT [OVERWRITE] ALL <into>+ <query>` (unconditional) or
/// `INSERT [OVERWRITE] {ALL|FIRST} (WHEN <cond> THEN <into>+)+ [ELSE <into>+] <query>`.
fn multi_table_insert(p: &mut Parser) {
    p.bump_any(); // ALL or FIRST
    if p.at(WHEN_KW) {
        while p.at(WHEN_KW) {
            insert_when(p);
        }
        if p.eat(ELSE_KW) {
            while p.at(INTO_KW) {
                into_clause(p);
            }
        }
    } else {
        while p.at(INTO_KW) {
            into_clause(p);
        }
    }
    super::query_expr(p); // the source rows
}

fn insert_when(p: &mut Parser) {
    let m = p.start();
    p.bump(WHEN_KW);
    super::expr(p);
    p.expect(THEN_KW);
    while p.at(INTO_KW) {
        into_clause(p);
    }
    m.complete(p, INSERT_WHEN);
}

fn into_clause(p: &mut Parser) {
    let m = p.start();
    p.bump(INTO_KW);
    super::name_ref(p);
    if p.at(L_PAREN) {
        super::column_list(p);
    }
    if p.at(VALUES_KW) {
        super::values_clause(p);
    }
    m.complete(p, INTO_CLAUSE);
}

pub(super) fn update_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(UPDATE_KW);
    super::table_ref(p);
    set_clause(p);
    if p.at(FROM_KW) {
        super::from_clause(p);
    }
    if p.at(WHERE_KW) {
        super::where_clause(p);
    }
    m.complete(p, UPDATE_STMT);
}

pub(super) fn delete_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(DELETE_KW);
    p.expect(FROM_KW);
    super::table_ref(p);
    if p.eat(USING_KW) {
        super::table_ref(p);
        while p.eat(COMMA) {
            super::table_ref(p);
        }
    }
    if p.at(WHERE_KW) {
        super::where_clause(p);
    }
    m.complete(p, DELETE_STMT);
}

fn set_clause(p: &mut Parser) {
    let m = p.start();
    p.expect(SET_KW);
    assignment(p);
    while p.eat(COMMA) {
        assignment(p);
    }
    m.complete(p, SET_CLAUSE);
}

fn assignment(p: &mut Parser) {
    let m = p.start();
    super::name_ref(p);
    p.expect(EQ);
    super::expr(p);
    m.complete(p, ASSIGNMENT);
}

pub(super) fn merge_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(MERGE_KW);
    p.expect(INTO_KW);
    super::table_ref(p);
    p.expect(USING_KW);
    super::table_ref(p);
    p.expect(ON_KW);
    super::expr(p);
    while p.at(WHEN_KW) {
        merge_when(p);
    }
    m.complete(p, MERGE_STMT);
}

fn merge_when(p: &mut Parser) {
    let m = p.start();
    p.bump(WHEN_KW);
    p.eat(NOT_KW);
    p.expect(MATCHED_KW);
    // Databricks `WHEN NOT MATCHED BY {SOURCE|TARGET}` qualifier. `BY` is reserved; `SOURCE`/`TARGET`
    // are contextual (so they stay ordinary identifiers). Snowflake has no `BY` here, so this never
    // fires under Snowflake and its MERGE is unchanged.
    if p.dialect().supports_delta_commands() && p.at(BY_KW) {
        p.bump(BY_KW);
        if p.nth_contextual(0, ContextualKeyword::Source)
            || p.nth_contextual(0, ContextualKeyword::Target)
        {
            p.bump_as(CONTEXTUAL_KEYWORD); // SOURCE / TARGET
        } else {
            p.error("expected SOURCE or TARGET after BY");
        }
    }
    if p.eat(AND_KW) {
        super::expr(p); // WHEN [NOT] MATCHED [BY ...] AND <cond>
    }
    p.expect(THEN_KW);
    if p.at(UPDATE_KW) {
        p.bump(UPDATE_KW);
        set_clause(p);
    } else if p.at(DELETE_KW) {
        p.bump(DELETE_KW);
    } else if p.at(INSERT_KW) {
        p.bump(INSERT_KW);
        // Databricks `INSERT *` — insert all source columns.
        if p.dialect().supports_delta_commands() && p.at(STAR) {
            p.bump(STAR);
        } else {
            if p.at(L_PAREN) {
                super::column_list(p);
            }
            if p.at(VALUES_KW) {
                super::values_clause(p);
            }
        }
    } else {
        p.error("expected UPDATE, DELETE, or INSERT after THEN");
    }
    m.complete(p, MERGE_WHEN);
}
