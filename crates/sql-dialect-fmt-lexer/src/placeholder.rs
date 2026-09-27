//! Custom placeholder matchers (the `param_types.custom` option).
//!
//! The core lexer is deliberately dependency-free and linear-time, so it does not compile patterns
//! itself. Instead, callers that want arbitrary placeholder syntaxes (for example a regex over
//! templating tags) implement [`PlaceholderMatcher`] and pass instances through
//! [`LexOptions::with_custom_placeholders`](crate::LexOptions::with_custom_placeholders). The lexer
//! tries each matcher at every token start and, on a match, emits a single atomic `PLACEHOLDER`
//! token — keeping the stream lossless without the lexer depending on a regex engine.

/// A user-supplied placeholder recognizer.
///
/// `Debug` is required so `LexOptions` can keep deriving `Debug`.
pub trait PlaceholderMatcher: std::fmt::Debug {
    /// If a placeholder begins at byte `at` in `input`, return its length in bytes (must be `> 0`).
    /// Return `None` when no placeholder starts there.
    ///
    /// `at` is always a byte boundary of `input`.
    fn match_len(&self, input: &str, at: usize) -> Option<usize>;
}
