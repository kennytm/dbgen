use criterion::{black_box, criterion_group, criterion_main, Bencher, Criterion};
use dbgen::{
    eval::{CompileContext, State},
    format::{Format, SqlFormat},
    parser::Template,
    span::Registry,
};
use rand::SeedableRng;
use rand_hc::Hc128Rng;
use std::{
    fs::read_to_string,
    io::{sink, Write},
};

fn run_benchmark(b: &mut Bencher<'_>, path: &str) {
    let mut registry = Registry::default();
    let mut template = Template::parse(&read_to_string(path).unwrap(), &[], None, &mut registry).unwrap();
    let ctx = CompileContext::new(template.variables_count);
    let row = ctx.compile_row(template.tables.swap_remove(0).exprs).unwrap();
    let mut state = State::new(1, Box::new(Hc128Rng::from_seed([0x41; 32])), ctx);
    let format = SqlFormat::default();
    let mut sink: Box<dyn Write> = Box::new(sink());

    b.iter(move || {
        let values = black_box(&row).eval(black_box(&mut state)).unwrap();
        for value in values {
            format.write_value(black_box(&mut *sink), &value).unwrap();
        }
    });
}

fn bench_templates(c: &mut Criterion) {
    c.bench_function("sysbench_oltp_uniform", |b| {
        run_benchmark(b, "res/sysbench/oltp_uniform_mysql.sql");
    });
}

criterion_group!(benches, bench_templates);
criterion_main!(benches);
