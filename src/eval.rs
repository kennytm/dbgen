//! Evaluating compiled expressions into values.

use crate::{
    error::Error,
    functions::{Arguments, Function},
    parser::{Expr, QName},
    value::Value,
};
use chrono::NaiveDateTime;
use chrono_tz::Tz;
use rand::{distributions::Bernoulli, seq::SliceRandom, Rng, RngCore};
use rand_distr::{LogNormal, Uniform};
use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
    fmt,
    sync::Arc,
};
use zipf::ZipfDistribution;

/// Environment information shared by all compilations
#[derive(Clone, Debug)]
pub struct CompileContext {
    /// The time zone used to interpret strings into timestamps.
    pub time_zone: Tz,
    /// The current timestamp in UTC.
    pub current_timestamp: NaiveDateTime,
    /// The global variables.
    pub variables: Box<[Value]>,
}

impl CompileContext {
    /// Creates a default compile context storing the given number of variables.
    pub fn new(variables_count: usize) -> Self {
        Self {
            time_zone: Tz::UTC,
            current_timestamp: NaiveDateTime::from_timestamp(0, 0),
            variables: vec![Value::Null; variables_count].into_boxed_slice(),
        }
    }
}

/// The external mutable state used during evaluation.
pub struct State {
    pub(crate) row_num: u64,
    /// Defines the value of `subrownum`.
    pub sub_row_num: u64,
    rng: Box<dyn RngCore>,
    compile_context: CompileContext,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("row_num", &self.row_num)
            .field("sub_row_num", &self.sub_row_num)
            .field("rng", &())
            .field("variables", &self.compile_context.variables)
            .finish()
    }
}

impl State {
    /// Creates a new state.
    ///
    /// # Parameters
    ///
    /// - `row_num`: The starting row number in this state. The first file should have this set
    ///     to 1, and the second to `rows_count * inserts_count + 1`, etc.
    /// - `rng`: The seeded random number generator.
    pub fn new(row_num: u64, rng: Box<dyn RngCore>, compile_context: CompileContext) -> Self {
        Self {
            row_num,
            sub_row_num: 1,
            rng,
            compile_context,
        }
    }

    /// Extracts the compile context from the state.
    pub fn into_compile_context(self) -> CompileContext {
        self.compile_context
    }

    /// Increases the rownum by 1.
    pub fn increase_row_num(&mut self) {
        self.row_num += 1;
    }
}

/// A compiled table
#[derive(Debug)]
pub struct Table {
    /// Table name.
    pub name: QName,
    /// Content of table schema.
    pub content: String,
    /// Compiled row.
    pub row: Row,
    /// Information of dervied tables (index, and number of rows to generate)
    pub derived: Vec<(usize, Compiled)>,
}

impl CompileContext {
    /// Compiles a table.
    pub fn compile_table(&self, table: crate::parser::Table) -> Result<Table, Error> {
        Ok(Table {
            name: table.name,
            content: table.content,
            row: self.compile_row(table.exprs)?,
            derived: table
                .derived
                .into_iter()
                .map(|(i, e)| self.compile(e).map(|c| (i, c)))
                .collect::<Result<_, _>>()?,
        })
    }
}

/// Represents a row of compiled values.
#[derive(Debug)]
pub struct Row(Vec<Compiled>);

impl CompileContext {
    /// Compiles a vector of parsed expressions into a row.
    pub fn compile_row(&self, exprs: Vec<Expr>) -> Result<Row, Error> {
        Ok(Row(exprs
            .into_iter()
            .map(|e| self.compile(e))
            .collect::<Result<Vec<_>, Error>>()?))
    }
}

impl Row {
    /// Evaluates the row into a vector of values.
    pub fn eval(&self, state: &mut State) -> Result<Vec<Value>, Error> {
        let mut result = Vec::with_capacity(self.0.len());
        for compiled in &self.0 {
            result.push(compiled.eval(state)?);
        }
        Ok(result)
    }
}

/// Interior of a compiled expression.
#[derive(Clone, Debug)]
pub(crate) enum C {
    /// The row number.
    RowNum,
    /// The derived row number.
    SubRowNum,
    /// An evaluated constant.
    Constant(Value),
    /// An unevaluated function.
    RawFunction {
        /// The function.
        function: &'static dyn Function,
        /// Function arguments.
        args: Box<[Compiled]>,
    },
    /// Obtains a local variable.
    GetVariable(usize),
    /// Assigns a value to a local variable.
    SetVariable(usize, Box<Compiled>),
    /// The `CASE … WHEN` expression.
    CaseValueWhen {
        /// The value to match against.
        value: Option<Box<Compiled>>,
        /// The conditions and their corresponding results.
        conditions: Box<[(Compiled, Compiled)]>,
        /// The result when all conditions failed.
        otherwise: Box<Compiled>,
    },

    /// Regex-based random string.
    RandRegex(rand_regex::Regex),
    /// Uniform distribution for `u64`.
    RandUniformU64(Uniform<u64>),
    /// Uniform distribution for `i64`.
    RandUniformI64(Uniform<i64>),
    /// Uniform distribution for `f64`.
    RandUniformF64(Uniform<f64>),
    /// Zipfian distribution.
    RandZipf(ZipfDistribution),
    /// Log-normal distribution.
    RandLogNormal(LogNormal<f64>),
    /// Bernoulli distribution for `bool` (i.e. a weighted random boolean).
    RandBool(Bernoulli),
    /// Random f32 with uniform bit pattern
    RandFiniteF32(Uniform<u32>),
    /// Random f64 with uniform bit pattern
    RandFiniteF64(Uniform<u64>),
    /// Random u31 timestamp
    RandU31Timestamp(Uniform<i64>),
    /// Random shuffled array
    RandShuffle(Arc<[Value]>),
    /// Random (version 4) UUID
    RandUuid,
}

/// A compiled expression
#[derive(Clone, Debug)]
pub struct Compiled(pub(crate) C);

impl TryFrom<Compiled> for Value {
    type Error = ();

    fn try_from(compiled: Compiled) -> Result<Self, Self::Error> {
        match compiled.0 {
            C::Constant(value) => Ok(value),
            _ => Err(()),
        }
    }
}

impl CompileContext {
    /// Compiles an expression.
    pub fn compile(&self, expr: Expr) -> Result<Compiled, Error> {
        Ok(Compiled(match expr {
            Expr::RowNum => C::RowNum,
            Expr::SubRowNum => C::SubRowNum,
            Expr::CurrentTimestamp => C::Constant(Value::Timestamp(self.current_timestamp, self.time_zone)),
            Expr::Value(v) => C::Constant(v),
            Expr::GetVariable(index) => C::GetVariable(index),
            Expr::SetVariable(index, e) => C::SetVariable(index, Box::new(self.compile(*e)?)),
            Expr::Function { function, args } => {
                let args = args
                    .into_iter()
                    .map(|e| self.compile(e))
                    .collect::<Result<Vec<_>, _>>()?;
                if args.iter().all(Compiled::is_constant) {
                    let args = args.into_iter().map(|c| c.try_into().unwrap()).collect();
                    function.compile(self, args)?.0
                } else {
                    C::RawFunction {
                        function,
                        args: args.into_boxed_slice(),
                    }
                }
            }
            Expr::CaseValueWhen {
                value,
                conditions,
                otherwise,
            } => {
                let value = value.map(|v| Ok::<_, Error>(Box::new(self.compile(*v)?))).transpose()?;
                let conditions = conditions
                    .into_iter()
                    .map(|(p, r)| Ok((self.compile(p)?, self.compile(r)?)))
                    .collect::<Result<Vec<_>, Error>>()?
                    .into_boxed_slice();
                let otherwise = Box::new(if let Some(o) = otherwise {
                    self.compile(*o)?
                } else {
                    Compiled(C::Constant(Value::Null))
                });
                C::CaseValueWhen {
                    value,
                    conditions,
                    otherwise,
                }
            }
        }))
    }
}

impl Compiled {
    /// Returns whether this compiled value is a constant.
    pub fn is_constant(&self) -> bool {
        match self.0 {
            C::Constant(_) => true,
            _ => false,
        }
    }

    /// Evaluates a compiled expression and updates the state. Returns the evaluated value.
    pub fn eval(&self, state: &mut State) -> Result<Value, Error> {
        Ok(match &self.0 {
            C::RowNum => state.row_num.into(),
            C::SubRowNum => state.sub_row_num.into(),
            C::Constant(v) => v.clone(),
            C::RawFunction { function, args } => {
                let mut eval_args = Arguments::with_capacity(args.len());
                for c in &**args {
                    eval_args.push(c.eval(state)?);
                }
                let compiled = (*function).compile(&state.compile_context, eval_args)?;
                compiled.eval(state)?
            }
            C::GetVariable(index) => state.compile_context.variables[*index].clone(),
            C::SetVariable(index, c) => {
                let value = c.eval(state)?;
                state.compile_context.variables[*index] = value.clone();
                value
            }

            C::CaseValueWhen {
                value: Some(value),
                conditions,
                otherwise,
            } => {
                let value = value.eval(state)?;
                for (p, r) in &**conditions {
                    let p = p.eval(state)?;
                    if value.sql_cmp(&p, "when")? == Some(Ordering::Equal) {
                        return r.eval(state);
                    }
                }
                otherwise.eval(state)?
            }

            C::CaseValueWhen {
                value: None,
                conditions,
                otherwise,
            } => {
                for (p, r) in &**conditions {
                    if p.eval(state)?.is_sql_true("when")? {
                        return r.eval(state);
                    }
                }
                otherwise.eval(state)?
            }

            C::RandRegex(generator) => state.rng.sample::<Result<_, _>, _>(generator).into(),
            C::RandUniformU64(uniform) => state.rng.sample(uniform).into(),
            C::RandUniformI64(uniform) => state.rng.sample(uniform).into(),
            C::RandUniformF64(uniform) => state.rng.sample(uniform).into(),
            C::RandZipf(zipf) => (state.rng.sample(zipf) as u64).into(),
            C::RandLogNormal(log_normal) => state.rng.sample(log_normal).into(),
            C::RandBool(bern) => u64::from(state.rng.sample(bern)).into(),
            C::RandFiniteF32(uniform) => f32::from_bits(state.rng.sample(uniform).rotate_right(1)).into(),
            C::RandFiniteF64(uniform) => f64::from_bits(state.rng.sample(uniform).rotate_right(1)).into(),

            C::RandU31Timestamp(uniform) => {
                let seconds = state.rng.sample(uniform);
                let timestamp = NaiveDateTime::from_timestamp(seconds, 0);
                Value::new_timestamp(timestamp, state.compile_context.time_zone)
            }

            C::RandShuffle(array) => {
                let mut shuffled_array = Arc::<[Value]>::from(&**array);
                Arc::get_mut(&mut shuffled_array).unwrap().shuffle(&mut state.rng);
                Value::Array(shuffled_array)
            }

            C::RandUuid => {
                // we will loss 6 bits but that's still uniform.
                let g = state.rng.gen::<[u16; 8]>();
                format!(
                    "{:04x}{:04x}-{:04x}-4{:03x}-{:04x}-{:04x}{:04x}{:04x}",
                    g[0],
                    g[1],
                    g[2],
                    g[3] & 0xfff,
                    (g[4] & 0x3fff) | 0x8000,
                    g[5],
                    g[6],
                    g[7],
                )
                .into()
            }
        })
    }
}
