use chrono::{Duration, NaiveDateTime};
use crate::{
    error::{Error, ErrorKind},
    parser::{Expr, Function},
    regex,
    value::{Number, TryFromValue, Value, TIMESTAMP_FORMAT},
};
use failure::ResultExt;
use rand::{
    distributions::{self, Uniform},
    Rng, SeedableRng, StdRng,
};
use std::cmp::Ordering;
use zipf::ZipfDistribution;

pub type Seed = <StdRng as SeedableRng>::Seed;

/// The external state used during evaluation.
pub struct State {
    pub(crate) row_num: u64,
    rng: StdRng,
    variables: Vec<Value>,
}

impl State {
    /// Creates a new state from the seed and starting row number.
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))] // false positive
    pub fn new(row_num: u64, seed: Seed, variables_count: usize) -> Self {
        Self {
            row_num,
            rng: StdRng::from_seed(seed),
            variables: vec![Value::Null; variables_count],
        }
    }
}

#[derive(Clone)]
enum C {
    /// The row number.
    RowNum,
    /// An evaluated constant.
    Constant(Value),
    /// An unevaluated function.
    RawFunction {
        name: Function,
        args: Vec<Compiled>,
    },
    GetVariable(usize),
    SetVariable(usize, Box<Compiled>),

    RandRegex(regex::Generator),
    RandUniformU64(Uniform<u64>),
    RandUniformI64(Uniform<i64>),
    RandUniformF64(Uniform<f64>),
    RandZipf(ZipfDistribution),
    RandLogNormal(distributions::LogNormal),
    RandBool(distributions::Bernoulli),
}

/// A compiled expression
#[derive(Clone)]
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
        }))
    }

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

            C::RandRegex(generator) => generator.eval(&mut state.rng).into(),
            C::RandUniformU64(uniform) => state.rng.sample(uniform).into(),
            C::RandUniformI64(uniform) => state.rng.sample(uniform).into(),
            C::RandUniformF64(uniform) => state.rng.sample(uniform).into(),
            C::RandZipf(zipf) => (state.rng.sample(zipf) as u64).into(),
            C::RandLogNormal(log_normal) => state.rng.sample(log_normal).into(),
            C::RandBool(bern) => u64::from(state.rng.sample(bern)).into(),
        })
    }
}

pub trait AsValue {
    fn as_value(&self) -> Option<&Value>;
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
    macro_rules! try_or_overflow {
        ($e:expr, $($fmt:tt)+) => {
            if let Some(e) = $e {
                e
            } else {
                return Err(ErrorKind::IntegerOverflow(format!($($fmt)+)).into());
            }
        }
    }

    match name {
        Function::RandRegex => {
            let regex = arg(name, args, 0, None)?;
            let flags = arg(name, args, 1, Some(""))?;
            let max_repeat = arg(name, args, 2, Some(100))?;
            let generator = regex::Generator::new(regex, flags, max_repeat)?;
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

        Function::Add => {
            let lhs = arg::<&Value, _>(name, args, 0, None)?;
            let rhs = arg::<&Value, _>(name, args, 1, None)?;
            Ok(Compiled(C::Constant(match (lhs, rhs) {
                (Value::Number(lhs), Value::Number(rhs)) => (*lhs + *rhs).into(),
                (Value::Timestamp(ts), Value::Interval(dur)) | (Value::Interval(dur), Value::Timestamp(ts)) => {
                    Value::Timestamp(try_or_overflow!(
                        ts.checked_add_signed(Duration::microseconds(*dur)),
                        "{} + {}us",
                        ts,
                        dur
                    ))
                }
                (Value::Interval(a), Value::Interval(b)) => {
                    Value::Interval(try_or_overflow!(a.checked_add(*b), "{} + {}", a, b))
                }
                _ => require!(@false, "unsupport argument types"),
            })))
        }
        Function::Sub => {
            let lhs = arg::<&Value, _>(name, args, 0, None)?;
            let rhs = arg::<&Value, _>(name, args, 1, None)?;
            Ok(Compiled(C::Constant(match (lhs, rhs) {
                (Value::Number(lhs), Value::Number(rhs)) => (*lhs - *rhs).into(),
                (Value::Timestamp(ts), Value::Interval(dur)) => Value::Timestamp(try_or_overflow!(
                    ts.checked_sub_signed(Duration::microseconds(*dur)),
                    "{} - {}us",
                    ts,
                    dur
                )),
                (Value::Interval(a), Value::Interval(b)) => {
                    Value::Interval(try_or_overflow!(a.checked_sub(*b), "{} - {}", a, b))
                }
                _ => require!(@false, "unsupport argument types"),
            })))
        }
        Function::Mul => {
            let lhs = arg::<&Value, _>(name, args, 0, None)?;
            let rhs = arg::<&Value, _>(name, args, 1, None)?;
            Ok(Compiled(C::Constant(match (lhs, rhs) {
                (Value::Number(lhs), Value::Number(rhs)) => (*lhs * *rhs).into(),
                (Value::Number(multiple), Value::Interval(duration))
                | (Value::Interval(duration), Value::Number(multiple)) => {
                    let mult_res = *multiple * Number::from(*duration);
                    if let Some(res) = mult_res.to::<i64>() {
                        Value::Interval(res)
                    } else {
                        return Err(ErrorKind::IntegerOverflow(format!("{} microseconds", mult_res)).into());
                    }
                }
                _ => require!(@false, "unsupport argument types"),
            })))
        }
        Function::FloatDiv => {
            let lhs = arg::<Number, _>(name, args, 0, None)?;
            let rhs = arg::<Number, _>(name, args, 1, None)?;
            Ok(Compiled(C::Constant(if rhs.to_sql_bool() == Some(true) {
                (lhs / rhs).into()
            } else {
                Value::Null
            })))
        }

        Function::Timestamp => {
            let input = arg(name, args, 0, None)?;
            let timestamp = NaiveDateTime::parse_from_str(input, TIMESTAMP_FORMAT)
                .with_context(|_| ErrorKind::InvalidTimestampString(input.to_owned()))?;
            Ok(Compiled(C::Constant(Value::Timestamp(timestamp))))
        }

        Function::CaseValueWhen => {
            let check = arg::<&Value, _>(name, args, 0, None)?;
            let args_count = args.len();

            for i in (1..args_count).step_by(2) {
                let compare = arg::<&Value, _>(name, args, i, None)?;
                if check.sql_cmp(compare, name)? == Some(Ordering::Equal) {
                    return Ok(args[i + 1].to_compiled());
                }
            }
            Ok(if args_count % 2 == 0 {
                // contains an "else" clause
                args[args_count - 1].to_compiled()
            } else {
                Compiled(C::Constant(Value::Null))
            })
        }
    }
}
