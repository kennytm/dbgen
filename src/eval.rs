//! Evaluating compiled expressions into values.

use crate::{
    array::{Array, Permutation},
    error::Error,
    functions::{Arguments, Function},
    parser::{Expr, QName},
    span::{ResultExt, S, Span, SpanExt},
    value::Value,
};
use chrono::{DateTime, NaiveDateTime};
use rand::{Rng, RngCore, distributions::Bernoulli};
use rand_distr::{LogNormal, Uniform, Zipf, weighted_alias::WeightedAliasIndex};
use rand_regex::EncodedString;
use std::{fmt, ops::Range, sync::Arc};

/// Environment information shared by all compilations
#[derive(Clone, Debug)]
pub struct CompileContext {
    /// The current timestamp in UTC.
    pub current_timestamp: NaiveDateTime,
    /// The global variables.
    pub variables: Box<[Value]>,
}

impl CompileContext {
    /// Creates a default compile context storing the given number of variables.
    pub fn new(variables_count: usize) -> Self {
        Self {
            current_timestamp: NaiveDateTime::MIN,
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
    /// The ranges in `content` which column names appear.
    pub column_name_ranges: Vec<Range<usize>>,
    /// Compiled row.
    pub row: Row,
    /// Information of dervied tables (index, and number of rows to generate)
    pub derived: Vec<(usize, Compiled)>,
}

/// The schema information extracted from the compiled table.
#[derive(Debug, Copy, Clone)]
pub struct Schema<'a> {
    /// Table name (qualified or unqualified).
    pub name: &'a str,
    /// Content of table schema.
    pub content: &'a str,
    /// The ranges in `content` which column names appear.
    column_name_ranges: &'a [Range<usize>],
}

impl Schema<'_> {
    /// Returns an iterator of column names associated with the table.
    pub fn column_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.column_name_ranges.iter().map(move |r| &self.content[r.clone()])
    }
}

impl Table {
    /// Gets the schema associated with the table.
    pub fn schema(&self, qualified: bool) -> Schema<'_> {
        Schema {
            name: self.name.table_name(qualified),
            content: &self.content,
            column_name_ranges: &self.column_name_ranges,
        }
    }
}

impl CompileContext {
    /// Compiles a table.
    pub fn compile_table(&self, table: crate::parser::Table) -> Result<Table, S<Error>> {
        Ok(Table {
            name: table.name,
            content: table.content,
            column_name_ranges: table.column_name_ranges,
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
    pub fn compile_row(&self, exprs: Vec<S<Expr>>) -> Result<Row, S<Error>> {
        Ok(Row(exprs
            .into_iter()
            .map(|e| self.compile(e))
            .collect::<Result<_, _>>()?))
    }
}

impl Row {
    /// Evaluates the row into a vector of values.
    pub fn eval(&self, state: &mut State) -> Result<Vec<Value>, S<Error>> {
        let mut result = Vec::with_capacity(self.0.len());
        for compiled in &self.0 {
            result.push(compiled.eval(state)?);
        }
        Ok(result)
    }
}

/// Interior of a compiled expression.
#[derive(Clone, Debug)]
pub enum C {
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
    /// A conditional expression.
    ///
    /// The inner array stores the condition and their corresponding results.
    Conditions(Box<[(Compiled, Compiled)]>),

    /// Regex-based random string.
    RandRegex(rand_regex::Regex),
    /// Uniform distribution for `u64`.
    RandUniformU64(Uniform<u64>),
    /// Uniform distribution for `i64`.
    RandUniformI64(Uniform<i64>),
    /// Uniform distribution for `f64`.
    RandUniformF64(Uniform<f64>),
    /// Zipfian distribution.
    RandZipf(Zipf<f64>),
    /// Log-normal distribution.
    RandLogNormal(LogNormal<f64>),
    /// Bernoulli distribution for `bool` (i.e. a weighted random boolean).
    RandBool(Bernoulli),
    /// Weighted distribution
    RandWeighted(WeightedAliasIndex<f64>),
    /// Random f32 with uniform bit pattern
    RandFiniteF32(Uniform<u32>),
    /// Random f64 with uniform bit pattern
    RandFiniteF64(Uniform<u64>),
    /// Random u31 timestamp
    RandU31Timestamp(Uniform<i64>),
    /// Random shuffled array
    RandShuffle {
        /// The cached permutation.
        permutation: Box<Permutation>,
        /// The pre-shuffled array.
        inner: Arc<Array>,
    },
    /// Random (version 4) UUID
    RandUuid,
}

impl C {
    fn span(self, span: Span) -> Compiled {
        Compiled(S { span, inner: self })
    }
}

/// A compiled expression
#[derive(Clone, Debug)]
pub struct Compiled(pub(crate) S<C>);

impl CompileContext {
    /// Compiles an expression.
    pub fn compile(&self, expr: S<Expr>) -> Result<Compiled, S<Error>> {
        Ok(match expr.inner {
            Expr::RowNum => C::RowNum,
            Expr::SubRowNum => C::SubRowNum,
            Expr::CurrentTimestamp => C::Constant(Value::Timestamp(self.current_timestamp)),
            Expr::Value(v) => C::Constant(v),
            Expr::GetVariable(index) => C::GetVariable(index),
            Expr::SetVariable(index, e) => C::SetVariable(index, Box::new(self.compile(*e)?)),
            Expr::Function { function, args } => {
                let args = args
                    .into_iter()
                    .map(|e| self.compile(e))
                    .collect::<Result<Vec<_>, _>>()?;
                if args.iter().all(Compiled::is_constant) {
                    let args = args
                        .into_iter()
                        .map(|c| match c.0.inner {
                            C::Constant(v) => v.span(c.0.span),
                            _ => unreachable!(),
                        })
                        .collect();
                    function.compile(self, expr.span, args)?
                } else {
                    C::RawFunction {
                        function,
                        args: args.into_boxed_slice(),
                    }
                }
            }
            Expr::Conditions(conditions) => C::Conditions(
                IntoIterator::into_iter(conditions)
                    .map(|(p, r)| Ok((self.compile(p)?, self.compile(r)?)))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
        }
        .span(expr.span))
    }
}

impl Compiled {
    /// Returns whether this compiled value is a constant.
    pub fn is_constant(&self) -> bool {
        matches!(self.0.inner, C::Constant(_))
    }

    /// Evaluates a compiled expression and updates the state. Returns the evaluated value.
    pub fn eval(&self, state: &mut State) -> Result<Value, S<Error>> {
        let span = self.0.span;
        Ok(match &self.0.inner {
            C::RowNum => state.row_num.into(),
            C::SubRowNum => state.sub_row_num.into(),
            C::Constant(v) => v.clone(),
            C::RawFunction { function, args } => {
                let mut eval_args = Arguments::with_capacity(args.len());
                for c in &**args {
                    eval_args.push(c.eval(state)?.span(c.0.span));
                }
                (*function)
                    .compile(&state.compile_context, span, eval_args)?
                    .span(span)
                    .eval(state)?
            }
            C::GetVariable(index) => state.compile_context.variables[*index].clone(),
            C::SetVariable(index, c) => {
                let value = c.eval(state)?;
                state.compile_context.variables[*index] = value.clone();
                value
            }

            C::Conditions(conditions) => {
                for (p, r) in &**conditions {
                    if p.eval(state)?.is_sql_true().span_err(p.0.span)? {
                        return r.eval(state);
                    }
                }
                Value::Null
            }

            C::RandRegex(generator) => state.rng.sample::<EncodedString, _>(generator).into(),
            C::RandUniformU64(uniform) => state.rng.sample(uniform).into(),
            C::RandUniformI64(uniform) => state.rng.sample(uniform).into(),
            C::RandUniformF64(uniform) => Value::from_finite_f64(state.rng.sample(uniform)),
            C::RandZipf(zipf) => (state.rng.sample(zipf) as u64).into(),
            C::RandLogNormal(log_normal) => Value::from_finite_f64(state.rng.sample(log_normal)),
            C::RandBool(bern) => state.rng.sample(bern).into(),
            C::RandWeighted(weighted) => (state.rng.sample(weighted) + 1).into(),
            C::RandFiniteF32(uniform) => {
                Value::from_finite_f64(f32::from_bits(state.rng.sample(uniform).rotate_right(1)).into())
            }
            C::RandFiniteF64(uniform) => {
                Value::from_finite_f64(f64::from_bits(state.rng.sample(uniform).rotate_right(1)))
            }

            C::RandU31Timestamp(uniform) => {
                let seconds = state.rng.sample(uniform);
                let timestamp = DateTime::from_timestamp(seconds, 0)
                    .expect("u31 range of timestamp must be valid")
                    .naive_utc();
                Value::new_timestamp(timestamp)
            }

            C::RandShuffle { permutation, inner } => {
                let mut permutation = permutation.clone();
                permutation.shuffle(inner.len(), &mut state.rng);
                Value::Array(inner.add_permutation(*permutation))
            }

            C::RandUuid => {
                // we will loss 6 bits but that's still uniform.
                let g = state.rng.r#gen::<[u16; 8]>();
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
