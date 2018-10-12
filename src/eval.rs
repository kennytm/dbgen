use chrono::NaiveDateTime;
use crate::{
    error::{Error, ErrorKind},
    parser::{Expr, Function},
    regex,
    value::{Number, TryFromValue, Value, TIMESTAMP_FORMAT},
};
use failure::ResultExt;
use rand::{
    distributions::{self, Uniform},
    Rng, RngCore, SeedableRng, StdRng,
};
use std::{borrow::Cow, cmp::Ordering};
use zipf::ZipfDistribution;

pub type Seed = <StdRng as SeedableRng>::Seed;

/// The external state used during evaluation.
pub struct State {
    pub(crate) row_num: u64,
    rng: Box<dyn RngCore>,
    variables: Vec<Value>,
}

impl State {
    /// Creates a new state from the random number generator and starting row number.
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))] // false positive
    pub fn new(row_num: u64, rng: Box<dyn RngCore>, variables_count: usize) -> Self {
        Self {
            row_num,
            rng,
            variables: vec![Value::Null; variables_count],
        }
    }
}

#[derive(Clone, Debug)]
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
    CaseValueWhen {
        value: Box<Compiled>,
        conditions: Vec<(Compiled, Compiled)>,
        otherwise: Box<Compiled>,
    },

    RandRegex(regex::Generator),
    RandUniformU64(Uniform<u64>),
    RandUniformI64(Uniform<i64>),
    RandUniformF64(Uniform<f64>),
    RandZipf(ZipfDistribution),
    RandLogNormal(distributions::LogNormal),
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

            C::RandRegex(generator) => generator.eval(&mut state.rng),
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
