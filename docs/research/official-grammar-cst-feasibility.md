<!-- i18n: language-switcher -->
[English](official-grammar-cst-feasibility.md) | [日本語](official-grammar-cst-feasibility.ja.md)

# Generating a Pure Rust CST Parser from Official Grammars: Feasibility

Last checked: 2026-09-24. Tracking issue: [#121](https://github.com/hjosugi/sql-dialect-fmt/issues/121).
First assessment: 2026-07 ([issue comment](https://github.com/hjosugi/sql-dialect-fmt/issues/121)).

## Question

Can a machine-readable, official grammar be fed to a generator that emits the Pure Rust lossless
CST parser this project needs, replacing the handwritten event/rowan parser? If not today, what
would have to change?

A generated parser would have to keep every guarantee the current one gives:

1. **Byte-exact lossless CST.** Every byte, including whitespace and comments, is a token in the
   tree (`event.rs` re-inserts trivia while building).
2. **Total error recovery.** Every input produces a tree plus diagnostics; malformed regions are
   kept, never dropped (`parser.rs`, fuel-bounded).
3. **Positional contextual keywords.** A word is a keyword only where the grammar position says so
   (`contextual.rs`: `AT`, `MATCH_RECOGNIZE`, `GROUPING SETS`, `COMMENT ON`, ...).
4. **Pure Rust, Wasm-friendly, fast and maintainable** enough for the CLI, LSP, VS Code extension and
   Wasm build.

## Verdict

- **The trigger is still not met.** As of 2026-09-24 neither Snowflake nor Databricks publishes the
  grammar its own parser is built from. The re-evaluation trigger in [ROADMAP.md](../../ROADMAP.md)
  (research item 5) is unchanged.
- **Keep the handwritten event/rowan parser as the source of truth**, and keep third-party grammars
  as conformance oracles (`scripts/grammar-oracle-report.py`, weekly `External grammar oracles`
  job). The last scheduled run (2026-09-21) was green: all 51 grammars-v4 Snowflake examples and
  all 373 Spark SQL test inputs passed the lossless/idempotency harness.
- **Even an official grammar would not be a drop-in generator input.** The only working precedent,
  `postgresql-cst-parser`, wrote its own lexer generator, parser generator and automata crates
  around PostgreSQL's implementation grammar. A published grammar is necessary, not sufficient.

## 1. Which dialects publish a machine-readable grammar

### The dialects this project formats

| Source | Dialect | Format | License | Implementation grammar? | Activity | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| [Snowflake SQL reference](https://docs.snowflake.com/en/sql-reference) | Snowflake | HTML syntax blocks | docs | No | continuous | The only official description. Not a grammar file, and not what Snowflake's parser is built from. |
| [Snowflake-Labs/lezer-snowsql](https://github.com/Snowflake-Labs/lezer-snowsql) | Snowflake | Lezer grammar + 828-line JS tokenizer | Apache-2.0 | No | last push 2023-12-04 | A Snowflake Labs editor grammar, not a supported product. Its README still lists `SELECT`, `ALTER`, `INSERT`, `DELETE`, `MERGE`, `SET` and `CALL` as unimplemented. |
| [antlr/grammars-v4 `sql/snowflake`](https://github.com/antlr/grammars-v4/tree/master/sql/snowflake) | Snowflake | ANTLR4 | MIT (file header) | No, written from docs by the community | active (`7e08234262`, 2026-09-21) | 699 parser and 980 lexer rules, with no actions (grammars-v4 policy). Already used as an oracle. |
| [fivetran/zetasql-snowflake](https://github.com/fivetran/zetasql-snowflake) | Snowflake | ZetaSQL fork | Apache-2.0 | No | last push 2023-09-25 | Stale analyzer fork. |
| [bytebase/omni](https://github.com/bytebase/omni) | Snowflake and others | Handwritten Go recursive descent | MIT | No | active | Code, not a grammar. |
| [Apache Spark `SqlBaseParser.g4` / `SqlBaseLexer.g4`](https://github.com/apache/spark/tree/master/sql/api/src/main/antlr4/org/apache/spark/sql/catalyst/parser) | Databricks (via Spark) | ANTLR4 with Java actions | Apache-2.0 | Yes for Apache Spark; Databricks Runtime is a fork and its extensions are not in it | active (`4cfbd41196`, 2026-09-24) | 313 parser and 527 lexer rules. Already used as an oracle. |
| [sqlfluff dialects](https://github.com/sqlfluff/sqlfluff/tree/main/src/sqlfluff/dialects) | Snowflake, Databricks | Python segment DSL | MIT | No | active | Readable only as Python code. Used for the keyword and segment checklists. |

### Dialects that do publish their implementation grammar (for comparison)

| Dialect | Grammar | License | What it shows |
| --- | --- | --- | --- |
| PostgreSQL | `gram.y` + `scan.l` (Bison/Flex) | PostgreSQL License | The case that made [`postgresql-cst-parser`](https://github.com/future-architect/postgresql-cst-parser) possible. |
| GoogleSQL / BigQuery | [`googlesql/parser/googlesql.tm`](https://github.com/google/googlesql) (Textmapper; the project was formerly ZetaSQL) | Apache-2.0 | An official implementation grammar can exist and still target a generator (Textmapper) with no Rust backend. |
| Trino | [`core/trino-grammar/.../SqlBase.g4`](https://github.com/trinodb/trino) | Apache-2.0 | Same shape as Spark: ANTLR4 plus Java runtime hooks. |

## 2. Spike: what a generator would have to consume

The weekly oracle report now includes a **Generator Hazards** table
([docs/CORPUS.md](../CORPUS.md#external-grammar-oracles)). It counts the constructs that are code
rather than data, so a Rust generator cannot take them from the grammar as-is. Numbers at the
revisions pinned above:

| Grammar | Parser rules | Lexer rules | Semantic predicates | Inline actions | Named actions | Lexer modes | Trivia off tree | Keyword fallback rules (alternatives) |
| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- |
| grammars-v4 Snowflake | 699 | 980 | 0 | 0 | — | 0 | 4 | `non_reserved_words` 142, `keyword` 52 |
| Apache Spark SQL | 313 | 527 | 28 | 8 | `@header`, `@members` | 1 | 3 | `nonReserved` 445, `ansiNonReserved` 379, `strictNonReserved` 16 |
| Snowflake-Labs lezer-snowsql | 361 rules | — | — | — | — | — | `@skip` block | 2 external specializers backed by the JS tokenizer |

What the numbers mean:

- **Trivia is dropped by design in every candidate.** Both ANTLR grammars send whitespace and
  comments to `channel(HIDDEN)`, and Lezer uses `@skip`. That is the opposite of requirement 1: a
  generated parser would need a custom trivia layer, which is exactly the part `event.rs` already
  implements.
- **Spark's implementation grammar is only complete together with its Java.** Its 28 predicates
  reference runtime SQL settings declared in `@members`: `SQL_standard_keyword_behavior` (ANSI
  mode), `legacy_setops_precedence_enabled`, `legacy_exponent_literal_as_decimal_enabled`,
  `double_quoted_identifiers`, `parameter_substitution_enabled`, `legacy_identifier_clause_only`.
  They also call helpers such as `isValidDecimal()`, `isHint()`, `isShiftRightOperator()` and
  `isOperatorPipeStart()`. Every one of them would have to be ported by hand and kept in sync.
- **Keyword handling is a static list, not a position.** Spark switches between three fallback lists
  (445 / 379 / 16 alternatives) by ANSI mode. grammars-v4 keeps one 142-alternative
  `non_reserved_words` rule. Neither expresses "`AT` is a keyword only after a table reference",
  which `contextual.rs` does per position.
- **grammars-v4 Snowflake is mechanically consumable** (no actions), but the community writes it
  from the documentation, with no promise to keep pace with Snowflake releases. It does not meet
  the trigger.

Reproduce with the same sparse checkouts the `Corpus` workflow uses:

```sh
scripts/grammar-oracle-report.py \
  --grammars-v4 /path/to/grammars-v4 \
  --spark /path/to/spark \
  --sqlfluff /path/to/sqlfluff
# see the "Generator Hazards" section of target/grammar-oracle-report.md
```

## 3. Rust tooling options (crates.io, 2026-09-24)

| Tool | Latest | Grammar input | Lossless trivia | Error recovery | Contextual keywords | Fit |
| --- | --- | --- | --- | --- | --- | --- |
| [tree-sitter](https://crates.io/crates/tree-sitter) | 0.27.0 (2026-08-30) | its own JS DSL, generated C | comments float as `extras` | heuristic, can change between versions | external scanner (C) | Kept only for highlighting and currently paused (ROADMAP). A C runtime works against the Pure Rust/Wasm goal. |
| [antlr4rust](https://crates.io/crates/antlr4rust) | 0.5.2 (2025-10-25); the earlier `antlr-rust` stopped at 0.2.2 (2022) | ANTLR4 `.g4` | hidden channel, not in the tree | ANTLR default sync/recover | predicates must be rewritten in Rust | Not an official ANTLR target, needs the JVM tool to generate, maintained again since 2025-10 but with little track record. |
| [lrpar / cfgrammar](https://crates.io/crates/lrpar) (grmtools) | 0.15.0 (2026-07-22) | Yacc/Bison-style `.y` ("mostly unchanged") | lexer-dependent; needs a custom layer | automatic CPCT+ repair sequences | lexer/grammar tricks | **The best generic fit if a vendor ever publishes a Bison/Yacc implementation grammar** (the `gram.y` scenario). Repairs are token insert/delete sequences, so they would still have to become diagnostics that keep all source bytes. |
| [lalrpop](https://crates.io/crates/lalrpop) | 0.23.1 (2026-03-11) | its own LR DSL | no | limited | no | Ruff moved off it in v0.4.0 (2024) for speed and recovery. |
| [pest](https://crates.io/crates/pest) / [chumsky](https://crates.io/crates/chumsky) | 2.9.2 / 0.13.0 | PEG / combinators written by hand | no / by hand | no / yes | by hand | The grammar is rewritten by hand, so an official grammar brings no benefit. |
| [rowan](https://crates.io/crates/rowan) / [cstree](https://crates.io/crates/cstree) | 0.17.0 / 0.14.0 | tree libraries | yes | n/a | n/a | Orthogonal: any generated parser would still build one of these. `rowan` is what we use; `postgresql-cst-parser` uses `cstree`. |
| handwritten event/rowan (current) | — | Rust code | yes (requirement 1) | total (requirement 2) | positional (requirement 3) | Same design as rust-analyzer, Biome and Ruff. |

## 4. The precedent: `postgresql-cst-parser`

[`future-architect/postgresql-cst-parser`](https://github.com/future-architect/postgresql-cst-parser)
(MIT, 0.2.0 on crates.io, last push 2026-06-23) is the only Pure Rust CST parser generated from an
official SQL grammar. It took:

- PostgreSQL's own `gram.y` and `scan.l`, with the
  [libpg_query](https://github.com/pganalyze/libpg_query) patches applied;
- `scan.l` rewritten for Rust by hand;
- three purpose-built crates in the repository (`lexer-generator`, `parser-generator`, `automata`)
  that build the parse table and a `cstree` tree whose nodes follow `gram.y` rule names.

Its public `parse()` returns a `Result`: it does not promise a tree for malformed input. That is
fine for a PostgreSQL formatter, which formats valid SQL, but falls short of requirement 2 for an
LSP and editor extension.

## 5. What would change the verdict

| Event | Action |
| --- | --- |
| Snowflake publishes the grammar its own parser is generated from (any format) | Prototype on one statement family: generate the parser (with `lrpar` if it is Yacc/Bison), build the CST through the existing event layer, then measure lossless round-trip on the fixtures, recovery-diagnostic parity with the malformed suite, and Wasm size and speed against the current parser. Migrate only if all four requirements hold. |
| Databricks publishes its Runtime grammar | The same prototype. Budget for porting the Spark-style predicates listed above and for the ANSI-mode keyword split. |
| A new community grammar, a docs-derived ANTLR grammar, or an official grammar that is not the implementation | Not a trigger. Add it as a conformance oracle instead. |

## 6. How this stays current

- The weekly `External grammar oracles` job re-counts the hazards at upstream `HEAD` and uploads
  them with the corpus results. A jump in predicates or actions, or a new source, is the cue to
  revisit this note.
- Re-check the Snowflake-side sources above whenever Snowflake release notes mention parser or
  grammar tooling. `gh api repos/<owner>/<repo> --jq .pushed_at` is enough to see whether the
  community sources are still maintained.
- The trigger itself lives in [ROADMAP.md](../../ROADMAP.md) (research item 5).
