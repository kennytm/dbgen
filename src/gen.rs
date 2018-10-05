use data_encoding::HEXUPPER;
use failure::ResultExt;
use rand::{distributions as distr, prng::Hc128Rng, Rng, SeedableRng};
use std::io::Write;
use std::{i64, str, u64};

use crate::error::{Error, ErrorKind};
use crate::parser::{Expr, Function, Template};
use crate::quote::Quote;

#[derive(Clone)]
enum Value {
    Integer(i128),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
}

impl Value {
    fn write_sql(&self, mut output: impl Write) -> Result<(), Error> {
        match self {
            Value::Integer(i) => write!(output, "{}", i),
            Value::Float(f) => write!(output, "{}", f),
            Value::String(s) => output.write_all(&Quote::Single.escape_bytes(s.as_bytes())),
            Value::Bytes(b) => write!(output, "x'{}'", HEXUPPER.encode(b)),
        }
        .context(ErrorKind::WriteSqlValue)?;
        Ok(())
    }
}

type DefaultRng = Hc128Rng;
pub type RngSeed = <Hc128Rng as SeedableRng>::Seed;

pub struct Generator {
    escaped_table_name: String,
    template: Template,
}

impl Generator {
    pub fn new(template: Template, quote: Quote, table_name: &str) -> Self {
        Generator {
            escaped_table_name: quote.escape(table_name),
            template,
        }
    }

    pub fn write_sql_schema(&self, mut output: impl Write) -> Result<(), Error> {
        writeln!(
            output,
            "CREATE TABLE {} {}",
            self.escaped_table_name, self.template.table_content
        )
        .context(ErrorKind::WriteSqlSchema)?;
        Ok(())
    }
}

enum Compiled {
    RowNum,
    Constant(Value),
    RawFunction { name: Function, args: Vec<Compiled> },

    RandI8(u32),
    RandI16(u32),
    RandI32(u32),
    RandI64(u32),
    RandU8(u32),
    RandU16(u32),
    RandU32(u32),
    RandU64(u32),
    RandRegex(crate::regex::Compiled),
    RandURange(distr::Uniform<u64>),
    RandIRange(distr::Uniform<i64>),
    RandUniform(distr::Uniform<f64>),
}

trait ArgExtract {
    fn extract_u32(&self) -> Option<u32>;
    fn extract_i128(&self) -> Option<i128>;
    fn extract_f64(&self) -> Option<f64>;
    fn extract_str(&self) -> Option<&str>;
    fn extract_value(&self) -> Option<&Value>;
}

impl ArgExtract for Value {
    fn extract_u32(&self) -> Option<u32> {
        match *self {
            Value::Integer(i) if 0 <= i && i <= 0xffff_ffff => Some(i as u32),
            _ => None,
        }
    }
    fn extract_i128(&self) -> Option<i128> {
        match *self {
            Value::Integer(i) => Some(i),
            _ => None,
        }
    }
    fn extract_f64(&self) -> Option<f64> {
        match *self {
            Value::Integer(i) => Some(i as f64),
            Value::Float(f) => Some(f),
            _ => None,
        }
    }
    fn extract_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            Value::Bytes(b) => str::from_utf8(b).ok(),
            _ => None,
        }
    }
    fn extract_value(&self) -> Option<&Value> {
        Some(self)
    }
}

impl ArgExtract for Compiled {
    fn extract_u32(&self) -> Option<u32> {
        match self {
            Compiled::Constant(v) => v.extract_u32(),
            _ => None,
        }
    }
    fn extract_i128(&self) -> Option<i128> {
        match self {
            Compiled::Constant(v) => v.extract_i128(),
            _ => None,
        }
    }
    fn extract_f64(&self) -> Option<f64> {
        match self {
            Compiled::Constant(v) => v.extract_f64(),
            _ => None,
        }
    }
    fn extract_str(&self) -> Option<&str> {
        match self {
            Compiled::Constant(v) => v.extract_str(),
            _ => None,
        }
    }
    fn extract_value(&self) -> Option<&Value> {
        match self {
            Compiled::Constant(v) => Some(v),
            _ => None,
        }
    }
}

fn precompile_function<T: ArgExtract>(name: Function, args: &[T]) -> Result<Compiled, Error> {
    macro_rules! define_getter {
        ($get:ident, $get_opt:ident, $extract:ident, $name:expr) => {
            let $get = |name, index: usize| -> Result<_, Error> {
                let arg = args.get(index).ok_or(ErrorKind::NotEnoughArguments(name))?;
                let arg = arg.$extract().ok_or(ErrorKind::InvalidArgumentType {
                    name,
                    index,
                    expected: $name,
                })?;
                Ok(arg)
            };
            let $get_opt = |name, index: usize, mut value| -> Result<_, Error> {
                if let Some(arg) = args.get(index) {
                    value = arg.$extract().ok_or(ErrorKind::InvalidArgumentType {
                        name,
                        index,
                        expected: $name,
                    })?;
                }
                Ok(value)
            };
        };
    }

    define_getter!(get_u32, get_u32_opt, extract_u32, "unsigned integer");
    define_getter!(get_i128, get_i128_opt, extract_i128, "integer");
    define_getter!(get_f64, get_f64_opt, extract_f64, "number");
    define_getter!(get_str, get_str_opt, extract_str, "string");
    define_getter!(get_value, get_value_opt, extract_value, "constant");

    match name {
        Function::RandInt | Function::RandUInt => {
            let bits = get_u32(name, 0)?;
            compile_rand_int(name, bits)
        }
        Function::RandRegex => {
            let regex = get_str(name, 0)?;
            let flags = get_str_opt(name, 1, "")?;
            let max_repeat = get_u32_opt(name, 2, 100)?;
            crate::regex::Compiled::new(regex, flags, max_repeat).map(Compiled::RandRegex)
        }
        Function::RandRange | Function::RandRangeInclusive => {
            let lower = get_i128(name, 0)?;
            let upper = get_i128(name, 1)?;
            let is_inclusive = name == Function::RandRangeInclusive;
            if lower == upper && is_inclusive {
                Ok(Compiled::Constant(Value::Integer(lower)))
            } else if !(lower < upper) {
                Err(ErrorKind::InvalidArguments {
                    name,
                    cause: format!("limits are wrongly ordered, expecting {} < {}", lower, upper),
                }
                .into())
            } else if 0 <= lower && upper <= i128::from(u64::MAX) {
                Ok(Compiled::RandURange(compile_uniform(
                    lower as u64,
                    upper as u64,
                    is_inclusive,
                )))
            } else if i128::from(i64::MIN) <= lower && upper <= i128::from(i64::MAX) {
                Ok(Compiled::RandIRange(compile_uniform(
                    lower as i64,
                    upper as i64,
                    is_inclusive,
                )))
            } else {
                Err(ErrorKind::IntegerOverflow(format!("{}({}, {})", name, lower, upper)).into())
            }
        }
        Function::RandUniform | Function::RandUniformInclusive => {
            let lower = get_f64(name, 0)?;
            let upper = get_f64(name, 1)?;
            let is_inclusive = name == Function::RandUniformInclusive;
            if lower == upper && is_inclusive {
                Ok(Compiled::Constant(Value::Float(lower)))
            } else if !(lower < upper) {
                Err(ErrorKind::InvalidArguments {
                    name,
                    cause: format!("limits are wrongly ordered, expecting {} < {}", lower, upper),
                }
                .into())
            } else {
                Ok(Compiled::RandUniform(compile_uniform(lower, upper, is_inclusive)))
            }
        }

        Function::Neg => {
            let value = get_value(name, 0)?;
            match value {
                Value::Integer(i) => Ok(Compiled::Constant(Value::Integer(-i))),
                Value::Float(f) => Ok(Compiled::Constant(Value::Float(-f))),
                _ => Err(ErrorKind::InvalidArgumentType {
                    name,
                    index: 0,
                    expected: "number",
                }
                .into()),
            }
        }
    }
}

struct State {
    row_num: u64,
    rng: DefaultRng,
}

impl Compiled {
    fn compile(expr: Expr) -> Result<Self, Error> {
        Ok(match expr {
            Expr::RowNum => Compiled::RowNum,
            Expr::Integer(u) => Compiled::Constant(Value::Integer(u.into())),
            Expr::Float(f) => Compiled::Constant(Value::Float(f)),
            Expr::String(s) => Compiled::Constant(Value::String(s)),
            Expr::Function { name, args } => {
                let args = args.into_iter().map(Compiled::compile).collect::<Result<Vec<_>, _>>()?;
                match precompile_function(name, &args) {
                    Ok(c) => c,
                    Err(e) => match e.kind() {
                        ErrorKind::InvalidArgumentType { name: n, .. } if *n == name => {
                            Compiled::RawFunction { name, args }
                        }
                        _ => return Err(e),
                    },
                }
            }
        })
    }

    fn eval(&self, state: &mut State) -> Result<Value, Error> {
        Ok(match self {
            Compiled::RowNum => Value::Integer(state.row_num.into()),
            Compiled::Constant(v) => v.clone(),
            Compiled::RawFunction { name, args } => {
                let args = args.iter().map(|c| c.eval(state)).collect::<Result<Vec<_>, _>>()?;
                let compiled = precompile_function(*name, &args)?;
                compiled.eval(state)?
            }

            Compiled::RandI8(shift) => Value::Integer((state.rng.gen::<i8>() >> shift).into()),
            Compiled::RandI16(shift) => Value::Integer((state.rng.gen::<i16>() >> shift).into()),
            Compiled::RandI32(shift) => Value::Integer((state.rng.gen::<i32>() >> shift).into()),
            Compiled::RandI64(shift) => Value::Integer((state.rng.gen::<i64>() >> shift).into()),
            Compiled::RandU8(shift) => Value::Integer((state.rng.gen::<u8>() >> shift).into()),
            Compiled::RandU16(shift) => Value::Integer((state.rng.gen::<u16>() >> shift).into()),
            Compiled::RandU32(shift) => Value::Integer((state.rng.gen::<u32>() >> shift).into()),
            Compiled::RandU64(shift) => Value::Integer((state.rng.gen::<u64>() >> shift).into()),

            Compiled::RandRegex(compiled) => {
                let mut result = Vec::new();
                compiled.eval_into(&mut state.rng, &mut result);
                match String::from_utf8(result) {
                    Ok(s) => Value::String(s),
                    Err(e) => Value::Bytes(e.into_bytes()),
                }
            }

            Compiled::RandIRange(d) => Value::Integer(state.rng.sample(d).into()),
            Compiled::RandURange(d) => Value::Integer(state.rng.sample(d).into()),
            Compiled::RandUniform(d) => Value::Float(state.rng.sample(d)),
        })
    }
}

fn compile_rand_int(name: Function, bits: u32) -> Result<Compiled, Error> {
    let gen_bits = match bits {
        0 => return Ok(Compiled::Constant(Value::Integer(0))),
        1..=8 => 8,
        9..=16 => 16,
        17..=32 => 32,
        33..=64 => 64,
        _ => return Err(ErrorKind::IntegerOverflow(format!("1<<{}", bits)).into()),
    };
    let f = match (name, gen_bits) {
        (Function::RandInt, 8) => Compiled::RandI8,
        (Function::RandInt, 16) => Compiled::RandI16,
        (Function::RandInt, 32) => Compiled::RandI32,
        (Function::RandInt, 64) => Compiled::RandI64,
        (Function::RandUInt, 8) => Compiled::RandU8,
        (Function::RandUInt, 16) => Compiled::RandU16,
        (Function::RandUInt, 32) => Compiled::RandU32,
        (Function::RandUInt, 64) => Compiled::RandU64,
        _ => unreachable!(),
    };
    Ok(f(gen_bits - bits))
}

fn compile_uniform<T>(lower: T, upper: T, inclusive: bool) -> distr::Uniform<T>
where
    T: distr::uniform::SampleUniform,
{
    if inclusive {
        distr::Uniform::new_inclusive(lower, upper)
    } else {
        distr::Uniform::new(lower, upper)
    }
}

pub struct CompiledGenerator {
    state: State,
    escaped_table_name: String,
    compiled: Vec<Compiled>,
}

impl Generator {
    pub fn compile(self, seed: RngSeed) -> Result<CompiledGenerator, Error> {
        Ok(CompiledGenerator {
            compiled: self
                .template
                .exprs
                .into_iter()
                .map(Compiled::compile)
                .collect::<Result<Vec<_>, _>>()?,
            state: State {
                row_num: 1,
                rng: DefaultRng::from_seed(seed),
            },
            escaped_table_name: self.escaped_table_name,
        })
    }
}

impl CompiledGenerator {
    pub fn write_sql(&mut self, mut output: impl Write, rows_per_insert: u32) -> Result<(), Error> {
        write!(output, "INSERT INTO {} VALUES ", self.escaped_table_name).context(ErrorKind::WriteSqlData)?;

        for row_index in 0..rows_per_insert {
            output.write_all(b"(").context(ErrorKind::WriteSqlData)?;
            for (i, compiled) in self.compiled.iter().enumerate() {
                if i != 0 {
                    output.write_all(b", ").context(ErrorKind::WriteSqlData)?;
                }
                let value = compiled.eval(&mut self.state).context(ErrorKind::WriteSqlData)?;
                value.write_sql(&mut output).context(ErrorKind::WriteSqlData)?;
            }

            let sep = if row_index == rows_per_insert - 1 {
                b");\n"
            } else {
                b"), "
            };
            output.write_all(sep).context(ErrorKind::WriteSqlData)?;

            self.state.row_num += 1;
        }

        Ok(())
    }
}
