//! String functions.

use super::{arg, iter_args, Function};
use crate::{
    error::Error,
    eval::{CompileContext, Compiled, C},
    value::Value,
};
use std::{convert::TryInto, usize};

//------------------------------------------------------------------------------

/// Extracts the arguments for the `substring` SQL functions.
fn args_substring(name: &'static str, args: &[Value], max: usize) -> Result<(usize, usize), Error> {
    let start = arg::<i64>(name, args, 1, None)? - 1;
    let length = arg::<Option<i64>>(name, args, 2, Some(None))?;
    if let Some(length) = length {
        let end = (start + length).try_into().unwrap_or(0);
        let start = start.try_into().unwrap_or(0);
        Ok((start.min(max), end.max(start).min(max)))
    } else {
        Ok((start.try_into().unwrap_or(0).min(max), max))
    }
}

/// The `substring(… using characters)` SQL function.
#[derive(Debug)]
pub struct SubstringUsingCharacters;

impl Function for SubstringUsingCharacters {
    fn compile(&self, _: &CompileContext, args: &[Value]) -> Result<Compiled, Error> {
        let name = "substring using characters";
        let input = arg::<&str>(name, args, 0, None)?;
        #[allow(clippy::replace_consts)] // FIXME: allow this lint until usize::MAX becomes an assoc const
        let (start, end) = args_substring(name, args, usize::MAX)?;
        Ok(Compiled(C::Constant(
            input.chars().take(end).skip(start).collect::<String>().into(),
        )))
    }
}

/// The `substring(… using octets)` SQL function.
#[derive(Debug)]
pub struct SubstringUsingOctets;

impl Function for SubstringUsingOctets {
    fn compile(&self, _: &CompileContext, args: &[Value]) -> Result<Compiled, Error> {
        let name = "substring using octets";
        let input = arg::<&[u8]>(name, args, 0, None)?;
        let (start, end) = args_substring(name, args, input.len())?;
        Ok(Compiled(C::Constant(input[start..end].to_vec().into())))
    }
}

//------------------------------------------------------------------------------

/// The string concatenation (`||`) SQL function.
#[derive(Debug)]
pub struct Concat;

impl Function for Concat {
    fn compile(&self, _: &CompileContext, args: &[Value]) -> Result<Compiled, Error> {
        let result = Value::try_sql_concat(iter_args::<&Value>("||", args).map(|item| item.map(Value::clone)))?;
        Ok(Compiled(C::Constant(result)))
    }
}
