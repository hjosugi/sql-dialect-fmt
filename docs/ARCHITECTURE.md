<!-- i18n: language-switcher -->
[English](ARCHITECTURE.md) | [日本語](ARCHITECTURE.ja.md)

# Architecture

sql-dialect-fmt is split into small layers so contributors can work on one concern at a
time. The formatter is not implemented yet; the current base is the language
front-end and editor-facing metadata.

## Design Goals

- Lossless: comments, whitespace, and broken input are preserved.
- Resilient: parser and editor features keep working while the user is typing.
- Fast by default: hot paths avoid unnecessary allocation.
- Snowflake-first: features such as `->>`, semi-structured paths, stages,
  procedures, tasks, and embedded bodies are first-class.
- Easy to publish: Tree-sitter, hover, highlight, CLI, and future LSP pieces are
  separate packages instead of one tangled crate.

## Layer Map

```text
source SQL
  |
  v
sql-dialect-fmt-encoding     bytes -> UTF-8 text, or opaque bytes when unsafe
  |
  v
sql-dialect-fmt-lexer        lossless tokens + lexical diagnostics
  |
  +--> sql-dialect-fmt-highlight    lexical token classification
  |
  +--> sql-dialect-fmt-hover        editor hover summaries
  |
  v
sql-dialect-fmt-parser       resilient rowan CST
  |
  v
future formatter/LSP  Doc IR, semantic tokens, diagnostics

tree-sitter-snowflake is a parallel editor grammar for tools that consume
Tree-sitter directly. It is intentionally permissive and token-centric.
```

## Syntax and Lexer

`sql-dialect-fmt-encoding` owns the CLI/file boundary. It detects UTF-8, UTF-8 with
BOM, and UTF-16 LE/BE with BOM, and can encode edited text back to the original
encoding. Invalid or unsupported byte streams stay opaque and round-trip as
bytes; formatter layers must not guess an encoding and rewrite them.

`sql-dialect-fmt-syntax` owns the shared vocabulary:

- token and node kinds in `SyntaxKind`
- case-insensitive keyword lookup
- rowan language glue

`sql-dialect-fmt-lexer` is hand-written. It should remain boring and predictable:

- one pass over bytes
- no regex on the hot path
- no allocation per token
- never slice outside UTF-8 boundaries
- report lexical errors without stopping

The strongest invariant: concatenating every token text must reproduce the
input exactly.

Embedded procedure/function bodies are recognized through a small delimiter
table (`BodyDelimiter` + `LexOptions`). Current Snowflake uses `$$...$$` for the
lossless body token; future delimiters should be added as data, not by cloning
lexer states. See [docs/research/delimiter-strategy.md](research/delimiter-strategy.md).

## Parser

`sql-dialect-fmt-parser` builds a rowan CST through events. The parser should never
turn bad SQL into a panic. Unknown or incomplete input should become errors in
the tree while preserving source bytes.

Use parser tests for:

- precedence and structure
- recovery around incomplete syntax
- line ending preservation
- long inputs that could expose slow paths

## Editor Features

`sql-dialect-fmt-highlight` starts from the lexer so it can work before the full parser
knows every Snowflake construct.

`sql-dialect-fmt-hover` is LSP-agnostic. It returns a small `Hover` model with a byte
range, title, body, kind, and optional docs URL. The future LSP server should
adapt this model instead of duplicating hover text.

`tree-sitter-snowflake` is for editors and code hosts. It should be robust under
new Snowflake syntax. Add structure gradually only when it improves real editor
features such as folding, injections, or selection.

## Adding Snowflake Syntax

1. Add or confirm token support in `sql-dialect-fmt-lexer`.
2. Add keyword/type classification in `sql-dialect-fmt-syntax` or `sql-dialect-fmt-highlight`.
3. Add parser support only when structure matters.
4. Add hover/query support if it improves editor feedback.
5. Add focused tests and include a Snowflake docs link in the PR.
