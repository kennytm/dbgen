use chrono::NaiveDateTime;
use dbgen::{
    error::Error,
    eval::{CompileContext, Schema, State},
    format::{Format, SqlFormat},
    parser::Template,
    span::{Registry, ResultExt, S},
    value::{Value, TIMESTAMP_FORMAT},
    writer::{Env, Writer},
    FULL_VERSION,
};
use rand::{Rng, SeedableRng};
use rand_hc::Hc128Rng;
use serde::Serialize;
use std::{convert::TryFrom, mem};
use wasm_bindgen::prelude::*;

#[derive(Default)]
struct TableWriter {
    rows: Vec<Vec<String>>,
}

#[derive(Serialize)]
struct Table {
    name: String,
    column_names: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Writer for TableWriter {
    fn write_value(&mut self, value: &Value) -> Result<(), S<Error>> {
        let mut output = Vec::new();
        SqlFormat::default().write_value(&mut output, value).unwrap_throw();
        let output = String::from_utf8(output).unwrap_throw();
        self.rows.last_mut().unwrap_throw().push(output);
        Ok(())
    }

    fn write_file_header(&mut self, _: &Schema<'_>) -> Result<(), S<Error>> {
        Ok(())
    }

    fn write_header(&mut self, _: &Schema<'_>) -> Result<(), S<Error>> {
        self.write_row_separator()
    }

    fn write_value_header(&mut self, _: &str) -> Result<(), S<Error>> {
        Ok(())
    }

    fn write_value_separator(&mut self) -> Result<(), S<Error>> {
        Ok(())
    }

    fn write_row_separator(&mut self) -> Result<(), S<Error>> {
        let columns = self.rows.last().map_or(0, |r| r.len());
        self.rows.push(Vec::with_capacity(columns));
        Ok(())
    }

    fn write_trailer(&mut self) -> Result<(), S<Error>> {
        Ok(())
    }
}

fn try_generate_rows(
    template: &str,
    rows: usize,
    now: &str,
    seed: &[u8],
    span_registry: &mut Registry,
) -> Result<Vec<Table>, S<Error>> {
    let now = NaiveDateTime::parse_from_str(now, TIMESTAMP_FORMAT).no_span_err()?;
    let seed = <&<Hc128Rng as SeedableRng>::Seed>::try_from(seed)
        .map_err(|e| Error::InvalidArguments(format!("invalid seed: {}", e)))
        .no_span_err()?;

    let template = Template::parse(template, &[], None, span_registry)?;
    let mut ctx = CompileContext::new(template.variables_count);
    ctx.current_timestamp = now;
    let tables = template
        .tables
        .into_iter()
        .map(|t| ctx.compile_table(t))
        .collect::<Result<Vec<_>, _>>()?;

    // we perform this double seeding to be compatible with the CLI.
    let mut seeding_rng = Hc128Rng::from_seed(*seed);
    let mut rng = move || Box::new(Hc128Rng::from_seed(seeding_rng.gen()));

    if !template.global_exprs.is_empty() {
        let row_gen = ctx.compile_row(template.global_exprs)?;
        let mut state = State::new(0, rng(), ctx);
        row_gen.eval(&mut state)?;
        ctx = state.into_compile_context();
    }

    let mut state = State::new(1, rng(), ctx);
    let mut env = Env::new(&tables, &mut state, false, |_| Ok(TableWriter::default()))?;
    for _ in 0..rows {
        env.write_row()?;
    }

    Ok(env
        .tables()
        .map(|(table, writer)| {
            let schema = table.schema(false);
            Table {
                name: schema.name.to_owned(),
                column_names: schema.column_names().map(|s| s.to_owned()).collect(),
                rows: mem::take(&mut writer.rows),
            }
        })
        .collect())
}

#[wasm_bindgen]
pub fn generate_rows(template: &str, rows: usize, now: &str, seed: &[u8]) -> Result<JsValue, JsValue> {
    let mut registry = Registry::default();
    match try_generate_rows(template, rows, now, seed, &mut registry) {
        Ok(result) => JsValue::from_serde(&result).map_err(|e| e.to_string().into()),
        Err(e) => Err(registry.describe(&e).into()),
    }
}

#[wasm_bindgen]
pub fn version() -> String {
    FULL_VERSION.to_owned()
}
