//! Array functions.

use super::Function;
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
