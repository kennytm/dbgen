//! Time functions.

use super::{arg, Function};
use crate::{
    error::Error,
    eval::{CompileContext, Compiled, C},
    value::{Value, TIMESTAMP_FORMAT},
};

use chrono::TimeZone;
use chrono_tz::Tz;

/// The `timestamp` SQL function
#[derive(Debug)]
pub struct Timestamp;

impl Function for Timestamp {
    fn compile(&self, ctx: &CompileContext, args: &[Value]) -> Result<Compiled, Error> {
        let input = arg("timestamp", args, 0, None)?;
        let tz = ctx.time_zone;
        let timestamp = tz
            .datetime_from_str(input, TIMESTAMP_FORMAT)
            .map_err(|source| Error::InvalidTimestampString {
                timestamp: input.to_owned(),
                source,
            })?
            .naive_utc();
        Ok(Compiled(C::Constant(Value::Timestamp(timestamp, tz))))
    }
}

/// The `timestamp with time zone` SQL function
#[derive(Debug)]
pub struct TimestampWithTimeZone;

impl Function for TimestampWithTimeZone {
    fn compile(&self, ctx: &CompileContext, args: &[Value]) -> Result<Compiled, Error> {
        let name = "timestamp with time zone";
        let mut input = arg::<&str>(name, args, 0, None)?;
        let tz = match input.find(|c: char| c.is_ascii_alphabetic()) {
            None => ctx.time_zone,
            Some(i) => {
                let tz = input[i..]
                    .parse::<Tz>()
                    .map_err(|cause| Error::InvalidArguments { name, cause })?;
                input = input[..i].trim_end();
                tz
            }
        };
        let timestamp = tz
            .datetime_from_str(input, TIMESTAMP_FORMAT)
            .map_err(|source| Error::InvalidTimestampString {
                timestamp: input.to_owned(),
                source,
            })?
            .naive_utc();
        Ok(Compiled(C::Constant(Value::Timestamp(timestamp, tz))))
    }
}
