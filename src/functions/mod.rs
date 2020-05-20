//! Defines functions for evaluation.

use crate::{
    error::Error,
    eval::{CompileContext, C},
    span::{ResultExt, Span, SpanExt, S},
    value::Value,
};

use std::{convert::TryFrom, fmt::Debug};

pub mod array;
pub mod ops;
pub mod rand;
pub mod string;
pub mod time;
pub mod debug;

/// Container of the arguments passed to functions.
pub type Arguments = smallvec::SmallVec<[S<Value>; 2]>;

/// An SQL function.
pub trait Function: Sync + Debug {
    /// Compiles or evaluates this function taking the provided arguments.
    fn compile(&self, ctx: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>>;
}

trait TryFromSpannedValue: Sized {
    fn try_from_spanned_value(value: S<Value>) -> Result<Self, S<Error>>;
}

impl<T> TryFromSpannedValue for T
where
    T: TryFrom<Value>,
    Error: From<T::Error>,
{
    fn try_from_spanned_value(value: S<Value>) -> Result<Self, S<Error>> {
        let span = value.span;
        Self::try_from(value.inner).span_err(span)
    }
}

impl TryFromSpannedValue for S<String> {
    fn try_from_spanned_value(value: S<Value>) -> Result<Self, S<Error>> {
        String::try_from(value.inner).span_ok_err(value.span)
    }
}

impl TryFromSpannedValue for S<Value> {
    fn try_from_spanned_value(value: S<Value>) -> Result<Self, S<Error>> {
        Ok(value)
    }
}

macro_rules! declare_arg_fn {
    (
        $(#[$meta:meta])*
        fn $name:ident($($def:ident: $ty:ident),+);
    ) => {
        $(#[$meta])*
        fn $name<$($ty),+>(span: Span, args: Arguments, $($def: Option<$ty>),+) -> Result<($($ty),+), S<Error>>
        where
            $($ty: TryFromSpannedValue,)+
        {
            let mut it = args.into_iter();
            $(
                let $def = if let Some(arg) = it.next() {
                    $ty::try_from_spanned_value(arg)
                } else {
                    $def.ok_or(Error::NotEnoughArguments.span(span))
                }?;
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
fn iter_args<T>(args: Arguments) -> impl Iterator<Item = Result<T, S<Error>>>
where
    T: TryFromSpannedValue,
{
    args.into_iter().map(T::try_from_spanned_value)
}

fn require(span: Span, cond: bool, cause: impl FnOnce() -> String) -> Result<(), S<Error>> {
    if cond {
        Ok(())
    } else {
        Err(Error::InvalidArguments(cause()).span(span))
    }
}
