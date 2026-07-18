//! Access-control statements (`GRANT` / `REVOKE`).

use sql_dialect_fmt_syntax::SyntaxKind::*;

use crate::parser::{ContextualKeyword, Parser};

use super::{at_stmt_terminator, name_ref};

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

// ---- access control: GRANT / REVOKE ----

/// `GRANT <privileges> ON <object> TO [ROLE|USER] <name> [WITH GRANT OPTION]`, plus the role/share
/// grant shapes (`GRANT ROLE r TO …`, `GRANT <role> TO USER u`). The privilege list, the `ON …`
/// securable, and the `TO …` grantee are each their own node so the formatter can stack them; a
/// trailing `WITH GRANT OPTION` (or any unmodeled tail) is kept as inline tokens.
pub(super) fn grant_stmt(p: &mut Parser) {
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
pub(super) fn revoke_stmt(p: &mut Parser) {
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
