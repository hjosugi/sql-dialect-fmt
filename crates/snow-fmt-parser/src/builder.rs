//! Replay the event stream into a `rowan` green tree, re-inserting trivia so the CST is
//! byte-exact-lossless (concatenating every token's text reproduces the source).

use rowan::{GreenNode, GreenNodeBuilder};
use snow_fmt_lexer::Token;
use snow_fmt_syntax::SyntaxKind;

use crate::event::Event;

#[inline]
fn to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
    rowan::SyntaxKind(kind.to_u16())
}

pub(crate) fn build_tree(all: &[Token<'_>], events: Vec<Event>) -> GreenNode {
    let mut builder = GreenNodeBuilder::new();
    let mut idx = 0usize; // cursor into `all` (includes trivia)
    let mut depth = 0u32;

    for event in events {
        match event {
            Event::Open { kind } => {
                builder.start_node(to_raw(kind));
                depth += 1;
            }
            Event::Close => {
                depth -= 1;
                // At the root close, flush any trailing trivia so nothing is lost.
                if depth == 0 {
                    while idx < all.len() {
                        let t = &all[idx];
                        builder.token(to_raw(t.kind), t.text);
                        idx += 1;
                    }
                }
                builder.finish_node();
            }
            Event::Tombstone => {} // an abandoned wrapper: nothing to emit
            Event::Advance { kind } => {
                // Emit leading trivia, then the meaningful token tagged with the event's kind.
                while idx < all.len() && all[idx].kind.is_trivia() {
                    let t = &all[idx];
                    builder.token(to_raw(t.kind), t.text);
                    idx += 1;
                }
                let text = all[idx].text;
                builder.token(to_raw(kind), text);
                idx += 1;
            }
        }
    }

    builder.finish()
}
