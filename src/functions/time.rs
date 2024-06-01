//! Time functions.

use super::{args_1, Arguments, Function};
use crate::{
    error::Error,
    eval::{CompileContext, C},
    span::{ResultExt, Span, SpanExt, S},
    value::{Value, TIMESTAMP_FORMAT},
};

use chrono::NaiveDateTime;

/// The `timestamp` SQL function
#[derive(Debug)]
pub struct Timestamp;

impl Function for Timestamp {
    fn compile(&self, ctx: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let input = args_1::<String>(span, args, None)?;
        let (local_ts, remainder) = NaiveDateTime::parse_and_remainder(&input, TIMESTAMP_FORMAT).span_err(span)?;
        let tz = match remainder.trim_start() {
            "" => ctx.time_zone.clone(),
            name => ctx.parse_time_zone(name).span_err(span)?,
        };
        let timestamp = local_ts
            .and_local_timezone(&*tz)
            .single()
            .ok_or_else(|| Error::InvalidOrAmbiguousLocalTime.span(span))?
            .naive_utc();
        Ok(C::Constant(Value::Timestamp(timestamp, tz)))
    }
}
