//! The single-pass tokenizer.

use crate::token::{LexError, Lexed, Token};
use crate::{BodyDelimiter, LexOptions};
use sql_dialect_fmt_syntax::{Dialect, SyntaxKind};
use sql_dialect_fmt_text::LineIndex;

/// Tokenize Snowflake SQL into a lossless token stream.
///
/// The concatenation of `tokens[i].text` always equals `input`.
pub fn tokenize(input: &str) -> Lexed<'_> {
    tokenize_with_options(input, LexOptions::default())
}

pub fn tokenize_with_options<'a>(input: &'a str, options: LexOptions<'_>) -> Lexed<'a> {
    Lexer::new(input, options).run()
}

/// Tokenize `input` for `dialect`, using that dialect's default lexing rules.
///
/// The dialect drives quoting and special-token behavior (`$$`/`$n` dollar handling, `@stage`
/// references); see [`Dialect`].
pub fn tokenize_for_dialect(input: &str, dialect: Dialect) -> Lexed<'_> {
    tokenize_with_options(input, LexOptions::default().with_dialect(dialect))
}

struct Lexer<'a, 'cfg> {
    input: &'a str,
    bytes: &'a [u8],
    line_index: LineIndex<'a>,
    options: LexOptions<'cfg>,
    pos: usize,
    tokens: Vec<Token<'a>>,
    errors: Vec<LexError>,
}

impl<'a, 'cfg> Lexer<'a, 'cfg> {
    fn new(input: &'a str, options: LexOptions<'cfg>) -> Self {
        Lexer {
            input,
            bytes: input.as_bytes(),
            line_index: LineIndex::new(input),
            options,
            pos: 0,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    #[inline]
    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    /// Current byte, or `0` at end of input (`0` never appears in valid SQL we care about).
    #[inline]
    fn peek(&self) -> u8 {
        if self.pos < self.bytes.len() {
            self.bytes[self.pos]
        } else {
            0
        }
    }

    #[inline]
    fn peek_at(&self, n: usize) -> u8 {
        let i = self.pos + n;
        if i < self.bytes.len() {
            self.bytes[i]
        } else {
            0
        }
    }

    /// Advance one byte and return it. Callers must ensure `!at_end()`.
    #[inline]
    fn bump(&mut self) -> u8 {
        let c = self.bytes[self.pos];
        self.pos += 1;
        c
    }

    #[inline]
    fn eat_while(&mut self, mut pred: impl FnMut(u8) -> bool) {
        while !self.at_end() && pred(self.peek()) {
            self.pos += 1;
        }
    }

    #[inline]
    fn starts_with(&self, text: &str) -> bool {
        self.bytes[self.pos..].starts_with(text.as_bytes())
    }

    #[inline]
    fn push(&mut self, kind: SyntaxKind, start: usize) {
        // `self.input` is a `&'a str`; reborrowing through it yields a `&'a str` that does not
        // borrow `self`, so this composes fine with the `&mut self` push.
        let text = &self.input[start..self.pos];
        self.tokens.push(Token { kind, text });
    }

    /// Record a lexical diagnostic spanning `offset..self.pos` (the offending token text consumed
    /// so far). Called once the lexer has advanced past the start of the bad token, so the current
    /// position marks the end of the span being reported.
    #[inline]
    fn error(&mut self, message: impl Into<String>, offset: usize) {
        self.errors.push(LexError {
            message: message.into(),
            offset,
            len: self.pos.saturating_sub(offset),
            line_column: Some(self.line_index.line_column(offset)),
        });
    }

    fn run(mut self) -> Lexed<'a> {
        // Dialects that support dollar quoting (`$$ … $$`) treat such a run as one body token; the
        // rest (none yet) lex the `$` characters as ordinary operators.
        let dollar_quoting = self.options.dialect.supports_dollar_quoting();
        while !self.at_end() {
            let start = self.pos;
            if dollar_quoting {
                if let Some(delimiter) = self.body_delimiter_at() {
                    self.delimited_body(start, delimiter);
                    self.push(SyntaxKind::DOLLAR_STRING, start);
                    continue;
                }
            }
            match self.peek() {
                b' ' | b'\t' => {
                    self.eat_while(|c| c == b' ' || c == b'\t');
                    self.push(SyntaxKind::WHITESPACE, start);
                }
                b'\n' => {
                    self.pos += 1;
                    self.push(SyntaxKind::NEWLINE, start);
                }
                b'\r' => {
                    self.pos += 1;
                    if self.peek() == b'\n' {
                        self.pos += 1;
                    }
                    self.push(SyntaxKind::NEWLINE, start);
                }
                // Line comments: -- ...  and  // ...
                b'-' if self.peek_at(1) == b'-' => {
                    self.line_comment();
                    self.push(SyntaxKind::COMMENT, start);
                }
                b'/' if self.options.dialect.supports_double_slash_comments()
                    && self.peek_at(1) == b'/' =>
                {
                    self.line_comment();
                    self.push(SyntaxKind::COMMENT, start);
                }
                // Block comment: /* ... */  (Snowflake block comments do not nest)
                b'/' if self.peek_at(1) == b'*' => {
                    self.block_comment(start);
                    self.push(SyntaxKind::BLOCK_COMMENT, start);
                }
                b'\'' => {
                    self.string_body(start, true);
                    self.push(SyntaxKind::STRING, start);
                }
                b'r' | b'R'
                    if self.options.dialect.supports_prefixed_strings()
                        && self.peek_at(1) == b'\'' =>
                {
                    self.pos += 1; // raw-string prefix
                    self.string_body(start, false);
                    self.push(SyntaxKind::STRING, start);
                }
                b'x' | b'X'
                    if self.options.dialect.supports_prefixed_strings()
                        && self.peek_at(1) == b'\'' =>
                {
                    self.pos += 1; // hex-string prefix
                    self.string_body(start, false);
                    self.push(SyntaxKind::STRING, start);
                }
                b'"' => {
                    self.quoted_ident_body(start);
                    self.push(SyntaxKind::QUOTED_IDENT, start);
                }
                b'`' if self.options.dialect.supports_backtick_identifiers() => {
                    self.backtick_ident_body(start);
                    self.push(SyntaxKind::QUOTED_IDENT, start);
                }
                // `${ ... }` template-substitution placeholder. A `$` immediately followed by `{`
                // opens a placeholder whose body is a balanced brace run: JS template-literal
                // interpolation (`${cfg.table}`), Databricks / Spark / dbt variable substitution
                // (`${env:VAR}`), and similar. Recognized in every dialect (SQL is commonly embedded
                // in a host language), and lexed as one atomic token so the surrounding statement
                // still parses, formats, and highlights instead of derailing on the braces.
                b'$' if self.peek_at(1) == b'{' => {
                    self.placeholder(start);
                    self.push(SyntaxKind::PLACEHOLDER, start);
                }
                // $1 / $name variables (but not body delimiters, handled above). Gated on dollar
                // quoting so non-Snowflake dialects lex a bare `$` as the DOLLAR operator instead.
                b'$' if dollar_quoting
                    && (is_ident_start(self.peek_at(1)) || self.peek_at(1).is_ascii_digit()) =>
                {
                    self.pos += 1; // $
                    self.eat_while(is_ident_continue);
                    self.push(SyntaxKind::VARIABLE, start);
                }
                c if c.is_ascii_digit() => self.number(start),
                // Leading-dot float like `.5` (a bare `.` is the DOT operator, handled below).
                b'.' if self.peek_at(1).is_ascii_digit() => self.number(start),
                c if is_ident_start(c)
                    && self.options.dialect.supports_stage_refs()
                    && self.at_file_uri_start() =>
                {
                    self.file_uri();
                    self.push(SyntaxKind::FILE_URI, start);
                }
                c if is_ident_start(c) => {
                    self.eat_while(is_ident_continue);
                    self.push(SyntaxKind::IDENT, start);
                }
                _ => self.operator(start),
            }
        }
        Lexed {
            tokens: self.tokens,
            errors: self.errors,
        }
    }

    /// Whether the current position begins an unquoted Snowflake client-file URI. Recognizing the
    /// entire `file://...` run before identifier/comment dispatch prevents the URI's `//` from
    /// being mistaken for a line comment.
    fn at_file_uri_start(&self) -> bool {
        self.bytes
            .get(self.pos..self.pos.saturating_add(b"file://".len()))
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"file://"))
    }

    /// Consume an unquoted `file://...` URI through the next SQL separator. Paths containing
    /// spaces or other separators must be single-quoted per Snowflake syntax and are therefore
    /// handled by [`Self::string_body`] instead.
    fn file_uri(&mut self) {
        self.eat_while(|c| !c.is_ascii_whitespace() && c != b';');
    }

    /// Consume `--`/`//` and the rest of the physical line (not the newline itself).
    fn line_comment(&mut self) {
        self.pos += 2;
        while !self.at_end() {
            let c = self.peek();
            if c == b'\n' || c == b'\r' {
                break;
            }
            self.pos += 1;
        }
    }

    /// Consume `/* ... */`. Records an error if unterminated. Non-nesting (Snowflake semantics).
    fn block_comment(&mut self, start: usize) {
        self.pos += 2; // /*
        loop {
            if self.at_end() {
                self.error("unterminated block comment", start);
                break;
            }
            if self.peek() == b'*' && self.peek_at(1) == b'/' {
                self.pos += 2;
                break;
            }
            self.pos += 1;
        }
    }

    /// Consume a single-quoted string. Handles `''` (doubled quote) and `\` escapes
    /// (Snowflake interprets backslash escape sequences in string literals by default).
    fn string_body(&mut self, start: usize, backslash_escapes: bool) {
        self.pos += 1; // opening '
        loop {
            if self.at_end() {
                self.error("unterminated string literal", start);
                break;
            }
            match self.bump() {
                b'\\' if backslash_escapes => {
                    // Escape: consume the next byte if present (e.g. \' or \\).
                    if !self.at_end() {
                        self.pos += 1;
                    }
                }
                b'\'' => {
                    if self.peek() == b'\'' {
                        self.pos += 1; // doubled quote → escaped quote, keep going
                    } else {
                        break; // closing quote
                    }
                }
                _ => {}
            }
        }
    }

    /// Consume a `"quoted identifier"`. Handles `""` (doubled quote). No backslash escapes.
    fn quoted_ident_body(&mut self, start: usize) {
        self.pos += 1; // opening "
        loop {
            if self.at_end() {
                self.error("unterminated quoted identifier", start);
                break;
            }
            if self.bump() == b'"' {
                if self.peek() == b'"' {
                    self.pos += 1; // doubled quote → escaped, keep going
                } else {
                    break; // closing quote
                }
            }
        }
    }

    /// Consume a Databricks/Spark `` `quoted identifier` ``. Handles doubled backticks.
    fn backtick_ident_body(&mut self, start: usize) {
        self.pos += 1; // opening `
        loop {
            if self.at_end() {
                self.error("unterminated backtick identifier", start);
                break;
            }
            if self.bump() == b'`' {
                if self.peek() == b'`' {
                    self.pos += 1; // doubled backtick, keep going
                } else {
                    break; // closing backtick
                }
            }
        }
    }

    fn body_delimiter_at(&self) -> Option<BodyDelimiter> {
        self.options
            .body_delimiters
            .iter()
            .copied()
            .filter(|delimiter| {
                !delimiter.opener.is_empty()
                    && !delimiter.closer.is_empty()
                    && self.starts_with(delimiter.opener)
            })
            .max_by_key(|delimiter| delimiter.opener.len())
    }

    /// Consume a delimited embedded body. The current Snowflake default is `$$...$$`.
    fn delimited_body(&mut self, start: usize, delimiter: BodyDelimiter) {
        self.pos += delimiter.opener.len();
        loop {
            if self.at_end() {
                self.error(format!("unterminated {}", delimiter.name), start);
                break;
            }
            if self.starts_with(delimiter.closer) {
                self.pos += delimiter.closer.len();
                break;
            }
            self.pos += 1;
        }
    }

    /// Consume a `${ ... }` template-substitution placeholder, entered with the cursor on the
    /// opening `$` (and `{` immediately after). Advances past the matching close brace.
    ///
    /// The body is scanned with an explicit context stack rather than a naive search for the next
    /// `}`, so nested structure is handled exactly:
    /// - nested braces balance (`${ fn({a: 1, b: [2]}) }` stays one token),
    /// - a `}` inside a `'...'` / `"..."` string does not close the placeholder,
    /// - a back-tick template literal switches to string context, where only `` ` `` closes it and
    ///   an inner `${ ... }` opens a fresh nested placeholder scope (JS template nesting).
    ///
    /// The stack lives on the heap and grows at most one frame per opening brace/back-tick, so even
    /// pathological input cannot overflow the call stack. An unterminated placeholder records an
    /// error but still consumes to end of input, keeping the token lossless.
    fn placeholder(&mut self, start: usize) {
        self.pos += 2; // opening `${`
        let mut stack = vec![PlaceholderCtx::Brace];
        while let Some(ctx) = stack.last().copied() {
            if self.at_end() {
                self.error("unterminated placeholder", start);
                return;
            }
            match ctx {
                // JS-expression context: braces are structural, strings are skipped wholesale.
                PlaceholderCtx::Brace => match self.peek() {
                    b'{' => {
                        self.pos += 1;
                        stack.push(PlaceholderCtx::Brace);
                    }
                    b'}' => {
                        self.pos += 1;
                        stack.pop();
                    }
                    b'\'' => self.skip_placeholder_string(b'\''),
                    b'"' => self.skip_placeholder_string(b'"'),
                    b'`' => {
                        self.pos += 1;
                        stack.push(PlaceholderCtx::Template);
                    }
                    _ => self.pos += 1,
                },
                // Template-literal context: bare braces are text; only `` ` `` closes, and `${`
                // opens a nested interpolation scope.
                PlaceholderCtx::Template => match self.peek() {
                    b'\\' => {
                        self.pos += 1;
                        if !self.at_end() {
                            self.pos += 1; // escaped character
                        }
                    }
                    b'`' => {
                        self.pos += 1;
                        stack.pop();
                    }
                    b'$' if self.peek_at(1) == b'{' => {
                        self.pos += 2;
                        stack.push(PlaceholderCtx::Brace);
                    }
                    _ => self.pos += 1,
                },
            }
        }
    }

    /// Skip a `'...'` / `"..."` string inside a placeholder body so a `}` within it does not close
    /// the placeholder. Honors backslash escapes (JS interpolation semantics). Entered with the
    /// cursor on the opening quote; returns with the cursor past the closing quote, or at end of
    /// input if the string is unterminated (the caller then reports the placeholder as unterminated).
    fn skip_placeholder_string(&mut self, quote: u8) {
        self.pos += 1; // opening quote
        while !self.at_end() {
            match self.bump() {
                b'\\' => {
                    if !self.at_end() {
                        self.pos += 1; // escaped character
                    }
                }
                c if c == quote => return,
                _ => {}
            }
        }
    }

    /// Lex a numeric literal. Entered on a digit or on `.` immediately followed by a digit.
    fn number(&mut self, start: usize) {
        let mut is_float = false;
        if self.peek() == b'.' {
            // Leading-dot float: `.5`
            is_float = true;
            self.pos += 1;
            self.eat_while(|c| c.is_ascii_digit());
        } else {
            self.eat_while(|c| c.is_ascii_digit());
            if self.peek() == b'.' {
                // Fractional part (or a trailing dot, e.g. `100.`).
                is_float = true;
                self.pos += 1;
                self.eat_while(|c| c.is_ascii_digit());
            }
        }
        // Optional exponent: e / E [ + | - ] digits. Backtrack if no digits follow.
        if self.peek() == b'e' || self.peek() == b'E' {
            let save = self.pos;
            self.pos += 1;
            if self.peek() == b'+' || self.peek() == b'-' {
                self.pos += 1;
            }
            if self.peek().is_ascii_digit() {
                is_float = true;
                self.eat_while(|c| c.is_ascii_digit());
            } else {
                self.pos = save; // not actually an exponent
            }
        }
        self.push(
            if is_float {
                SyntaxKind::FLOAT_NUMBER
            } else {
                SyntaxKind::INT_NUMBER
            },
            start,
        );
    }

    /// Lex punctuation / operators (everything not handled by the dispatch in `run`).
    fn operator(&mut self, start: usize) {
        let c = self.peek();
        // A stray multi-byte (non-ASCII) char outside any literal: consume the whole char so
        // we never slice the &str off a UTF-8 boundary, and report it. `c >= 0x80` guarantees a
        // char starts here, but fall back to a single byte rather than panic if that ever changes.
        if c >= 0x80 {
            if let Some(ch) = self.input[self.pos..].chars().next() {
                self.pos += ch.len_utf8();
                self.error(format!("unexpected character {ch:?}"), start);
            } else {
                self.pos += 1;
                self.error("unexpected byte", start);
            }
            self.push(SyntaxKind::ERROR, start);
            return;
        }

        self.pos += 1; // consume `c`
        let kind = match c {
            b'(' => SyntaxKind::L_PAREN,
            b')' => SyntaxKind::R_PAREN,
            b'[' => SyntaxKind::L_BRACKET,
            b']' => SyntaxKind::R_BRACKET,
            b'{' => SyntaxKind::L_BRACE,
            b'}' => SyntaxKind::R_BRACE,
            b',' => SyntaxKind::COMMA,
            b';' => SyntaxKind::SEMICOLON,
            b'+' => SyntaxKind::PLUS,
            b'*' => SyntaxKind::STAR,
            b'/' => SyntaxKind::SLASH,
            b'%' => SyntaxKind::PERCENT,
            b'&' => SyntaxKind::AMP,
            b'^' => SyntaxKind::CARET,
            b'~' => SyntaxKind::TILDE,
            // `@` introduces stage references (`@stage`, `@~`, `@%table`). Dialects without stage
            // refs (none yet) treat it as an unexpected character rather than the AT operator.
            b'@' if self.options.dialect.supports_stage_refs() => SyntaxKind::AT,
            b'?' => SyntaxKind::QUESTION,
            b'.' => SyntaxKind::DOT,
            b'$' => SyntaxKind::DOLLAR,
            b'=' => {
                if self.peek() == b'>' {
                    self.pos += 1;
                    SyntaxKind::FAT_ARROW
                } else {
                    SyntaxKind::EQ
                }
            }
            b'!' => {
                if self.peek() == b'=' {
                    self.pos += 1;
                    SyntaxKind::NEQ
                } else {
                    self.error("unexpected '!'", start);
                    SyntaxKind::BANG
                }
            }
            b'<' => {
                if self.options.dialect.supports_null_safe_eq()
                    && self.peek() == b'='
                    && self.peek_at(1) == b'>'
                {
                    self.pos += 2;
                    SyntaxKind::NULL_SAFE_EQ
                } else if self.peek() == b'=' {
                    self.pos += 1;
                    SyntaxKind::LTE
                } else if self.peek() == b'>' {
                    self.pos += 1;
                    SyntaxKind::NEQ
                } else {
                    SyntaxKind::LT
                }
            }
            b'>' => {
                if self.peek() == b'=' {
                    self.pos += 1;
                    SyntaxKind::GTE
                } else {
                    SyntaxKind::GT
                }
            }
            b':' => {
                if self.peek() == b':' {
                    self.pos += 1;
                    SyntaxKind::COLON2
                } else if self.peek() == b'=' {
                    self.pos += 1;
                    SyntaxKind::ASSIGN
                } else {
                    SyntaxKind::COLON
                }
            }
            b'-' => {
                if self.peek() == b'>' && self.peek_at(1) == b'>' {
                    self.pos += 2;
                    SyntaxKind::FLOW_PIPE
                } else if self.peek() == b'>' {
                    self.pos += 1;
                    SyntaxKind::ARROW
                } else {
                    SyntaxKind::MINUS
                }
            }
            b'|' => {
                if self.peek() == b'>' {
                    self.pos += 1;
                    SyntaxKind::PIPE_GT
                } else if self.peek() == b'|' {
                    self.pos += 1;
                    SyntaxKind::CONCAT
                } else {
                    SyntaxKind::PIPE
                }
            }
            other => {
                self.error(format!("unexpected character {:?}", other as char), start);
                SyntaxKind::ERROR
            }
        };
        self.push(kind, start);
    }
}

/// A scope on the placeholder scanner's context stack. See [`Lexer::placeholder`].
#[derive(Clone, Copy)]
enum PlaceholderCtx {
    /// JS-expression / brace scope, opened by `{` (or the initial `${`) and closed by `}`.
    Brace,
    /// Back-tick template-literal scope, opened and closed by `` ` ``.
    Template,
}

#[inline]
fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

#[inline]
fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}
