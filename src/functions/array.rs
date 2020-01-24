//! Array functions.

use super::{args_2, args_3, Arguments, Function};
use crate::{
    error::Error,
    eval::{CompileContext, Compiled, C},
    value::Value,
};
use std::{cmp::Ordering, sync::Arc};

/// The array constructor.
#[derive(Debug)]
pub struct Array;

impl Function for Array {
    fn compile(&self, _: &CompileContext, args: Arguments) -> Result<Compiled, Error> {
        Ok(Compiled(C::Constant(Value::Array(args.into_vec().into()))))
    }
}

/// The array subscript operator.
#[derive(Debug)]
pub struct Subscript;

impl Function for Subscript {
    fn compile(&self, _: &CompileContext, args: Arguments) -> Result<Compiled, Error> {
        let (base, index) = args_2::<Arc<[Value]>, usize>("[]", args, None, None)?;
        Ok(Compiled(C::Constant(if index == 0 || index > base.len() {
            Value::Null
        } else {
            base[index - 1].clone()
        })))
    }
}

/// The `generate_series` SQL function.
#[derive(Debug)]
pub struct GenerateSeries;

impl Function for GenerateSeries {
    fn compile(&self, _: &CompileContext, args: Arguments) -> Result<Compiled, Error> {
        let name = "generate_series";
        let (start, end, step) = args_3::<Value, Value, Value>(name, args, None, None, Some(1.into()))?;

        let step_sign = step.sql_sign();
        if step_sign == Ordering::Equal {
            return Err(Error::InvalidArguments {
                name,
                cause: format!("cannot use zero step {}", step),
            });
        }

        let mut value = start;
        let mut result = Vec::new();
        loop {
            let cur_cmp = value.sql_cmp(&end, name)?;
            if cur_cmp.is_none() || cur_cmp == Some(step_sign) {
                break;
            }
            let next = value.sql_add_named(&step, name)?;
            result.push(value);
            value = next;
        }

        Ok(Compiled(C::Constant(Value::Array(result.into()))))
    }
}
