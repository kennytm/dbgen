//! Time functions.

use super::{args_1, Arguments, Function};
use crate::{
    error::Error,
    eval::{CompileContext, C},
    span::{ResultExt, Span, S},
    value::{Value, TIMESTAMP_FORMAT},
};

use chrono::TimeZone;

/// The `timestamp` SQL function
#[derive(Debug)]
pub struct Timestamp;

impl Function for Timestamp {
    fn compile(&self, ctx: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let input = args_1::<String>(span, args, None)?;
        let tz = ctx.time_zone.clone();
        let timestamp = tz
            .datetime_from_str(&input, TIMESTAMP_FORMAT)
            .span_err(span)?
            .naive_utc();
        Ok(C::Constant(Value::Timestamp(timestamp, tz)))
    }
}

/// The `timestamp with time zone` SQL function
#[derive(Debug)]
pub struct TimestampWithTimeZone;

impl Function for TimestampWithTimeZone {
    fn compile(&self, ctx: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let mut input = &*args_1::<String>(span, args, None)?;
        let tz = match input.find(|c: char| c.is_ascii_alphabetic()) {
            None => ctx.time_zone.clone(),
            Some(i) => {
                let tz = ctx.parse_time_zone(&input[i..]).span_err(span)?;
                input = input[..i].trim_end();
                tz
            }
        };
        let timestamp = tz
            .datetime_from_str(input, TIMESTAMP_FORMAT)
            .span_err(span)?
            .naive_utc();
        Ok(C::Constant(Value::Timestamp(timestamp, tz)))
    }
}
