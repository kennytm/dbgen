//! Array functions.

use super::{args_2, args_3, Arguments, Function};
use crate::{
    error::Error,
    eval::{CompileContext, C},
    span::{ResultExt, Span, SpanExt, S},
    value::Value,
};
use std::{cmp::Ordering, sync::Arc};

/// The array constructor.
#[derive(Debug)]
pub struct Array;

impl Function for Array {
    fn compile(&self, _: &CompileContext, _: Span, args: Arguments) -> Result<C, S<Error>> {
        Ok(C::Constant(Value::Array(
            args.into_iter().map(|arg| arg.inner).collect(),
        )))
    }
}

/// The array subscript operator.
#[derive(Debug)]
pub struct Subscript;

impl Function for Subscript {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let (base, index) = args_2::<Arc<[Value]>, usize>(span, args, None, None)?;
        Ok(C::Constant(if index == 0 || index > base.len() {
            Value::Null
        } else {
            base[index - 1].clone()
        }))
    }
}

/// The `generate_series` SQL function.
#[derive(Debug)]
pub struct GenerateSeries;

impl Function for GenerateSeries {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let (start, end, step) =
            args_3::<S<Value>, S<Value>, S<Value>>(span, args, None, None, Some(Value::from(1).span(span)))?;

        let step_sign = step.inner.sql_sign();
        if step_sign == Ordering::Equal {
            return Err(Error::InvalidArguments(format!("cannot use zero step {}", step.inner)).span(step.span));
        }

        let mut value = start.inner;
        let mut result = Vec::new();
        loop {
            let cur_cmp = value.sql_cmp(&end.inner).span_err(end.span)?;
            if cur_cmp.is_none() || cur_cmp == Some(step_sign) {
                break;
            }
            let next = value.sql_add(&step.inner).span_err(start.span)?;
            result.push(value);
            value = next;
        }

        Ok(C::Constant(Value::Array(result.into())))
    }
}
