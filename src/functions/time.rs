//! Time functions.

use super::{args_1, Arguments, Function};
use crate::{
    error::Error,
    eval::{CompileContext, C},
    span::{ResultExt, Span, S},
    value::{Value, TIMESTAMP_FORMAT},
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
