//! Array functions.

use super::{args_1, args_2, args_3, Arguments, Function};
use crate::{
    array::{Array, Permutation},
    error::Error,
    eval::{CompileContext, C},
    span::{ResultExt as _, Span, SpanExt as _, S},
    value::Value,
};
use std::{cmp::Ordering, sync::Arc};

/// The array constructor.
#[derive(Debug)]
pub struct ArrayConstructor;

impl Function for ArrayConstructor {
    fn compile(&self, _: &CompileContext, _: Span, args: Arguments) -> Result<C, S<Error>> {
        Ok(C::Constant(Value::Array(Array::from_values(
            args.into_iter().map(|arg| arg.inner),
        ))))
    }
}

/// The array subscript operator.
#[derive(Debug)]
pub struct Subscript;

impl Function for Subscript {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let (base, index) = args_2::<Array, u64>(span, args, None, None)?;
        Ok(C::Constant(if index == 0 || index > base.len() {
            Value::Null
        } else {
            base.get(index - 1)
        }))
    }
}

/// The `generate_series` SQL function.
#[derive(Debug)]
pub struct GenerateSeries;

impl Function for GenerateSeries {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let (start, end, step) = args_3::<Value, Value, Value>(span, args, None, None, Some(Value::Number(1.into())))?;
        let len_number = (|| end.sql_sub(&start)?.sql_add(&step)?.sql_div(&step))().span_err(span)?;

        let len = if len_number.sql_sign() == Ordering::Greater {
            len_number
                .try_into()
                .map_err(|_| Error::InvalidArguments("generated series will be too long".to_owned()).span(span))?
        } else {
            0
        };

        Ok(C::Constant(Value::Array(Array::new_series(start, step, len))))
    }
}

/// The `rand.shuffle` SQL function.
#[derive(Debug)]
pub struct Shuffle;

impl Function for Shuffle {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let array = args_1::<Array>(span, args, None)?;
        Ok(C::RandShuffle {
            permutation: Box::new(Permutation::prepare(array.len())),
            inner: Arc::new(array),
        })
    }
}
