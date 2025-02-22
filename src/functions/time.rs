//! Time functions.

use super::{Arguments, Function, args_1};
use crate::{
    error::Error,
    eval::{C, CompileContext},
    span::{ResultExt, S, Span},
    value::{TIMESTAMP_FORMAT, Value},
};

use chrono::NaiveDateTime;

/// The `timestamp` SQL function
#[derive(Debug)]
pub struct Timestamp;

impl Function for Timestamp {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let input = args_1::<String>(span, args, None)?;
        let timestamp = NaiveDateTime::parse_from_str(&input, TIMESTAMP_FORMAT).span_err(span)?;
        Ok(C::Constant(Value::Timestamp(timestamp)))
    }
}
