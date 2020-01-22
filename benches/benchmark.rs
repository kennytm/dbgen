use anyhow::Error;
use criterion::{black_box, criterion_group, criterion_main, Bencher, Criterion};
use dbgen::{
    eval::{CompileContext, State},
    format::{Format, SqlFormat},
    parser::Template,
    value::Value,
};
use rand::SeedableRng;
use rand_hc::Hc128Rng;
use std::{
    fs::read_to_string,
    io::{sink, Write},
};

fn run_benchmark(b: &mut Bencher<'_>, path: &str) -> Result<(), Error> {
    let template = Template::parse(&read_to_string(path)?)?;
    let ctx = CompileContext {
        variables: vec![Value::Null; template.variables_count],
        ..CompileContext::default()
    };
    let row = ctx.compile_row(template.exprs)?;
    let mut state = State::new(1, Box::new(Hc128Rng::from_seed([0x41; 32])), ctx);
    let format = SqlFormat {
        escape_backslash: false,
    };
    let mut sink: Box<dyn Write> = Box::new(sink());

    b.iter(move || -> Result<(), Error> {
        let values = black_box(&row).eval(black_box(&mut state))?;
        for value in values {
            format.write_value(black_box(&mut *sink), &value)?;
        }
        Ok(())
    });

    Ok(())
}

fn bench_templates(c: &mut Criterion) {
    c.bench_function("sysbench_oltp_uniform", |b| {
        run_benchmark(b, "res/sysbench/oltp_uniform_mysql.sql").unwrap();
    });
}

criterion_group!(benches, bench_templates);
criterion_main!(benches);
