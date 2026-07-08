# Development

The workspace is split into small crates so parser, formatter, editor, and distribution changes can
be tested at the layer that owns the behavior.

```text
source bytes
  -> sql-dialect-fmt-encoding
  -> sql-dialect-fmt-lexer
  -> sql-dialect-fmt-parser
  -> sql-dialect-fmt-formatter
```

## Standard Gates

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo bench -p sql-dialect-fmt-formatter --bench format -- --test
```

## Fuzzing

```sh
cargo install cargo-fuzz --locked
cargo +nightly fuzz run lexer_roundtrip
cargo +nightly fuzz run parser_lossless
cargo +nightly fuzz run formatter_idempotent
```

The scheduled fuzz workflow runs the same targets and uploads crash artifacts.

## Corpus

The committed formatter corpus lives in
`crates/sql-dialect-fmt-formatter/tests/corpus_sample/`. Larger private or generated corpora should
use the external harness:

```sh
scripts/run-external-corpus.sh --path /path/to/sqls --limit 500
scripts/conformance-report.py --path crates/sql-dialect-fmt-formatter/tests/corpus_sample \
  --out target/conformance-report.md
```

See also `docs/ARCHITECTURE.md`, `docs/TESTING.md`, and `docs/CORPUS.md` in the repository.
