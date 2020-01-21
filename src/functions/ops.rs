//! Numerical and logical functions.

use super::{args_1, args_2, iter_args, Function};
use crate::{
    error::Error,
    eval::{CompileContext, Compiled, C},
    value::{Number, Value},
};
use std::cmp::Ordering;

//------------------------------------------------------------------------------

/// The unary negation SQL function
#[derive(Debug)]
pub struct Neg;

impl Function for Neg {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let inner = args_1::<Number>("neg", args, None)?;
        Ok(Compiled(C::Constant((-inner).into())))
    }
}

//------------------------------------------------------------------------------

/// The value comparison (`<`, `=`, `>`, `<=`, `<>`, `>=`) SQL functions.
#[derive(Debug)]
pub struct Compare {
    /// Whether a less-than result is considered TRUE.
    pub lt: bool,
    /// Whether an equals result is considered TRUE.
    pub eq: bool,
    /// Whether a greater-than result is considered TRUE.
    pub gt: bool,
}

impl Compare {
    fn name(&self) -> &'static str {
        match (self.lt, self.eq, self.gt) {
            (true, false, false) => "<",
            (false, true, false) => "=",
            (false, false, true) => ">",
            (true, true, false) => "<=",
            (true, false, true) => "<>",
            (false, true, true) => ">=",
            _ => "?",
        }
    }
}

impl Function for Compare {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = self.name();
        if let [lhs, rhs] = &*args {
            Ok(Compiled(C::Constant(match lhs.sql_cmp(rhs, name)? {
                None => Value::Null,
                Some(Ordering::Less) => self.lt.into(),
                Some(Ordering::Equal) => self.eq.into(),
                Some(Ordering::Greater) => self.gt.into(),
            })))
        } else {
            panic!("should have exactly 2 arguments");
        }
    }
}

//------------------------------------------------------------------------------

/// The identity comparison (`IS`, `IS NOT`) SQL functions.
#[derive(Debug)]
pub struct Identical {
    /// Whether an identical result is considered TRUE.
    pub eq: bool,
}

impl Function for Identical {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        if let [lhs, rhs] = &*args {
            let is_eq = lhs == rhs;
            Ok(Compiled(C::Constant((is_eq == self.eq).into())))
        } else {
            panic!("should have exactly 2 arguments");
        }
    }
}

//------------------------------------------------------------------------------

/// The logical `NOT` SQL function.
#[derive(Debug)]
pub struct Not;

impl Function for Not {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let inner = args_1::<Option<bool>>("not", args, None)?;
        Ok(Compiled(C::Constant(inner.map(|b| !b).into())))
    }
}

//------------------------------------------------------------------------------

/// The logical `AND`/`OR` SQL functions.
#[derive(Debug)]
pub struct Logic {
    /// The identity value. True means `AND` and false means `OR`.
    pub identity: bool,
}

impl Logic {
    fn name(&self) -> &'static str {
        if self.identity {
            "and"
        } else {
            "or"
        }
    }
}

impl Function for Logic {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = self.name();
        let mut result = Some(self.identity);

        for arg in iter_args::<Option<bool>>(name, args) {
            if let Some(v) = arg? {
                if v == self.identity {
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
}

//------------------------------------------------------------------------------

/// The arithmetic (`+`, `-`, `*`, `/`) SQL functions.
#[derive(Debug)]
pub enum Arith {
    /// Addition (`+`)
    Add,
    /// Subtraction (`-`)
    Sub,
    /// Multiplication (`*`)
    Mul,
    /// Floating-point division (`/`)
    FloatDiv,
}

impl Function for Arith {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let func = match self {
            Self::Add => Value::sql_add,
            Self::Sub => Value::sql_sub,
            Self::Mul => Value::sql_mul,
            Self::FloatDiv => Value::sql_float_div,
        };

        let result = args
            .into_iter()
            .try_fold(None::<Value>, |accum, cur| -> Result<_, Error> {
                Ok(Some(if let Some(prev) = accum {
                    func(&prev, &cur)?
                } else {
                    cur
                }))
            });
        Ok(Compiled(C::Constant(result?.expect("at least 1 argument"))))
    }
}

//------------------------------------------------------------------------------

/// The extremum (`least`, `greatest`) SQL functions.
#[derive(Debug)]
pub struct Extremum {
    /// The order to drive the extremum.
    pub order: Ordering,
}

impl Extremum {
    fn name(&self) -> &'static str {
        match self.order {
            Ordering::Less => "least",
            Ordering::Greater => "greatest",
            _ => "?",
        }
    }
}

impl Function for Extremum {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = self.name();
        let mut res = Value::Null;
        for value in args {
            let should_replace = if let Some(order) = value.sql_cmp(&res, name)? {
                order == self.order
            } else {
                res == Value::Null
            };
            if should_replace {
                res = value;
            }
        }
        Ok(Compiled(C::Constant(res)))
    }
}

//------------------------------------------------------------------------------

/// The `round` SQL function.
#[derive(Debug)]
pub struct Round;

impl Function for Round {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let (value, digits) = args_2::<f64, i32>("round", args, None, Some(0))?;
        let scale = 10.0_f64.powi(digits);
        let result = (value * scale).round() / scale;
        Ok(Compiled(C::Constant(result.into())))
    }
}

//------------------------------------------------------------------------------

/// The `div` SQL function.
#[derive(Debug)]
pub struct Div;

impl Function for Div {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let (n, d) = args_2::<Number, Number>("div", args, None, None)?;
        Ok(Compiled(C::Constant(n.div(&d).into())))
    }
}

/// The `mod` SQL function.
#[derive(Debug)]
pub struct Mod;

impl Function for Mod {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let (n, d) = args_2::<Number, Number>("mod", args, None, None)?;
        Ok(Compiled(C::Constant(n.rem(&d).into())))
    }
}

//------------------------------------------------------------------------------

/// The `coalesce` SQL function.
#[derive(Debug)]
pub struct Coalesce;

impl Function for Coalesce {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let res = args.into_iter().find(|v| *v != Value::Null).unwrap_or(Value::Null);
        Ok(Compiled(C::Constant(res)))
    }
}
