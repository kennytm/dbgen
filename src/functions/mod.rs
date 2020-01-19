//! Defines functions for evaluation.

use crate::error::Error;
use crate::eval::{CompileContext, Compiled};
use crate::value::{TryFromValue, Value};

use std::fmt::Debug;

pub mod ops;
pub mod rand;
pub mod string;
pub mod time;

/// An SQL function.
pub trait Function: Sync + Debug {
    /// Compiles or evaluates this function taking the provided arguments.
    fn compile(&self, ctx: &CompileContext, args: &[Value]) -> Result<Compiled, Error>;
}

/// Extracts a single argument in a specific type.
fn arg<'a, T>(name: &'static str, args: &'a [Value], index: usize, default: Option<T>) -> Result<T, Error>
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
fn iter_args<'a, T>(name: &'static str, args: &'a [Value]) -> impl Iterator<Item = Result<T, Error>> + 'a
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

fn require(name: &'static str, cond: bool, cause: impl FnOnce() -> String) -> Result<(), Error> {
    if cond {
        Ok(())
    } else {
        Err(Error::InvalidArguments { name, cause: cause() })
    }
}
