//! Snowflake client/stage file-operation statements (`PUT`, `GET`, `LIST`, `REMOVE`).

use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{ContextualKeyword, Parser};

const COMMANDS: &[ContextualKeyword] = &[
    ContextualKeyword::Put,
    ContextualKeyword::Get,
    ContextualKeyword::List,
    ContextualKeyword::Remove,
];

pub(super) fn at_stage_file_stmt(p: &Parser) -> bool {
    p.dialect().supports_stage_refs() && p.nth_any_contextual(0, COMMANDS)
}

/// Snowflake client/stage file operations:
///
/// * `PUT file://<local-path> @<stage> [option = value ...]`
/// * `GET @<stage> file://<local-path> [option = value ...]`
/// * `{ LIST | REMOVE } @<stage> [PATTERN = '<regex>']`
///
/// The command and option names are contextual so they remain ordinary identifiers outside this
/// statement position. Locations reuse [`COPY_LOCATION`] / [`STAGE_REF`] nodes because they need
/// the same verbatim path rendering as COPY INTO.
pub(super) fn stage_file_stmt(p: &mut Parser) {
    let m = p.start();
    let is_put = p.nth_contextual(0, ContextualKeyword::Put);
    let is_get = p.nth_contextual(0, ContextualKeyword::Get);
    p.bump_as(CONTEXTUAL_KEYWORD);

    if is_put {
        local_file_location(p);
        stage_location(p);
    } else if is_get {
        stage_location(p);
        local_file_location(p);
    } else {
        stage_location(p); // LIST / REMOVE
    }

    while !super::at_stmt_terminator(p) {
        if p.nth_at(1, EQ) {
            super::copy_option(p);
        } else {
            p.err_and_bump("expected a stage file option in 'name = value' form");
        }
    }
    m.complete(p, STAGE_FILE_STMT);
}

fn local_file_location(p: &mut Parser) {
    if !p.at(FILE_URI) && !p.at(STRING) {
        p.error("expected a file:// URI");
        return;
    }
    let m = p.start();
    p.bump_any();
    m.complete(p, COPY_LOCATION);
}

fn stage_location(p: &mut Parser) {
    if !p.at(AT) && !p.at(STRING) {
        p.error("expected a stage location");
        return;
    }
    let m = p.start();
    if p.at(AT) {
        super::stage_ref(p);
    } else {
        p.bump(STRING); // quoted stage/path, e.g. '@"my stage"/incoming/'
    }
    m.complete(p, COPY_LOCATION);
}
