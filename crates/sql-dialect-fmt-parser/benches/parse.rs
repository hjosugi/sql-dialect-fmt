//! Parser throughput benchmarks.
//!
//! Run with `cargo bench -p sql-dialect-fmt-parser`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sql_dialect_fmt_parser::{parse_with_dialect, Dialect};
use sql_dialect_fmt_test_fixtures::EASY_CASES;

fn wide_select() -> String {
    let projection: Vec<String> = (0..96)
        .map(|i| format!("sum(t.measure_{i}) AS measure_{i}"))
        .collect();
    format!(
        "SELECT {projection} FROM fact_events t JOIN dim_users u ON u.id = t.user_id \
         WHERE t.event_date >= '2024-01-01' AND t.status IN ('ok', 'retry') \
         GROUP BY u.region, u.segment HAVING count(*) > 10 ORDER BY u.region, u.segment",
        projection = projection.join(", "),
    )
}

fn long_ddl() -> String {
    let columns: Vec<String> = (0..80)
        .map(|i| format!("column_{i} NUMBER(18, 4) DEFAULT 0 COMMENT 'generated column {i}'"))
        .collect();
    format!(
        "CREATE OR REPLACE TABLE analytics.large_table ({columns})",
        columns = columns.join(", "),
    )
}

fn scripting_block() -> String {
    let mut sql = String::from("DECLARE\nv NUMBER DEFAULT 0;\nBEGIN\n");
    for i in 0..32 {
        sql.push_str(&format!("LET v := v + {i};\n"));
    }
    sql.push_str("IF (v > 100) THEN\nRETURN v;\nELSE\nRETURN 0;\nEND IF;\nEND");
    sql
}

fn databricks_delta() -> String {
    String::from(
        "MERGE INTO catalog.schema.target t USING catalog.schema.source s ON t.id = s.id \
         WHEN MATCHED AND s.op = 'U' THEN UPDATE SET t.a = s.a, t.b = s.b \
         WHEN NOT MATCHED BY SOURCE THEN DELETE \
         WHEN NOT MATCHED THEN INSERT (id, a, b) VALUES (s.id, s.a, s.b)",
    )
}

fn bench_cases(c: &mut Criterion) {
    let cases: [(&str, Dialect, String); 4] = [
        ("wide_select", Dialect::Snowflake, wide_select()),
        ("long_ddl", Dialect::Snowflake, long_ddl()),
        ("scripting_block", Dialect::Snowflake, scripting_block()),
        ("databricks_delta", Dialect::Databricks, databricks_delta()),
    ];

    let mut group = c.benchmark_group("parse_statement");
    for (name, dialect, sql) in &cases {
        group.throughput(Throughput::Bytes(sql.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), sql, |b, sql| {
            b.iter(|| parse_with_dialect(std::hint::black_box(sql), *dialect));
        });
    }
    group.finish();
}

fn bench_corpus(c: &mut Criterion) {
    let total: usize = EASY_CASES.iter().map(|case| case.input.len()).sum();
    let mut group = c.benchmark_group("parse_corpus");
    group.throughput(Throughput::Bytes(total as u64));
    group.bench_function("easy_cases", |b| {
        b.iter(|| {
            for case in EASY_CASES {
                std::hint::black_box(parse_with_dialect(
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
