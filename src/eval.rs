//! Evaluating compiled expressions into values.

use chrono::NaiveDateTime;
use crate::{
    error::{Error, ErrorKind},
    parser::{Expr, Function},
    value::{Number, TryFromValue, Value, TIMESTAMP_FORMAT},
};
use failure::ResultExt;
use rand::{
    distributions::{self, Uniform},
    Rng, RngCore,
};
use std::{borrow::Cow, cmp::Ordering, fmt};
use zipf::ZipfDistribution;

/// The external mutable state used during evaluation.
pub struct State {
    pub(crate) row_num: u64,
    rng: Box<dyn RngCore>,
    variables: Vec<Value>,
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
    pub fn new(row_num: u64, rng: Box<dyn RngCore>, variables_count: usize) -> Self {
        Self {
            row_num,
            rng,
            variables: vec![Value::Null; variables_count],
        }
    }
}

/// Represents a row of compiled values.
#[derive(Debug)]
pub struct Row(Vec<Compiled>);

impl Row {
    /// Compiles a vector of parsed expressions into a row.
    pub fn compile(exprs: Vec<Expr>) -> Result<Self, Error> {
        Ok(Row(exprs
            .into_iter()
            .map(Compiled::compile)
            .collect::<Result<Vec<_>, Error>>()?))
    }

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
    RandLogNormal(distributions::LogNormal),
    /// Bernoulli distribution for `bool` (i.e. a weighted random boolean).
    RandBool(distributions::Bernoulli),
}

/// A compiled expression
#[derive(Clone, Debug)]
pub struct Compiled(C);

impl AsValue for Compiled {
    fn as_value(&self) -> Option<&Value> {
        match &self.0 {
            C::Constant(value) => Some(value),
            _ => None,
        }
    }
    fn to_compiled(&self) -> Compiled {
        self.clone()
    }
}

/// Extracts a single argument in a specific type.
fn arg<'a, T, E>(name: Function, args: &'a [E], index: usize, default: Option<T>) -> Result<T, ErrorKind>
where
    T: TryFromValue<'a>,
    E: AsValue,
{
    if let Some(arg) = args.get(index) {
        arg.as_value()
            .and_then(T::try_from_value)
            .ok_or(ErrorKind::InvalidArgumentType {
                name,
                index,
                expected: T::NAME,
            })
    } else {
        #[cfg_attr(feature = "cargo-clippy", allow(clippy::or_fun_call))] // false positive, this is cheap
        default.ok_or(ErrorKind::NotEnoughArguments(name))
    }
}

/// Converts a slice of arguments all into a specific type.
fn iter_args<'a, T, E>(name: Function, args: &'a [E]) -> impl Iterator<Item = Result<T, Error>> + 'a
where
    T: TryFromValue<'a>,
    E: AsValue,
{
    args.iter().enumerate().map(move |(index, arg)| {
        arg.as_value().and_then(T::try_from_value).ok_or_else(|| {
            ErrorKind::InvalidArgumentType {
                name,
                index,
                expected: T::NAME,
            }
            .into()
        })
    })
}

impl Compiled {
    /// Compiles an expression.
    pub fn compile(expr: Expr) -> Result<Self, Error> {
        Ok(Compiled(match expr {
            Expr::RowNum => C::RowNum,
            Expr::Value(v) => C::Constant(v),
            Expr::GetVariable(index) => C::GetVariable(index),
            Expr::SetVariable(index, e) => C::SetVariable(index, Box::new(Self::compile(*e)?)),
            Expr::Function { name, args } => {
                let args = args.into_iter().map(Self::compile).collect::<Result<Vec<_>, _>>()?;
                match compile_function(name, &args) {
                    Ok(c) => c.0,
                    Err(e) => match e.kind() {
                        ErrorKind::InvalidArgumentType { .. } => C::RawFunction { name, args },
                        _ => return Err(e),
                    },
                }
            }
            Expr::CaseValueWhen {
                value,
                conditions,
                otherwise,
            } => {
                let value = Box::new(Self::compile(*value)?);
                let conditions = conditions
                    .into_iter()
                    .map(|(p, r)| Ok((Self::compile(p)?, Self::compile(r)?)))
                    .collect::<Result<_, Error>>()?;
                let otherwise = Box::new(if let Some(o) = otherwise {
                    Self::compile(*o)?
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

    /// Evaluates a compiled expression and updates the state. Returns the evaluated value.
    pub fn eval(&self, state: &mut State) -> Result<Value, Error> {
        Ok(match &self.0 {
            C::RowNum => state.row_num.into(),
            C::Constant(v) => v.clone(),
            C::RawFunction { name, args } => {
                let args = args.iter().map(|c| c.eval(state)).collect::<Result<Vec<_>, _>>()?;
                let compiled = compile_function(*name, &args)?;
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
        })
    }
}

/// Types which can be treated like a [`Value`].
pub trait AsValue {
    /// Borrows a [`Value`] out of this instance. Returns `None` if this instance does not contain
    /// any `Value`s.
    fn as_value(&self) -> Option<&Value>;

    /// Converts this instance into an owned compiled expression.
    fn to_compiled(&self) -> Compiled;
}

impl AsValue for Value {
    fn as_value(&self) -> Option<&Value> {
        Some(self)
    }
    fn to_compiled(&self) -> Compiled {
        Compiled(C::Constant(self.clone()))
    }
}

/// Compiles a function with some value-like objects as input.
pub fn compile_function(name: Function, args: &[impl AsValue]) -> Result<Compiled, Error> {
    macro_rules! require {
        (@false, $($fmt:tt)+) => {
            return Err(ErrorKind::InvalidArguments { name, cause: format!($($fmt)+) }.into());
        };
        ($e:expr, $($fmt:tt)+) => {
            #[cfg_attr(feature = "cargo-clippy", allow(clippy::neg_cmp_op_on_partial_ord))] {
                if !$e {
                    require!(@false, $($fmt)+);
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
            let lower = arg::<Number, _>(name, args, 0, None)?;
            let upper = arg::<Number, _>(name, args, 1, None)?;
            require!(lower < upper, "{} < {}", lower, upper);
            if let (Some(a), Some(b)) = (lower.to::<u64>(), upper.to::<u64>()) {
                Ok(Compiled(C::RandUniformU64(Uniform::new(a, b))))
            } else if let (Some(a), Some(b)) = (lower.to::<i64>(), upper.to::<i64>()) {
                Ok(Compiled(C::RandUniformI64(Uniform::new(a, b))))
            } else {
                Err(ErrorKind::IntegerOverflow(format!("rand.range({}, {})", lower, upper)).into())
            }
        }

        Function::RandRangeInclusive => {
            let lower = arg::<Number, _>(name, args, 0, None)?;
            let upper = arg::<Number, _>(name, args, 1, None)?;
            require!(lower <= upper, "{} <= {}", lower, upper);
            if let (Some(a), Some(b)) = (lower.to::<u64>(), upper.to::<u64>()) {
                Ok(Compiled(C::RandUniformU64(Uniform::new_inclusive(a, b))))
            } else if let (Some(a), Some(b)) = (lower.to::<i64>(), upper.to::<i64>()) {
                Ok(Compiled(C::RandUniformI64(Uniform::new_inclusive(a, b))))
            } else {
                Err(ErrorKind::IntegerOverflow(format!("rand.range_inclusive({}, {})", lower, upper)).into())
            }
        }

        Function::RandUniform => {
            let lower = arg(name, args, 0, None)?;
            let upper = arg(name, args, 1, None)?;
            require!(lower < upper, "{} < {}", lower, upper);
            Ok(Compiled(C::RandUniformF64(Uniform::new(lower, upper))))
        }

        Function::RandUniformInclusive => {
            let lower = arg(name, args, 0, None)?;
            let upper = arg(name, args, 1, None)?;
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
            let std_dev = arg::<f64, _>(name, args, 1, None)?.abs();
            Ok(Compiled(C::RandLogNormal(distributions::LogNormal::new(mean, std_dev))))
        }

        Function::RandBool => {
            let p = arg(name, args, 0, None)?;
            require!(0.0 <= p && p <= 1.0, "{} between 0 and 1", p);
            Ok(Compiled(C::RandBool(distributions::Bernoulli::new(p))))
        }

        Function::Neg => {
            let inner = arg::<Number, _>(name, args, 0, None)?;
            Ok(Compiled(C::Constant((-inner).into())))
        }

        Function::Eq | Function::Ne | Function::Lt | Function::Le | Function::Gt | Function::Ge => {
            let lhs = arg::<&Value, _>(name, args, 0, None)?;
            let rhs = arg::<&Value, _>(name, args, 1, None)?;
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
            let lhs = arg::<&Value, _>(name, args, 0, None)?;
            let rhs = arg::<&Value, _>(name, args, 1, None)?;
            let is_eq = lhs == rhs;
            let should_eq = name == Function::Is;
            Ok(Compiled(C::Constant((is_eq == should_eq).into())))
        }

        Function::Not => {
            let inner = arg::<Option<bool>, _>(name, args, 0, None)?;
            Ok(Compiled(C::Constant(inner.map(|b| !b).into())))
        }

        Function::And | Function::Or => {
            let identity_value = name == Function::And;
            let mut result = Some(identity_value);

            for arg in iter_args(name, args) {
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
            let result = Value::try_sql_concat(iter_args::<&Value, _>(name, args).map(|item| item.map(Value::clone)))?;
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
                iter_args::<&Value, _>(name, args).try_fold(None::<Cow<'_, Value>>, |accum, cur| -> Result<_, Error> {
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
            let timestamp = NaiveDateTime::parse_from_str(input, TIMESTAMP_FORMAT)
                .with_context(|_| ErrorKind::InvalidTimestampString(input.to_owned()))?;
            Ok(Compiled(C::Constant(Value::Timestamp(timestamp))))
        }

        Function::Greatest | Function::Least => {
            let mut res = &Value::Null;
            for value in iter_args::<&Value, _>(name, args) {
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
            _ => return Err(ErrorKind::UnknownRegexFlag(flag).into()),
        };
    }

    let hir = parser
        .build()
        .parse(regex)
        .with_context(|_| ErrorKind::InvalidRegex(regex.to_owned()))?;
    let gen =
        rand_regex::Regex::with_hir(hir, max_repeat).with_context(|_| ErrorKind::InvalidRegex(regex.to_owned()))?;
    Ok(gen)
}
