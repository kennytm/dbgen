//! Array functions.

use super::{args_2, Function};
use crate::{
    error::Error,
    eval::{CompileContext, Compiled, C},
    value::Value,
};
use std::sync::Arc;

/// The array constructor.
#[derive(Debug)]
pub struct Array;

impl Function for Array {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        Ok(Compiled(C::Constant(Value::Array(args.into()))))
    }
}

/// The array subscript operator.
#[derive(Debug)]
pub struct Subscript;

impl Function for Subscript {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let (base, index) = args_2::<Arc<[Value]>, usize>("[]", args, None, None)?;
        Ok(Compiled(C::Constant(if index == 0 || index > base.len() {
            Value::Null
        } else {
            base[index - 1].clone()
        })))
    }
}
