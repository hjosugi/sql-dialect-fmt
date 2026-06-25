//! Throughput benchmarks for the SQL formatter.
//!
//! These cover the representative statement shapes whose layout exercises the hot paths in the
//! Doc printer — width measurement, the fit/break decision, and forced-break propagation:
//!
//! * a **wide SELECT** with a long projection list and several clauses (lots of group fitting),
//! * a **CTE chain** (nested `WITH` queries, each a multi-clause SELECT),
//! * **DDL** (a `CREATE TABLE` with many column definitions — a big parenthesized list), and
//! * a **MERGE** (multiple `WHEN MATCHED` / `WHEN NOT MATCHED` branches).
//!
//! A final `corpus` benchmark formats the whole embedded golden set, so a regression on any real
//! fixture is visible too. Run with `cargo bench -p snow-fmt-formatter`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use snow_fmt_formatter::{format, FormatOptions};
use snow_fmt_test_fixtures::EASY_CASES;

/// A wide SELECT: a long projection plus WHERE / GROUP BY / ORDER BY, so many groups are measured.
fn wide_select() -> String {
    let cols: Vec<String> = (0..40).map(|i| format!("t.col_{i} AS alias_{i}")).collect();
    format!(
        "select {projection} \
         from schema.big_table t \
         join other o on o.id = t.id \
         where t.status = 'active' and t.amount > 100 and t.created_at >= '2020-01-01' \
         group by t.region, t.segment \
         having count(*) > 10 \
         order by t.region, t.segment desc \
         limit 100",
        projection = cols.join(", "),
    )
}

/// A CTE chain: several nested `WITH` queries feeding a final SELECT.
fn cte_chain() -> String {
    let mut sql = String::from("with ");
    let stages: Vec<String> = (0..8)
        .map(|i| {
            format!(
                "stage_{i} as (select a, b, sum(c) as total from src_{i} where flag = {i} group by a, b)"
            )
        })
        .collect();
    sql.push_str(&stages.join(", "));
    sql.push_str(
        " select s0.a, s7.total from stage_0 s0 join stage_7 s7 on s0.a = s7.a order by s7.total desc",
    );
    sql
}

/// A `CREATE TABLE` with many columns: a large parenthesized list that wraps one-per-line.
fn ddl() -> String {
    let cols: Vec<String> = (0..30)
        .map(|i| format!("column_{i} varchar(255) not null default ''"))
        .collect();
    format!(
        "create or replace table analytics.dimension_table ({cols})",
        cols = cols.join(", "),
    )
}

/// A MERGE with several matched / not-matched branches.
fn merge() -> String {
    String::from(
        "merge into target t using source s on t.id = s.id \
         when matched and s.op = 'U' then update set t.a = s.a, t.b = s.b, t.c = s.c \
         when matched and s.op = 'D' then delete \
         when not matched and s.op = 'I' then insert (id, a, b, c) values (s.id, s.a, s.b, s.c)",
    )
}

fn bench_statements(c: &mut Criterion) {
    let opts = FormatOptions::default();
    let cases: [(&str, String); 4] = [
        ("wide_select", wide_select()),
        ("cte_chain", cte_chain()),
        ("ddl", ddl()),
        ("merge", merge()),
    ];

    let mut group = c.benchmark_group("format_statement");
    for (name, sql) in &cases {
        group.throughput(Throughput::Bytes(sql.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), sql, |b, sql| {
            b.iter(|| format(std::hint::black_box(sql), &opts));
        });
    }
    group.finish();
}

fn bench_corpus(c: &mut Criterion) {
    let opts = FormatOptions::default();
    let total: usize = EASY_CASES.iter().map(|case| case.input.len()).sum();
    let mut group = c.benchmark_group("format_corpus");
    group.throughput(Throughput::Bytes(total as u64));
    group.bench_function("easy_cases", |b| {
        b.iter(|| {
            for case in EASY_CASES {
                std::hint::black_box(format(std::hint::black_box(case.input), &opts));
            }
        });
    });
    group.finish();
}

criterion_group!(benches, bench_statements, bench_corpus);
criterion_main!(benches);
