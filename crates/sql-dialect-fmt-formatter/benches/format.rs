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
//! The `embedded` benchmark covers a realistic JavaScript routine body, and the final `corpus`
//! benchmark formats the whole embedded golden set, so a regression on any real fixture is visible
//! too. Run with `cargo bench -p sql-dialect-fmt-formatter`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sql_dialect_fmt_formatter::{format, Dialect, FormatOptions};
use sql_dialect_fmt_test_fixtures::{javascript_routine_trailing_whitespace_input, EASY_CASES};

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

/// A Snowflake Scripting block with repeated lenient statements and a structured IF.
fn scripting_block() -> String {
    let mut sql = String::from("declare v number default 0; begin ");
    for i in 0..20 {
        sql.push_str(&format!("let v := v + {i}; "));
    }
    sql.push_str("if (v > 100) then return v; else return 0; end if; end");
    sql
}

/// A semantic view with model clauses, AI clauses, tags, and copy grants.
fn semantic_view() -> String {
    String::from(
        "create semantic view sv tables(orders as mart.orders primary key(order_id), \
         customers as mart.customers primary key(customer_id)) \
         relationships(order_customer as orders(customer_id) references customers) \
         facts(public orders.net_amount as net_amount) \
         dimensions(public customers.region as region) \
         metrics(public orders.revenue as sum(orders.net_amount)) \
         ai_sql_generation 'Use revenue for sales questions.' \
         ai_question_categorization 'Classify revenue questions.' \
         ai_verified_queries(top_revenue as(question 'Top revenue?' verified_at 1767225600 \
         onboarding_question true verified_by 'analyst@example.com' sql 'SELECT 1')) \
         with tag(governance.owner = 'analytics') copy grants",
    )
}

/// Databricks-specific DML with extended MERGE branches.
fn databricks_merge() -> String {
    String::from(
        "merge into catalog.schema.target t using catalog.schema.source s on t.id = s.id \
         when matched and s.op = 'U' then update set t.a = s.a, t.b = s.b \
         when matched and s.op = 'D' then delete \
         when not matched by source then delete \
         when not matched then insert (id, a, b) values (s.id, s.a, s.b)",
    )
}

fn bench_statements(c: &mut Criterion) {
    let snowflake = FormatOptions::default();
    let databricks = FormatOptions::default().with_dialect(Dialect::Databricks);
    let cases: [(&str, String, FormatOptions); 7] = [
        ("wide_select", wide_select(), snowflake),
        ("cte_chain", cte_chain(), snowflake),
        ("ddl", ddl(), snowflake),
        ("merge", merge(), snowflake),
        ("scripting_block", scripting_block(), snowflake),
        ("semantic_view", semantic_view(), snowflake),
        ("databricks_merge", databricks_merge(), databricks),
    ];

    let mut group = c.benchmark_group("format_statement");
    for (name, sql, opts) in &cases {
        group.throughput(Throughput::Bytes(sql.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), sql, |b, sql| {
            b.iter(|| format(std::hint::black_box(sql), opts));
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

fn bench_javascript_routine(c: &mut Criterion) {
    let opts = FormatOptions::default();
    let sql = javascript_routine_trailing_whitespace_input();
    let mut group = c.benchmark_group("format_embedded");
    group.throughput(Throughput::Bytes(sql.len() as u64));
    group.bench_function("javascript_routine", |b| {
        b.iter(|| format(std::hint::black_box(&sql), &opts));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_statements,
    bench_corpus,
    bench_javascript_routine
);
criterion_main!(benches);
