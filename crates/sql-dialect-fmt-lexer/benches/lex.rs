//! Lexer throughput benchmarks.
//!
//! Run with `cargo bench -p sql-dialect-fmt-lexer`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sql_dialect_fmt_lexer::{tokenize_for_dialect, Dialect};
use sql_dialect_fmt_test_fixtures::EASY_CASES;

fn wide_select() -> String {
    let projection: Vec<String> = (0..128)
        .map(|i| format!("t.col_{i} AS alias_{i}"))
        .collect();
    format!(
        "SELECT {projection} FROM db.schema.table_name t WHERE t.payload:'k'::STRING IS NOT NULL",
        projection = projection.join(", "),
    )
}

fn quoted_bodies() -> String {
    String::from(
        "CREATE PROCEDURE p() RETURNS STRING LANGUAGE JAVASCRIPT AS $$ \
         var statement = snowflake.createStatement({sqlText: \"select 1\"}); \
         return statement.execute().next(); $$",
    )
}

fn databricks_paths() -> String {
    String::from(
        "CREATE TABLE `catalog`.`schema`.`table` USING DELTA LOCATION 'dbfs:/mnt/raw/path' \
         TBLPROPERTIES ('delta.enableChangeDataFeed' = 'true'); \
         SELECT * FROM delta.`/mnt/raw/path` WHERE a <=> b",
    )
}

fn bench_cases(c: &mut Criterion) {
    let cases: [(&str, Dialect, String); 3] = [
        ("wide_select", Dialect::Snowflake, wide_select()),
        ("quoted_bodies", Dialect::Snowflake, quoted_bodies()),
        ("databricks_paths", Dialect::Databricks, databricks_paths()),
    ];

    let mut group = c.benchmark_group("lex_statement");
    for (name, dialect, sql) in &cases {
        group.throughput(Throughput::Bytes(sql.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), sql, |b, sql| {
            b.iter(|| tokenize_for_dialect(std::hint::black_box(sql), *dialect));
        });
    }
    group.finish();
}

fn bench_corpus(c: &mut Criterion) {
    let total: usize = EASY_CASES.iter().map(|case| case.input.len()).sum();
    let mut group = c.benchmark_group("lex_corpus");
    group.throughput(Throughput::Bytes(total as u64));
    group.bench_function("easy_cases", |b| {
        b.iter(|| {
            for case in EASY_CASES {
                std::hint::black_box(tokenize_for_dialect(
                    std::hint::black_box(case.input),
                    Dialect::Snowflake,
                ));
            }
        });
    });
    group.finish();
}

criterion_group!(benches, bench_cases, bench_corpus);
criterion_main!(benches);
