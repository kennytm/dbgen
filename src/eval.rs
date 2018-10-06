use crate::{
    error::{Error, ErrorKind},
    parser::{Expr, Function},
    regex,
    value::{AsValue, Number, TryFromValue, Value},
};
use rand::{distributions::Uniform, prng::Hc128Rng, Rng, SeedableRng};

type DefaultRng = Hc128Rng;

/// The type of the default RNG seed
pub type RngSeed = <DefaultRng as SeedableRng>::Seed;

/// The external state used during evaluation.
pub struct State {
    pub(crate) row_num: u64,
    rng: DefaultRng,
}

impl State {
    /// Creates a new state from the seed.
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))] // false positive
    pub fn from_seed(seed: RngSeed) -> Self {
        Self {
            row_num: 1,
            rng: DefaultRng::from_seed(seed),
        }
    }
}

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

    RandRegex(regex::Generator),
    RandUniformU64(Uniform<u64>),
    RandUniformI64(Uniform<i64>),
    RandUniformF64(Uniform<f64>),
}

/// A compiled expression
pub struct Compiled(C);

impl AsValue for Compiled {
    fn as_value(&self) -> Option<&Value> {
        match &self.0 {
            C::Constant(value) => Some(value),
            _ => None,
        }
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

impl Compiled {
    pub fn compile(expr: Expr) -> Result<Self, Error> {
        Ok(Compiled(match expr {
            Expr::RowNum => C::RowNum,
            Expr::Value(v) => C::Constant(v),
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

            C::RandRegex(generator) => generator.eval(&mut state.rng).into(),
            C::RandUniformU64(uniform) => state.rng.sample(uniform).into(),
            C::RandUniformI64(uniform) => state.rng.sample(uniform).into(),
            C::RandUniformF64(uniform) => state.rng.sample(uniform).into(),
        })
    }
}

pub fn compile_function(name: Function, args: &[impl AsValue]) -> Result<Compiled, Error> {
    macro_rules! require {
        ($e:expr, $($fmt:tt)+) => {
            #[cfg_attr(feature = "cargo-clippy", allow(clippy::neg_cmp_op_on_partial_ord))] {
                if !$e {
                    return Err(ErrorKind::InvalidArguments { name, cause: format!($($fmt)+) }.into());
                }
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

        Function::Neg => {
            let inner = arg::<Number, _>(name, args, 0, None)?;
            Ok(Compiled(C::Constant((-inner).into())))
        }
    }
}
