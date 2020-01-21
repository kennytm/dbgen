//! String functions.

use super::{args_3, Function};
use crate::{
    error::Error,
    eval::{CompileContext, Compiled, C},
    value::Value,
};
use std::{convert::TryInto, usize};

//------------------------------------------------------------------------------

/// Extracts the arguments for the `substring` SQL functions.
fn sql_to_range(start: i64, length: Option<i64>, max: usize) -> (usize, usize) {
    let start = start - 1;
    if let Some(length) = length {
        let end = (start + length).try_into().unwrap_or(0);
        let start = start.try_into().unwrap_or(0);
        (start.min(max), end.max(start).min(max))
    } else {
        (start.try_into().unwrap_or(0).min(max), max)
    }
}

/// The `substring(… using characters)` SQL function.
#[derive(Debug)]
pub struct SubstringUsingCharacters;

impl Function for SubstringUsingCharacters {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = "substring using characters";
        let (input, start, length) = args_3::<String, i64, Option<i64>>(name, args, None, None, Some(None))?;
        let (start, end) = sql_to_range(start, length, input.len());
        Ok(Compiled(C::Constant(
            input.chars().take(end).skip(start).collect::<String>().into(),
        )))
    }
}

/// The `substring(… using octets)` SQL function.
#[derive(Debug)]
pub struct SubstringUsingOctets;

impl Function for SubstringUsingOctets {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = "substring using octets";
        let (mut input, start, length) = args_3::<Vec<u8>, i64, Option<i64>>(name, args, None, None, Some(None))?;
        let (start, end) = sql_to_range(start, length, input.len());
        if start != 0 {
            input = input[start..end].to_vec();
        } else {
            input.truncate(end);
        }
        Ok(Compiled(C::Constant(input.into())))
    }
}

//------------------------------------------------------------------------------

/// The string concatenation (`||`) SQL function.
#[derive(Debug)]
pub struct Concat;

impl Function for Concat {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let result = Value::sql_concat(args.into_iter())?;
        Ok(Compiled(C::Constant(result)))
    }
}
