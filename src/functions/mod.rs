//! Defines functions for evaluation.

use crate::error::Error;
use crate::eval::{CompileContext, Compiled};
use crate::value::Value;

use std::{convert::TryFrom, fmt::Debug};

pub mod ops;
pub mod rand;
pub mod string;
pub mod time;

/// An SQL function.
pub trait Function: Sync + Debug {
    /// Compiles or evaluates this function taking the provided arguments.
    fn compile(&self, ctx: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error>;
}

macro_rules! declare_arg_fn {
    (
        $(#[$meta:meta])*
        fn $name:ident($($def:ident: $ty:ident),+);
    ) => {
        $(#[$meta])*
        fn $name<$($ty),+>(name: &'static str, args: Vec<Value>, $($def: Option<$ty>),+) -> Result<($($ty),+), Error>
        where
            $($ty: TryFrom<Value>,
            $ty::Error: ToString,)+
        {
            let mut it = args.into_iter();
            let mut index = 0;
            $(
                let $def = if let Some(arg) = it.next() {
                    $ty::try_from(arg).map_err(|e| Error::InvalidArgumentType {
                        name,
                        index,
                        expected: e.to_string(),
                    })
                } else {
                    $def.ok_or(Error::NotEnoughArguments(name))
                }?;
                #[allow(unused_assignments)]
                {index += 1;}
            )+
            Ok(($($def),+))
        }
    }
}

declare_arg_fn! {
    /// Extracts one value from the list of arguments.
    #[allow(unused_parens)] // we do want args_1 to return the value instead of 1-tuple.
    fn args_1(d1: T1);
}
declare_arg_fn! {
    /// Extracts two values from the list of arguments.
    fn args_2(d1: T1, d2: T2);
}
declare_arg_fn! {
    /// Extracts three values from the list of arguments.
    fn args_3(d1: T1, d2: T2, d3: T3);
}
declare_arg_fn! {
    /// Extracts four values from the list of arguments.
    fn args_4(d1: T1, d2: T2, d3: T3, d4: T4);
}

/// Converts a slice of arguments all into a specific type.
fn iter_args<T>(name: &'static str, args: Vec<Value>) -> impl Iterator<Item = Result<T, Error>>
where
    T: TryFrom<Value>,
    T::Error: ToString,
{
    args.into_iter().enumerate().map(move |(index, arg)| {
        T::try_from(arg).map_err(|e| Error::InvalidArgumentType {
            name,
            index,
            expected: e.to_string(),
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
