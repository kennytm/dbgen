//! Evaluating compiled expressions into values.

use crate::{
    error::Error,
    parser::{Expr, Function},
    value::{Number, TryFromValue, Value, TIMESTAMP_FORMAT},
};
use chrono::{NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use rand::{
    distributions::{Bernoulli, BernoulliError},
    Rng, RngCore,
};
use rand_distr::{LogNormal, NormalError, Uniform};
use std::{
    borrow::Cow,
    cmp::Ordering,
    convert::{TryFrom, TryInto},
    fmt, usize,
};
use zipf::ZipfDistribution;

/// Environment information shared by all compilations
#[derive(Clone, Debug)]
pub struct CompileContext {
    /// The time zone used to interpret strings into timestamps.
    pub time_zone: Tz,
}

/// The external mutable state used during evaluation.
pub struct State {
    pub(crate) row_num: u64,
    rng: Box<dyn RngCore>,
    variables: Vec<Value>,
    compile_context: CompileContext,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("row_num", &self.row_num)
            .field("rng", &())
            .field("variables", &self.variables)
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
    /// - `variables_count`: Number of local variables per row.
    pub fn new(row_num: u64, rng: Box<dyn RngCore>, variables_count: usize, compile_context: CompileContext) -> Self {
        Self {
            row_num,
            rng,
            variables: vec![Value::Null; variables_count],
            compile_context,
        }
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
    /// Evaluates the row into a vector of values and updates the state.
    pub fn eval(&self, state: &mut State) -> Result<Vec<Value>, Error> {
        let result = self
            .0
            .iter()
            .map(|compiled| compiled.eval(state))
            .collect::<Result<_, _>>()?;
        state.row_num += 1;
        Ok(result)
    }
}

/// Interior of a compiled expression.
#[derive(Clone, Debug)]
enum C {
    /// The row number.
    RowNum,
    /// An evaluated constant.
    Constant(Value),
    /// An unevaluated function.
    RawFunction {
        /// Function name.
        name: Function,
        /// Function arguments.
        args: Vec<Compiled>,
    },
    /// Obtains a local variable.
    GetVariable(usize),
    /// Assigns a value to a local variable.
    SetVariable(usize, Box<Compiled>),
    /// The `CASE … WHEN` expression.
    CaseValueWhen {
        /// The value to match against.
        value: Box<Compiled>,
        /// The conditions and their corresponding results.
        conditions: Vec<(Compiled, Compiled)>,
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
}

/// A compiled expression
#[derive(Clone, Debug)]
pub struct Compiled(C);

impl TryFrom<Compiled> for Value {
    type Error = ();

    fn try_from(compiled: Compiled) -> Result<Self, Self::Error> {
        match compiled.0 {
            C::Constant(value) => Ok(value),
            _ => Err(()),
        }
    }
}

/// Extracts a single argument in a specific type.
fn arg<'a, T>(name: Function, args: &'a [Value], index: usize, default: Option<T>) -> Result<T, Error>
where
    T: TryFromValue<'a>,
{
    if let Some(arg) = args.get(index) {
        T::try_from_value(arg).ok_or(Error::InvalidArgumentType {
            name,
            index,
            expected: T::name(),
        })
    } else {
        default.ok_or(Error::NotEnoughArguments(name))
    }
}

/// Converts a slice of arguments all into a specific type.
fn iter_args<'a, T>(name: Function, args: &'a [Value]) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    T: TryFromValue<'a>,
{
    args.iter().enumerate().map(move |(index, arg)| {
        T::try_from_value(arg).ok_or(Error::InvalidArgumentType {
            name,
            index,
            expected: T::name(),
        })
    })
}

/// Extracts the arguments for the SQL SUBSTRING function.
fn args_substring(name: Function, args: &[Value], max: usize) -> Result<(usize, usize), Error> {
    let start = arg::<i64>(name, args, 1, None)? - 1;
    let length = arg::<Option<i64>>(name, args, 2, Some(None))?;
    if let Some(length) = length {
        let end = (start + length).try_into().unwrap_or(0);
        let start = start.try_into().unwrap_or(0);
        Ok((start.min(max), end.max(start).min(max)))
    } else {
        Ok((start.try_into().unwrap_or(0).min(max), max))
    }
}

impl CompileContext {
    /// Compiles an expression.
    pub fn compile(&self, expr: Expr) -> Result<Compiled, Error> {
        Ok(Compiled(match expr {
            Expr::RowNum => C::RowNum,
            Expr::Value(v) => C::Constant(v),
            Expr::GetVariable(index) => C::GetVariable(index),
            Expr::SetVariable(index, e) => C::SetVariable(index, Box::new(self.compile(*e)?)),
            Expr::Function { name, args } => {
                let args = args
                    .into_iter()
                    .map(|e| self.compile(e))
                    .collect::<Result<Vec<_>, _>>()?;
                if args.iter().all(|c| c.is_constant()) {
                    let args = args.into_iter().map(|c| c.try_into().unwrap()).collect::<Vec<Value>>();
                    compile_function(self, name, &args)?.0
                } else {
                    C::RawFunction { name, args }
                }
            }
            Expr::CaseValueWhen {
                value,
                conditions,
                otherwise,
            } => {
                let value = Box::new(self.compile(*value)?);
                let conditions = conditions
                    .into_iter()
                    .map(|(p, r)| Ok((self.compile(p)?, self.compile(r)?)))
                    .collect::<Result<_, Error>>()?;
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
            C::Constant(v) => v.clone(),
            C::RawFunction { name, args } => {
                let args = args.iter().map(|c| c.eval(state)).collect::<Result<Vec<_>, _>>()?;
                let compiled = compile_function(&state.compile_context, *name, &args)?;
                compiled.eval(state)?
            }

            C::GetVariable(index) => state.variables[*index].clone(),
            C::SetVariable(index, c) => {
                let value = c.eval(state)?;
                state.variables[*index] = value.clone();
                value
            }
            C::CaseValueWhen {
                value,
                conditions,
                otherwise,
            } => {
                let value = value.eval(state)?;
                for (p, r) in conditions {
                    let p = p.eval(state)?;
                    if value.sql_cmp(&p, Function::Eq)? == Some(Ordering::Equal) {
                        return r.eval(state);
                    }
                }
                otherwise.eval(state)?
            }

            C::RandRegex(generator) => {
                if generator.is_utf8() {
                    state.rng.sample::<String, _>(generator).into()
                } else {
                    state.rng.sample::<Vec<u8>, _>(generator).into()
                }
            }
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
        })
    }
}

/// Compiles a function with some value-like objects as input.
pub fn compile_function(ctx: &CompileContext, name: Function, args: &[Value]) -> Result<Compiled, Error> {
    macro_rules! require {
        ($e:expr, $($fmt:tt)+) => {
            #[allow(clippy::neg_cmp_op_on_partial_ord)] {
                if !$e {
                    return Err(Error::InvalidArguments { name, cause: format!($($fmt)+) });
                }
            }
        };
    }

    match name {
        Function::RandRegex => {
            let regex = arg(name, args, 0, None)?;
            let flags = arg(name, args, 1, Some(""))?;
            let max_repeat = arg(name, args, 2, Some(100))?;
            let generator = compile_regex_generator(regex, flags, max_repeat)?;
            Ok(Compiled(C::RandRegex(generator)))
        }

        Function::RandRange => {
            let lower = arg::<Number>(name, args, 0, None)?;
            let upper = arg::<Number>(name, args, 1, None)?;
            require!(lower < upper, "{} < {}", lower, upper);
            if let (Some(a), Some(b)) = (lower.to::<u64>(), upper.to::<u64>()) {
                Ok(Compiled(C::RandUniformU64(Uniform::new(a, b))))
            } else if let (Some(a), Some(b)) = (lower.to::<i64>(), upper.to::<i64>()) {
                Ok(Compiled(C::RandUniformI64(Uniform::new(a, b))))
            } else {
                Err(Error::IntegerOverflow(format!("rand.range({}, {})", lower, upper)))
            }
        }

        Function::RandRangeInclusive => {
            let lower = arg::<Number>(name, args, 0, None)?;
            let upper = arg::<Number>(name, args, 1, None)?;
            require!(lower <= upper, "{} <= {}", lower, upper);
            if let (Some(a), Some(b)) = (lower.to::<u64>(), upper.to::<u64>()) {
                Ok(Compiled(C::RandUniformU64(Uniform::new_inclusive(a, b))))
            } else if let (Some(a), Some(b)) = (lower.to::<i64>(), upper.to::<i64>()) {
                Ok(Compiled(C::RandUniformI64(Uniform::new_inclusive(a, b))))
            } else {
                Err(Error::IntegerOverflow(format!(
                    "rand.range_inclusive({}, {})",
                    lower, upper
                )))
            }
        }

        Function::RandUniform => {
            let lower = arg::<f64>(name, args, 0, None)?;
            let upper = arg::<f64>(name, args, 1, None)?;
            require!(lower < upper, "{} < {}", lower, upper);
            Ok(Compiled(C::RandUniformF64(Uniform::new(lower, upper))))
        }

        Function::RandUniformInclusive => {
            let lower = arg::<f64>(name, args, 0, None)?;
            let upper = arg::<f64>(name, args, 1, None)?;
            require!(lower <= upper, "{} <= {}", lower, upper);
            Ok(Compiled(C::RandUniformF64(Uniform::new_inclusive(lower, upper))))
        }

        Function::RandZipf => {
            let count = arg(name, args, 0, None)?;
            let exponent = arg(name, args, 1, None)?;
            require!(count > 0, "count being position (but we have {})", count);
            require!(exponent > 0.0, "exponent being positive (but we have {})", exponent);
            Ok(Compiled(C::RandZipf(ZipfDistribution::new(count, exponent).unwrap())))
        }

        Function::RandLogNormal => {
            let mean = arg(name, args, 0, None)?;
            let std_dev = arg::<f64>(name, args, 1, None)?.abs();
            Ok(Compiled(C::RandLogNormal(LogNormal::new(mean, std_dev).map_err(
                |NormalError::StdDevTooSmall| Error::InvalidArguments {
                    name,
                    cause: format!("{} (std_dev) >= 0", std_dev),
                },
            )?)))
        }

        Function::RandBool => {
            let p = arg(name, args, 0, None)?;
            Ok(Compiled(C::RandBool(Bernoulli::new(p).map_err(
                |BernoulliError::InvalidProbability| Error::InvalidArguments {
                    name,
                    cause: format!("0 <= {} (p) <= 1", p),
                },
            )?)))
        }

        Function::RandFiniteF32 => Ok(Compiled(C::RandFiniteF32(Uniform::new(0, 0xff00_0000)))),

        Function::RandFiniteF64 => Ok(Compiled(C::RandFiniteF64(Uniform::new(0, 0xffe0_0000_0000_0000)))),

        Function::RandU31Timestamp => Ok(Compiled(C::RandU31Timestamp(Uniform::new(1, 0x8000_0000)))),

        Function::Neg => {
            let inner = arg::<Number>(name, args, 0, None)?;
            Ok(Compiled(C::Constant((-inner).into())))
        }

        Function::Eq | Function::Ne | Function::Lt | Function::Le | Function::Gt | Function::Ge => {
            let lhs = arg::<&Value>(name, args, 0, None)?;
            let rhs = arg::<&Value>(name, args, 1, None)?;
            let answer = match lhs.sql_cmp(rhs, name)? {
                None => Value::Null,
                Some(Ordering::Less) => (name == Function::Ne || name == Function::Lt || name == Function::Le).into(),
                Some(Ordering::Equal) => (name == Function::Le || name == Function::Eq || name == Function::Ge).into(),
                Some(Ordering::Greater) => {
                    (name == Function::Ge || name == Function::Gt || name == Function::Ne).into()
                }
            };
            Ok(Compiled(C::Constant(answer)))
        }

        Function::Is | Function::IsNot => {
            let lhs = arg::<&Value>(name, args, 0, None)?;
            let rhs = arg::<&Value>(name, args, 1, None)?;
            let is_eq = lhs == rhs;
            let should_eq = name == Function::Is;
            Ok(Compiled(C::Constant((is_eq == should_eq).into())))
        }

        Function::Not => {
            let inner = arg::<Option<bool>>(name, args, 0, None)?;
            Ok(Compiled(C::Constant(inner.map(|b| !b).into())))
        }

        Function::And | Function::Or => {
            let identity_value = name == Function::And;
            let mut result = Some(identity_value);

            for arg in iter_args::<Option<bool>>(name, args) {
                if let Some(v) = arg? {
                    if v == identity_value {
                        continue;
                    } else {
                        return Ok(Compiled(C::Constant(v.into())));
                    }
                } else {
                    result = None;
                }
            }
            Ok(Compiled(C::Constant(result.into())))
        }

        Function::Concat => {
            let result = Value::try_sql_concat(iter_args::<&Value>(name, args).map(|item| item.map(Value::clone)))?;
            Ok(Compiled(C::Constant(result)))
        }

        Function::Add | Function::Sub | Function::Mul | Function::FloatDiv => {
            let func = match name {
                Function::Add => Value::sql_add,
                Function::Sub => Value::sql_sub,
                Function::Mul => Value::sql_mul,
                Function::FloatDiv => Value::sql_float_div,
                _ => unreachable!(),
            };

            let result =
                iter_args::<&Value>(name, args).try_fold(None::<Cow<'_, Value>>, |accum, cur| -> Result<_, Error> {
                    let cur = cur?;
                    Ok(Some(if let Some(prev) = accum {
                        Cow::Owned(func(&*prev, cur)?)
                    } else {
                        Cow::Borrowed(cur)
                    }))
                });
            Ok(Compiled(C::Constant(
                result?.expect("at least 1 argument").into_owned(),
            )))
        }

        Function::Timestamp => {
            let input = arg(name, args, 0, None)?;
            let tz = ctx.time_zone;
            let timestamp = tz
                .datetime_from_str(input, TIMESTAMP_FORMAT)
                .map_err(|source| Error::InvalidTimestampString {
                    timestamp: input.to_owned(),
                    source,
                })?
                .naive_utc();
            Ok(Compiled(C::Constant(Value::Timestamp(timestamp, tz))))
        }

        Function::TimestampTz => {
            let mut input = arg::<&str>(name, args, 0, None)?;
            let tz = match input.find(|c: char| c.is_ascii_alphabetic()) {
                None => ctx.time_zone,
                Some(i) => {
                    let tz = input[i..]
                        .parse::<Tz>()
                        .map_err(|cause| Error::InvalidArguments { name, cause })?;
                    input = input[..i].trim_end();
                    tz
                }
            };
            let timestamp = tz
                .datetime_from_str(input, TIMESTAMP_FORMAT)
                .map_err(|source| Error::InvalidTimestampString {
                    timestamp: input.to_owned(),
                    source,
                })?
                .naive_utc();
            Ok(Compiled(C::Constant(Value::Timestamp(timestamp, tz))))
        }

        Function::Greatest | Function::Least => {
            let mut res = &Value::Null;
            for value in iter_args::<&Value>(name, args) {
                let value = value?;
                let should_replace = match value.sql_cmp(res, name)? {
                    Some(Ordering::Greater) => name == Function::Greatest,
                    Some(Ordering::Less) => name == Function::Least,
                    None => res == &Value::Null,
                    _ => false,
                };
                if should_replace {
                    res = value;
                }
            }
            Ok(Compiled(C::Constant(res.clone())))
        }

        Function::Round => {
            let value = arg::<f64>(name, args, 0, None)?;
            let digits = arg::<i32>(name, args, 1, Some(0))?;
            let scale = 10.0_f64.powi(digits);
            let result = (value * scale).round() / scale;
            Ok(Compiled(C::Constant(result.into())))
        }

        Function::SubstringChars => {
            let input = arg::<&str>(name, args, 0, None)?;
            #[allow(clippy::replace_consts)] // FIXME: allow this lint until usize::MAX becomes an assoc const
            let (start, end) = args_substring(name, args, usize::MAX)?;
            Ok(Compiled(C::Constant(
                input.chars().take(end).skip(start).collect::<String>().into(),
            )))
        }

        Function::SubstringBytes => {
            let input = arg::<&[u8]>(name, args, 0, None)?;
            let (start, end) = args_substring(name, args, input.len())?;
            Ok(Compiled(C::Constant(input[start..end].to_vec().into())))
        }
    }
}

fn compile_regex_generator(regex: &str, flags: &str, max_repeat: u32) -> Result<rand_regex::Regex, Error> {
    let mut parser = regex_syntax::ParserBuilder::new();
    for flag in flags.chars() {
        match flag {
            'o' => parser.octal(true),
            'a' => parser.allow_invalid_utf8(true).unicode(false),
            'u' => parser.allow_invalid_utf8(false).unicode(true),
            'x' => parser.ignore_whitespace(true),
            'i' => parser.case_insensitive(true),
            'm' => parser.multi_line(true),
            's' => parser.dot_matches_new_line(true),
            'U' => parser.swap_greed(true),
            _ => return Err(Error::UnknownRegexFlag(flag)),
        };
    }

    let hir = parser.build().parse(regex).map_err(|source| Error::InvalidRegex {
        pattern: regex.to_owned(),
        source: source.into(),
    })?;
    let gen = rand_regex::Regex::with_hir(hir, max_repeat).map_err(|source| Error::InvalidRegex {
        pattern: regex.to_owned(),
        source,
    })?;
    Ok(gen)
}
