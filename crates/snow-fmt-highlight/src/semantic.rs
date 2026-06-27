//! LSP semantic-token mapping for the lexical highlighter.
//!
//! The [`crate::highlight`] pass produces [`HighlightKind`]s with byte ranges. Editors that speak
//! the Language Server Protocol want *semantic tokens*: each significant token tagged with a
//! standard token **type** (`keyword`, `string`, `number`, …) and a bitset of **modifiers**, then
//! delta-encoded as `(deltaLine, deltaStartChar, length, tokenType, modifiers)` quintuples.
//!
//! This module is transport-free and depends on nothing but the lexer/syntax crates — the
//! `snow-fmt-lsp` binary maps these onto `lsp_types`, but the mapping itself (and its tests) live
//! here so the highlighter and the editor adapter can never disagree on what a kind *means*.
//!
//! It also understands [`Injection`]s: a `$$ … $$` body may embed JavaScript, Python, Java, Scala,
//! or SQL (per a `LANGUAGE` clause). Tokens that fall inside an injection region are tagged so an
//! editor can either re-highlight them with the embedded grammar or shade the whole body.

use crate::{highlight, HighlightKind, HighlightToken};

/// The standard LSP semantic token types this highlighter emits.
///
/// The variant order is the *legend*: [`SemanticTokenType::index`] is the position an editor uses
/// to decode the `tokenType` field, so adding a variant must only ever append.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticTokenType {
    Keyword,
    Type,
    Variable,
    String,
    Number,
    /// Bind / positional parameters (`$1`, `:name`, `?`) and session variables (`$name`).
    Parameter,
    Operator,
    Comment,
    /// Stage references (`@stage`, `@~`, `@%table`) — `namespace` is the closest standard type.
    Namespace,
    /// Built-in function names, currently used for Snowflake Cortex / AISQL functions.
    Function,
}

impl SemanticTokenType {
    /// The full legend, in `index` order. This is the contract an editor declares in its server
    /// capabilities; the `tokenType` field of every emitted token indexes into this slice.
    pub const LEGEND: &'static [SemanticTokenType] = &[
        SemanticTokenType::Keyword,
        SemanticTokenType::Type,
        SemanticTokenType::Variable,
        SemanticTokenType::String,
        SemanticTokenType::Number,
        SemanticTokenType::Parameter,
        SemanticTokenType::Operator,
        SemanticTokenType::Comment,
        SemanticTokenType::Namespace,
        SemanticTokenType::Function,
    ];

    /// The canonical LSP `SemanticTokenType` string (matches `lsp_types::SemanticTokenType`).
    pub const fn name(self) -> &'static str {
        match self {
            SemanticTokenType::Keyword => "keyword",
            SemanticTokenType::Type => "type",
            SemanticTokenType::Variable => "variable",
            SemanticTokenType::String => "string",
            SemanticTokenType::Number => "number",
            SemanticTokenType::Parameter => "parameter",
            SemanticTokenType::Operator => "operator",
            SemanticTokenType::Comment => "comment",
            SemanticTokenType::Namespace => "namespace",
            SemanticTokenType::Function => "function",
        }
    }

    /// The legend index — the value carried in a token's `token_type` field.
    pub const fn index(self) -> u32 {
        match self {
            SemanticTokenType::Keyword => 0,
            SemanticTokenType::Type => 1,
            SemanticTokenType::Variable => 2,
            SemanticTokenType::String => 3,
            SemanticTokenType::Number => 4,
            SemanticTokenType::Parameter => 5,
            SemanticTokenType::Operator => 6,
            SemanticTokenType::Comment => 7,
            SemanticTokenType::Namespace => 8,
            SemanticTokenType::Function => 9,
        }
    }
}

/// LSP semantic token modifiers, as a bitset. Only the few the lexical layer can justify are
/// modelled; `bits()` produces the `tokenModifiers` value an editor decodes against its legend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SemanticTokenModifiers(u32);

impl SemanticTokenModifiers {
    pub const NONE: SemanticTokenModifiers = SemanticTokenModifiers(0);
    /// `documentation` — block/line comments.
    pub const DOCUMENTATION: SemanticTokenModifiers = SemanticTokenModifiers(1 << 0);
    /// `defaultLibrary` — built-in types and keywords are part of the language, not user code.
    pub const DEFAULT_LIBRARY: SemanticTokenModifiers = SemanticTokenModifiers(1 << 1);

    /// The modifier legend, in bit order; an editor declares this in its capabilities.
    pub const LEGEND: &'static [&'static str] = &["documentation", "defaultLibrary"];

    /// The raw bitset.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Whether `other`'s bits are all set in `self`.
    pub const fn contains(self, other: SemanticTokenModifiers) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for SemanticTokenModifiers {
    type Output = SemanticTokenModifiers;
    fn bitor(self, rhs: SemanticTokenModifiers) -> SemanticTokenModifiers {
        SemanticTokenModifiers(self.0 | rhs.0)
    }
}

/// The LSP token type + modifiers for a highlight kind, or `None` for kinds that carry no semantic
/// token: whitespace (insignificant), punctuation (delimiters editors theme structurally), and lex
/// errors (surfaced as diagnostics, not tokens).
pub fn semantic_token(kind: HighlightKind) -> Option<(SemanticTokenType, SemanticTokenModifiers)> {
    use HighlightKind::*;
    use SemanticTokenModifiers as M;
    Some(match kind {
        Keyword => (SemanticTokenType::Keyword, M::DEFAULT_LIBRARY),
        Type => (SemanticTokenType::Type, M::DEFAULT_LIBRARY),
        Identifier | QuotedIdentifier => (SemanticTokenType::Variable, M::NONE),
        String | DollarString => (SemanticTokenType::String, M::NONE),
        Number => (SemanticTokenType::Number, M::NONE),
        Variable => (SemanticTokenType::Parameter, M::NONE),
        Operator => (SemanticTokenType::Operator, M::NONE),
        Comment => (SemanticTokenType::Comment, M::DOCUMENTATION),
        Whitespace | Punctuation | Error => return None,
    })
}

/// An embedded-language region inside a `$$ … $$` body. Built by [`detect_injections`]; consumers
/// can re-highlight the [`range`](Injection::range) with the embedded grammar named by
/// [`language`](Injection::language). `#[non_exhaustive]` so fields can be added compatibly.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Injection {
    /// The embedded language declared (or inferred) for this body.
    pub language: InjectedLanguage,
    /// Byte range of the dollar-quoted body *including* its `$$` delimiters.
    pub range: std::ops::Range<usize>,
}

/// A language that can be embedded in a Snowflake `$$ … $$` UDF / procedure body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InjectedLanguage {
    Sql,
    JavaScript,
    Python,
    Java,
    Scala,
}

impl InjectedLanguage {
    /// The Tree-sitter / TextMate-ish injection scope name editors use to pick a grammar.
    pub const fn scope(self) -> &'static str {
        match self {
            InjectedLanguage::Sql => "source.snowflake-sql",
            InjectedLanguage::JavaScript => "source.js",
            InjectedLanguage::Python => "source.python",
            InjectedLanguage::Java => "source.java",
            InjectedLanguage::Scala => "source.scala",
        }
    }

    /// Map a `LANGUAGE <name>` word (case-insensitive) to a language. SQL is the default body
    /// language, so an unrecognized word falls back to [`InjectedLanguage::Sql`].
    fn from_language_word(word: &str) -> InjectedLanguage {
        if word.eq_ignore_ascii_case("javascript") {
            InjectedLanguage::JavaScript
        } else if word.eq_ignore_ascii_case("python") {
            InjectedLanguage::Python
        } else if word.eq_ignore_ascii_case("java") {
            InjectedLanguage::Java
        } else if word.eq_ignore_ascii_case("scala") {
            InjectedLanguage::Scala
        } else {
            InjectedLanguage::Sql
        }
    }
}

/// A semantic token resolved to absolute byte coordinates (pre delta-encoding).
/// `#[non_exhaustive]` so fields can be added without breaking downstream matches.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResolvedToken {
    /// Byte range of the token in the source.
    pub range: std::ops::Range<usize>,
    /// The LSP semantic token type this token maps to.
    pub token_type: SemanticTokenType,
    /// The LSP semantic token modifiers (a bitset) for this token.
    pub modifiers: SemanticTokenModifiers,
}

/// A semantic token as the LSP wire format wants it: zero-based line / UTF-16 char position with a
/// UTF-16 length, plus the legend indices. These are *absolute* (not yet delta-encoded); call
/// [`delta_encode`] for the on-wire `(deltaLine, deltaStartChar, …)` form. `#[non_exhaustive]` so
/// fields can be added without breaking downstream matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct LineToken {
    /// Zero-based line of the token's start.
    pub line: u32,
    /// Zero-based UTF-16 column of the token's start.
    pub start_char: u32,
    /// Length in UTF-16 code units (the LSP unit).
    pub length: u32,
    /// Legend index of the token type (see [`SemanticTokenType::index`]).
    pub token_type: u32,
    /// The token's modifier bitset (see [`SemanticTokenModifiers::bits`]).
    pub modifiers: u32,
}

/// Scan `input` for embedded `$$ … $$` bodies and tag each with its language. Routine bodies use
/// `LANGUAGE <name> ... AS $$...$$`; dynamic SQL uses `EXECUTE IMMEDIATE $$...$$`. Bodies after
/// `AS $$...$$` with no `LANGUAGE` clause default to [`InjectedLanguage::Sql`]. Runs off the lexical
/// highlighter, so it never parses or panics.
pub fn detect_injections(input: &str) -> Vec<Injection> {
    let highlighted = highlight(input);
    let mut injections = Vec::new();
    let mut language: Option<InjectedLanguage> = None;
    let mut expect_language_name = false;
    let mut saw_as_after_language = false;
    let mut saw_as = false;
    let mut saw_execute = false;
    let mut saw_execute_immediate = false;

    for token in &highlighted.tokens {
        match token.kind {
            HighlightKind::Whitespace | HighlightKind::Comment => {}
            HighlightKind::DollarString => {
                if saw_as_after_language || saw_as || saw_execute_immediate {
                    injections.push(Injection {
                        language: language.take().unwrap_or(InjectedLanguage::Sql),
                        range: token.range.clone(),
                    });
                }
                expect_language_name = false;
                saw_as_after_language = false;
                saw_as = false;
                saw_execute = false;
                saw_execute_immediate = false;
            }
            HighlightKind::Punctuation if token.text == ";" => {
                // Statement boundary: a LANGUAGE clause does not carry across statements.
                language = None;
                expect_language_name = false;
                saw_as_after_language = false;
                saw_as = false;
                saw_execute = false;
                saw_execute_immediate = false;
            }
            HighlightKind::Keyword if token.text.eq_ignore_ascii_case("language") => {
                expect_language_name = true;
                saw_as_after_language = false;
                saw_as = false;
                saw_execute = false;
                saw_execute_immediate = false;
            }
            HighlightKind::Keyword | HighlightKind::Identifier if expect_language_name => {
                language = Some(InjectedLanguage::from_language_word(token.text));
                expect_language_name = false;
                saw_as_after_language = false;
                saw_as = false;
                saw_execute = false;
                saw_execute_immediate = false;
            }
            HighlightKind::Keyword if token.text.eq_ignore_ascii_case("as") => {
                saw_as_after_language = language.is_some();
                saw_as = language.is_none();
                expect_language_name = false;
                saw_execute = false;
                saw_execute_immediate = false;
            }
            HighlightKind::Keyword if token.text.eq_ignore_ascii_case("execute") => {
                expect_language_name = false;
                saw_as_after_language = false;
                saw_as = false;
                saw_execute = true;
                saw_execute_immediate = false;
            }
            HighlightKind::Keyword
                if token.text.eq_ignore_ascii_case("immediate") && saw_execute =>
            {
                expect_language_name = false;
                saw_as_after_language = false;
                saw_as = false;
                saw_execute = false;
                saw_execute_immediate = true;
            }
            _ => {
                expect_language_name = false;
                saw_as_after_language = false;
                saw_as = false;
                saw_execute = false;
                saw_execute_immediate = false;
            }
        }
    }
    injections
}

/// Resolve `input` to absolute-coordinate semantic tokens (no delta-encoding). Trivia, punctuation
/// and lex errors drop out; everything else becomes a [`ResolvedToken`] tagged per
/// [`semantic_token`]. Injections do not change a token's *type* — the lexical kind already wins —
/// but they are returned alongside by [`semantic_tokens`] so an editor can layer an embedded
/// grammar over the body's range.
pub fn resolve_tokens(input: &str) -> Vec<ResolvedToken> {
    let highlighted = highlight(input);
    highlighted
        .tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            if is_cortex_or_aisql_function_token(&highlighted.tokens, index) {
                return Some(ResolvedToken {
                    range: token.range.clone(),
                    token_type: SemanticTokenType::Function,
                    modifiers: SemanticTokenModifiers::DEFAULT_LIBRARY,
                });
            }
            let (token_type, modifiers) = semantic_token(token.kind)?;
            Some(ResolvedToken {
                range: token.range.clone(),
                token_type,
                modifiers,
            })
        })
        .collect()
}

fn is_cortex_or_aisql_function_token(tokens: &[HighlightToken<'_>], index: usize) -> bool {
    let token = &tokens[index];
    if !matches!(
        token.kind,
        HighlightKind::Identifier | HighlightKind::Keyword
    ) {
        return false;
    }
    if next_significant(tokens, index).is_none_or(|next| tokens[next].text != "(") {
        return false;
    }
    token.text.to_ascii_uppercase().starts_with("AI_")
        || is_snowflake_cortex_qualified_leaf(tokens, index)
}

fn is_snowflake_cortex_qualified_leaf(tokens: &[HighlightToken<'_>], index: usize) -> bool {
    let Some(dot_before_fn) = prev_significant(tokens, index) else {
        return false;
    };
    if tokens[dot_before_fn].text != "." {
        return false;
    }
    let Some(cortex) = prev_significant(tokens, dot_before_fn) else {
        return false;
    };
    if !tokens[cortex].text.eq_ignore_ascii_case("cortex") {
        return false;
    }
    let Some(dot_before_cortex) = prev_significant(tokens, cortex) else {
        return false;
    };
    if tokens[dot_before_cortex].text != "." {
        return false;
    }
    let Some(snowflake) = prev_significant(tokens, dot_before_cortex) else {
        return false;
    };
    tokens[snowflake].text.eq_ignore_ascii_case("snowflake")
}

fn prev_significant(tokens: &[HighlightToken<'_>], index: usize) -> Option<usize> {
    tokens[..index]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, token)| {
            !matches!(
                token.kind,
                HighlightKind::Whitespace | HighlightKind::Comment
            )
        })
        .map(|(index, _)| index)
}

fn next_significant(tokens: &[HighlightToken<'_>], index: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(index + 1)
        .find(|(_, token)| {
            !matches!(
                token.kind,
                HighlightKind::Whitespace | HighlightKind::Comment
            )
        })
        .map(|(index, _)| index)
}

/// The full semantic-token result: the per-token tagging plus the embedded-language regions.
/// `#[non_exhaustive]` so fields can be added without breaking downstream matches.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SemanticTokens {
    /// Every significant token, tagged with its LSP type + modifiers (trivia/punctuation dropped).
    pub tokens: Vec<ResolvedToken>,
    /// The `$$ … $$` embedded-language regions detected in the source.
    pub injections: Vec<Injection>,
}

/// Resolve `input` to semantic tokens *and* its embedded-language injection regions in one pass.
pub fn semantic_tokens(input: &str) -> SemanticTokens {
    SemanticTokens {
        tokens: resolve_tokens(input),
        injections: detect_injections(input),
    }
}

/// Maps byte offsets to LSP `(line, utf16-column)` positions. Self-contained so this module needs
/// no `lsp_types` dependency; it mirrors the `LineIndex` in `snow-fmt-lsp`.
struct LineMap<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineMap<'a> {
    fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        );
        LineMap { text, line_starts }
    }

    /// `(line, utf16_col)` of a byte offset, clamped to the document end.
    fn position(&self, offset: usize) -> (u32, u32) {
        let offset = offset.min(self.text.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        };
        let line_start = self.line_starts[line];
        let col: usize = self.text[line_start..offset]
            .chars()
            .map(char::len_utf16)
            .sum();
        (line as u32, col as u32)
    }
}

/// Lower resolved tokens to LSP [`LineToken`]s: split multi-line tokens (block comments,
/// dollar-quoted bodies) into one token per line — the LSP encoding forbids a token spanning a
/// newline — and compute UTF-16 positions and lengths. Output is sorted by `(line, start_char)`.
pub fn line_tokens(input: &str) -> Vec<LineToken> {
    let resolved = resolve_tokens(input);
    let map = LineMap::new(input);
    let mut out = Vec::new();

    for token in &resolved {
        let mut piece_start = token.range.start;
        for piece in input[token.range.clone()].split('\n') {
            let length: u32 = piece.chars().map(|c| c.len_utf16() as u32).sum();
            if length > 0 {
                let (line, start_char) = map.position(piece_start);
                out.push(LineToken {
                    line,
                    start_char,
                    length,
                    token_type: token.token_type.index(),
                    modifiers: token.modifiers.bits(),
                });
            }
            piece_start += piece.len() + 1; // skip the piece and its trailing '\n'
        }
    }
    out
}

/// Delta-encode absolute [`LineToken`]s into the LSP wire form: each quintuple is
/// `(deltaLine, deltaStartChar, length, tokenType, tokenModifiers)`, where deltas are relative to
/// the previous token (and `deltaStartChar` resets to the absolute column when the line advances).
/// Tokens must be sorted by `(line, start_char)`; [`line_tokens`] already produces them that way.
pub fn delta_encode(tokens: &[LineToken]) -> Vec<[u32; 5]> {
    let mut out = Vec::with_capacity(tokens.len());
    let (mut prev_line, mut prev_col) = (0u32, 0u32);
    for token in tokens {
        let delta_line = token.line - prev_line;
        let delta_start = if delta_line == 0 {
            token.start_char - prev_col
        } else {
            token.start_char
        };
        out.push([
            delta_line,
            delta_start,
            token.length,
            token.token_type,
            token.modifiers,
        ]);
        (prev_line, prev_col) = (token.line, token.start_char);
    }
    out
}

/// One-shot: resolve, lower to lines, and delta-encode. The vector an LSP server hands back for
/// `textDocument/semanticTokens/full` (modulo the server's wrapper type).
pub fn semantic_tokens_lsp(input: &str) -> Vec<[u32; 5]> {
    delta_encode(&line_tokens(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legend_indices_match_their_position() {
        for (i, ty) in SemanticTokenType::LEGEND.iter().enumerate() {
            assert_eq!(ty.index() as usize, i, "legend index drift for {ty:?}");
        }
        // Names are the canonical LSP spellings.
        assert_eq!(SemanticTokenType::Keyword.name(), "keyword");
        assert_eq!(SemanticTokenType::Parameter.name(), "parameter");
        assert_eq!(SemanticTokenType::Namespace.name(), "namespace");
    }

    #[test]
    fn modifier_bitset_is_a_real_bitset() {
        let both = SemanticTokenModifiers::DOCUMENTATION | SemanticTokenModifiers::DEFAULT_LIBRARY;
        assert_eq!(both.bits(), 0b11);
        assert!(both.contains(SemanticTokenModifiers::DOCUMENTATION));
        assert!(both.contains(SemanticTokenModifiers::DEFAULT_LIBRARY));
        assert!(!SemanticTokenModifiers::NONE.contains(SemanticTokenModifiers::DOCUMENTATION));
        assert_eq!(SemanticTokenModifiers::LEGEND.len(), 2);
    }

    #[test]
    fn every_highlight_kind_maps_consistently() {
        use HighlightKind::*;
        // Significant kinds carry a token; insignificant ones do not.
        for kind in [
            Keyword,
            Type,
            Identifier,
            QuotedIdentifier,
            String,
            DollarString,
            Number,
            Variable,
            Operator,
            Comment,
        ] {
            assert!(semantic_token(kind).is_some(), "{kind:?} should map");
        }
        for kind in [Whitespace, Punctuation, Error] {
            assert!(semantic_token(kind).is_none(), "{kind:?} should not map");
        }
        // Keywords and types are part of the language.
        assert_eq!(
            semantic_token(Keyword).unwrap().1,
            SemanticTokenModifiers::DEFAULT_LIBRARY
        );
        assert_eq!(
            semantic_token(Type).unwrap().1,
            SemanticTokenModifiers::DEFAULT_LIBRARY
        );
        // Comments are documentation.
        assert_eq!(
            semantic_token(Comment).unwrap().1,
            SemanticTokenModifiers::DOCUMENTATION
        );
    }

    #[test]
    fn resolve_drops_trivia_and_punctuation() {
        let toks = resolve_tokens("SELECT a, 1 -- c\n");
        let types: Vec<_> = toks.iter().map(|t| t.token_type).collect();
        // SELECT(kw) a(var) 1(num) -- c(comment); the comma is punctuation and drops.
        assert_eq!(
            types,
            vec![
                SemanticTokenType::Keyword,
                SemanticTokenType::Variable,
                SemanticTokenType::Number,
                SemanticTokenType::Comment,
            ]
        );
    }

    #[test]
    fn resolved_ranges_match_source_text() {
        let sql = "SELECT $1::NUMBER FROM t";
        for tok in resolve_tokens(sql) {
            assert!(tok.range.end <= sql.len());
        }
        // $1 is a parameter, NUMBER a type, :: an operator.
        let sem = semantic_tokens(sql);
        let by_text: Vec<_> = sem
            .tokens
            .iter()
            .map(|t| (&sql[t.range.clone()], t.token_type))
            .collect();
        assert!(by_text.contains(&("$1", SemanticTokenType::Parameter)));
        assert!(by_text.contains(&("NUMBER", SemanticTokenType::Type)));
        assert!(by_text.contains(&("::", SemanticTokenType::Operator)));
    }

    #[test]
    fn line_tokens_are_utf16_and_split_multiline() {
        // 芋 is 3 bytes but 1 UTF-16 unit; the block comment spans two lines.
        let sql = "SELECT '芋' /* a\nb */ x";
        let lines = line_tokens(sql);
        // The string token length is in UTF-16 units: '芋' => quote+芋+quote = 3 units.
        let string_tok = lines
            .iter()
            .find(|t| t.token_type == SemanticTokenType::String.index())
            .unwrap();
        assert_eq!(string_tok.length, 3);
        // The block comment is split into two LineTokens on consecutive lines.
        let comment_lines: Vec<_> = lines
            .iter()
            .filter(|t| t.token_type == SemanticTokenType::Comment.index())
            .map(|t| t.line)
            .collect();
        assert_eq!(comment_lines, vec![0, 1]);
    }

    #[test]
    fn delta_encode_resets_column_on_newline() {
        let sql = "SELECT a\nFROM t";
        let encoded = semantic_tokens_lsp(sql);
        // SELECT@(0,0) a@(0,7) FROM@(1,0) t@(1,5)
        assert_eq!(
            encoded[0],
            [0, 0, 6, 0, SemanticTokenModifiers::DEFAULT_LIBRARY.bits()]
        );
        // a: same line, +7 from SELECT start; variable type=2, no modifiers.
        assert_eq!(encoded[1], [0, 7, 1, 2, 0]);
        // FROM: next line, absolute column 0; keyword.
        assert_eq!(
            encoded[2],
            [1, 0, 4, 0, SemanticTokenModifiers::DEFAULT_LIBRARY.bits()]
        );
        // t: same line, +5.
        assert_eq!(encoded[3], [0, 5, 1, 2, 0]);
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(line_tokens("").is_empty());
        assert!(semantic_tokens_lsp("").is_empty());
        assert!(detect_injections("").is_empty());
        assert!(semantic_tokens("").tokens.is_empty());
    }

    #[test]
    fn detects_javascript_injection_from_language_clause() {
        let sql = "CREATE FUNCTION f() RETURNS STRING LANGUAGE JAVASCRIPT AS $$ return 1; $$;";
        let injections = detect_injections(sql);
        assert_eq!(injections.len(), 1);
        assert_eq!(injections[0].language, InjectedLanguage::JavaScript);
        // The range covers the whole $$ body including delimiters.
        let body = &sql[injections[0].range.clone()];
        assert!(body.starts_with("$$"));
        assert!(body.ends_with("$$"));
        assert!(body.contains("return 1;"));
    }

    #[test]
    fn detects_python_and_scala_and_java_injections() {
        for (word, expected) in [
            ("PYTHON", InjectedLanguage::Python),
            ("Java", InjectedLanguage::Java),
            ("scala", InjectedLanguage::Scala),
            ("SQL", InjectedLanguage::Sql),
        ] {
            let sql = format!("CREATE FUNCTION f() RETURNS INT LANGUAGE {word} AS $$x$$;");
            let injections = detect_injections(&sql);
            assert_eq!(injections.len(), 1, "for {word}");
            assert_eq!(injections[0].language, expected, "for {word}");
        }
    }

    #[test]
    fn body_without_language_clause_defaults_to_sql() {
        let sql = "CREATE PROCEDURE p() RETURNS STRING AS $$ BEGIN RETURN 'ok'; END $$; \
                   EXECUTE IMMEDIATE $$ SELECT 1 $$;";
        let injections = detect_injections(sql);
        assert_eq!(injections.len(), 2);
        assert_eq!(injections[0].language, InjectedLanguage::Sql);
        assert_eq!(injections[1].language, InjectedLanguage::Sql);
        assert_eq!(InjectedLanguage::Sql.scope(), "source.snowflake-sql");
        assert_eq!(InjectedLanguage::JavaScript.scope(), "source.js");
    }

    #[test]
    fn non_body_dollar_strings_do_not_get_injected() {
        let sql = "SELECT $$plain text$$ AS value; \
                   CREATE PROCEDURE p() LANGUAGE PYTHON IMPORTS = ($$stage/file.py$$) AS $$body$$;";
        let injections = detect_injections(sql);
        assert_eq!(injections.len(), 1);
        assert_eq!(injections[0].language, InjectedLanguage::Python);
        assert_eq!(&sql[injections[0].range.clone()], "$$body$$");
    }

    #[test]
    fn language_clause_does_not_leak_across_statements() {
        // The first statement is JS; the second has a bare body and must default to SQL.
        let sql = "CREATE FUNCTION a() RETURNS INT LANGUAGE JAVASCRIPT AS $$1$$; \
                   EXECUTE IMMEDIATE $$ SELECT 2 $$;";
        let injections = detect_injections(sql);
        assert_eq!(injections.len(), 2);
        assert_eq!(injections[0].language, InjectedLanguage::JavaScript);
        assert_eq!(injections[1].language, InjectedLanguage::Sql);
    }

    #[test]
    fn multiple_bodies_each_get_their_language() {
        let sql = "CREATE FUNCTION a() LANGUAGE PYTHON AS $$py$$; \
                   CREATE FUNCTION b() LANGUAGE JAVASCRIPT AS $$js$$;";
        let injections = detect_injections(sql);
        assert_eq!(injections.len(), 2);
        assert_eq!(injections[0].language, InjectedLanguage::Python);
        assert_eq!(injections[1].language, InjectedLanguage::JavaScript);
    }

    #[test]
    fn semantic_tokens_bundles_tokens_and_injections() {
        let sql = "CREATE FUNCTION f() LANGUAGE JAVASCRIPT AS $$ return 1; $$";
        let sem = semantic_tokens(sql);
        assert_eq!(sem.injections.len(), 1);
        assert_eq!(sem.injections[0].language, InjectedLanguage::JavaScript);
        // The dollar body is itself a String semantic token covering the injection range.
        let dollar = sem
            .tokens
            .iter()
            .find(|t| {
                t.token_type == SemanticTokenType::String && sql[t.range.clone()].contains("$$")
            })
            .expect("dollar string token");
        assert_eq!(dollar.range, sem.injections[0].range);
    }

    #[test]
    fn never_panics_on_adversarial_input() {
        for sql in [
            "$$",
            "$$ unterminated",
            "LANGUAGE",
            "LANGUAGE $$x$$",
            ";;;;",
            "@~/stage/ $1 :name ? ->> |> => :: ->",
            "'長芋' \"畑\" -- 芋\n/* 芋 */ $$芋$$",
            "\r\n\r\n",
        ] {
            // Each entry point must complete without panicking and stay self-consistent.
            let lines = line_tokens(sql);
            let encoded = delta_encode(&lines);
            assert_eq!(lines.len(), encoded.len());
            let _ = semantic_tokens(sql);
            let _ = semantic_tokens_lsp(sql);
        }
    }
}
