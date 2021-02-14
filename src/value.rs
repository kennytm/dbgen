//! Values

use chrono::{Duration, NaiveDateTime, TimeZone};
use rand_regex::EncodedString;
use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
    fmt,
    sync::Arc,
};
use tzfile::ArcTz;

use crate::{
    bytes::ByteString,
    error::Error,
    number::{Number, NumberError},
};

/// The string format of an SQL timestamp.
pub const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";

/// A scalar value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Null.
    Null,
    /// A number.
    Number(Number),
    /// A string or byte string.
    Bytes(ByteString),
    /// A timestamp. The `NaiveDateTime` field must be in the UTC time zone.
    Timestamp(NaiveDateTime, ArcTz),
    /// A time interval, as multiple of microseconds.
    Interval(i64),
    /// An array of values.
    Array(Arc<[Value]>),
}

impl Default for Value {
    fn default() -> Self {
        Self::Null
    }
}

macro_rules! try_or_overflow {
    ($e:expr, $($fmt:tt)+) => {
        if let Some(e) = $e {
            e
        } else {
            return Err(Error::IntegerOverflow(format!($($fmt)+)));
        }
    }
}

macro_rules! try_from_number {
    ($e:expr, $($fmt:tt)+) => {
        match $e {
            Ok(n) => Value::Number(n),
            Err(NumberError::NaN) => Value::Null,
            Err(NumberError::Overflow) => return Err(Error::IntegerOverflow(format!($($fmt)+))),
        }
    }
}

macro_rules! try_from_number_into_interval {
    ($e:expr, $($fmt:tt)+) => {
        match $e.and_then(i64::try_from) {
            Ok(n) => Value::Interval(n),
            Err(NumberError::NaN) => Value::Null,
            Err(NumberError::Overflow) => return Err(Error::IntegerOverflow(format!($($fmt)+))),
        }
    }
}

fn try_partial_cmp_by<I, J, F>(a: I, b: J, mut f: F) -> Result<Option<Ordering>, Error>
where
    I: IntoIterator,
    J: IntoIterator<Item = I::Item>,
    F: FnMut(I::Item, I::Item) -> Result<Option<Ordering>, Error>,
{
    let mut a = a.into_iter();
    let mut b = b.into_iter();
    loop {
        match (a.next(), b.next()) {
            (Some(aa), Some(bb)) => match f(aa, bb) {
                Ok(Some(Ordering::Equal)) => {}
                res => return res,
            },
            (aa, bb) => return Ok(aa.is_some().partial_cmp(&bb.is_some())),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::format::{Format, SqlFormat};

        let format = SqlFormat {
            escape_backslash: false,
        };
        let mut writer = Vec::new();
        format.write_value(&mut writer, self).map_err(|_| fmt::Error)?;
        let s = String::from_utf8(writer).map_err(|_| fmt::Error)?;
        f.write_str(&s)
    }
}

impl Value {
    /// Creates a timestamp value.
    pub fn new_timestamp(ts: NaiveDateTime, tz: ArcTz) -> Self {
        Self::Timestamp(ts, tz)
    }

    /// Creates a finite floating point value.
    pub(crate) fn from_finite_f64(v: f64) -> Self {
        Self::Number(Number::from_finite_f64(v))
    }

    /// Compares two values using the rules common among SQL implementations.
    ///
    /// * Comparing with NULL always return `None`.
    /// * Numbers and intervals are ordered by value.
    /// * Timestamps are ordered by its UTC value, ignoring time zone.
    /// * Strings are ordered by UTF-8 binary collation.
    /// * Arrays are ordered lexicographically.
    /// * Comparing between different types are inconsistent among database
    ///     engines, thus this function will just error with `InvalidArguments`.
    pub fn sql_cmp(&self, other: &Self) -> Result<Option<Ordering>, Error> {
        Ok(match (self, other) {
            (Self::Null, _) | (_, Self::Null) => None,
            (Self::Number(a), Self::Number(b)) => a.partial_cmp(b),
            (Self::Bytes(a), Self::Bytes(b)) => a.partial_cmp(b),
            (Self::Timestamp(a, _), Self::Timestamp(b, _)) => a.partial_cmp(b),
            (Self::Interval(a), Self::Interval(b)) => a.partial_cmp(b),
            (Self::Array(a), Self::Array(b)) => try_partial_cmp_by(a.iter(), b.iter(), |a, b| a.sql_cmp(b))?,
            _ => {
                return Err(Error::InvalidArguments(format!(
                    "cannot compare {} with {}",
                    self, other
                )));
            }
        })
    }

    /// Compares this value with the zero value of its own type.
    pub fn sql_sign(&self) -> Ordering {
        match self {
            Self::Null => Ordering::Equal,
            Self::Number(a) => a.sql_sign(),
            Self::Bytes(a) => true.cmp(&a.is_empty()),
            Self::Timestamp(..) => Ordering::Greater,
            Self::Interval(a) => a.cmp(&0),
            Self::Array(a) => true.cmp(&a.is_empty()),
        }
    }

    /// Adds two values using the rules common among SQL implementations.
    pub fn sql_add(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Self::Number(lhs), Self::Number(rhs)) => try_from_number!(lhs.add(*rhs), "{} + {}", lhs, rhs),
            (Self::Timestamp(ts, tz), Self::Interval(dur)) | (Self::Interval(dur), Self::Timestamp(ts, tz)) => {
                Self::Timestamp(
                    try_or_overflow!(
                        ts.checked_add_signed(Duration::microseconds(*dur)),
                        "{} + {}us",
                        ts,
                        dur
                    ),
                    tz.clone(),
                )
            }
            (Self::Interval(a), Self::Interval(b)) => {
                Self::Interval(try_or_overflow!(a.checked_add(*b), "{} + {}", a, b))
            }
            _ => {
                return Err(Error::InvalidArguments(format!("cannot add {} to {}", self, other)));
            }
        })
    }

    /// Subtracts two values using the rules common among SQL implementations.
    pub fn sql_sub(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Self::Number(lhs), Self::Number(rhs)) => try_from_number!(lhs.sub(*rhs), "{} - {}", lhs, rhs),
            (Self::Timestamp(ts, tz), Self::Interval(dur)) => Self::Timestamp(
                try_or_overflow!(
                    ts.checked_sub_signed(Duration::microseconds(*dur)),
                    "{} - {}us",
                    ts,
                    dur
                ),
                tz.clone(),
            ),
            (Self::Interval(a), Self::Interval(b)) => {
                Self::Interval(try_or_overflow!(a.checked_sub(*b), "{} + {}", a, b))
            }
            _ => {
                return Err(Error::InvalidArguments(format!(
                    "cannot subtract {} from {}",
                    self, other
                )));
            }
        })
    }

    /// Multiplies two values using the rules common among SQL implementations.
    pub fn sql_mul(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Self::Number(lhs), Self::Number(rhs)) => try_from_number!(lhs.mul(*rhs), "{} * {}", lhs, rhs),
            (Self::Number(m), Self::Interval(dur)) | (Self::Interval(dur), Self::Number(m)) => {
                try_from_number_into_interval!(Number::from(*dur).mul(*m), "interval {} microsecond * {}", dur, m)
            }
            _ => {
                return Err(Error::InvalidArguments(format!(
                    "cannot multiply {} with {}",
                    self, other
                )));
            }
        })
    }

    /// Divides two values using the rules common among SQL implementations.
    pub fn sql_float_div(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Self::Number(lhs), Self::Number(rhs)) => try_from_number!(lhs.float_div(*rhs), "{} / {}", lhs, rhs),
            (Self::Interval(dur), Self::Number(d)) => {
                try_from_number_into_interval!(Number::from(*dur).float_div(*d), "interval {} microsecond / {}", dur, d)
            }
            _ => {
                return Err(Error::InvalidArguments(format!("cannot divide {} by {}", self, other)));
            }
        })
    }

    /// Divides two values using the rules common among SQL implementations.
    pub fn sql_div(&self, other: &Self) -> Result<Self, Error> {
        if let (Self::Number(lhs), Self::Number(rhs)) = (self, other) {
            Ok(try_from_number!(lhs.div(*rhs), "div({}, {})", lhs, rhs))
        } else {
            Err(Error::InvalidArguments(format!("cannot divide {} by {}", self, other)))
        }
    }

    /// Computes the remainder when dividing two values using the rules common among SQL implementations.
    pub fn sql_rem(&self, other: &Self) -> Result<Self, Error> {
        if let (Self::Number(lhs), Self::Number(rhs)) = (self, other) {
            Ok(try_from_number!(lhs.rem(*rhs), "mod({}, {})", lhs, rhs))
        } else {
            Err(Error::InvalidArguments(format!(
                "cannot compute remainder of {} by {}",
                self, other
            )))
        }
    }

    /// Concatenates multiple values into a string.
    pub fn sql_concat<'a>(values: impl Iterator<Item = &'a Self>) -> Result<Self, Error> {
        use std::fmt::Write;

        let mut res = ByteString::default();
        for item in values {
            match item {
                Self::Null => return Ok(Self::Null),
                Self::Number(n) => res.extend_number(n),
                Self::Bytes(b) => res.extend_byte_string(b),
                Self::Timestamp(timestamp, tz) => {
                    write!(res, "{}", tz.from_utc_datetime(&timestamp).format(TIMESTAMP_FORMAT)).unwrap()
                }
                Self::Interval(interval) => write!(res, "INTERVAL {} MICROSECOND", interval).unwrap(),
                Self::Array(_) => {
                    return Err(Error::InvalidArguments(
                        "cannot concatenate arrays using || operator".to_owned(),
                    ))
                }
            }
        }
        Ok(Self::Bytes(res))
    }

    /// Checks whether this value is truthy in SQL sense.
    ///
    /// All nonzero numbers are considered "true", and both NULL and zero are
    /// considered "false". All other types cause the `InvalidArguments` error.
    pub fn is_sql_true(&self) -> Result<bool, Error> {
        match self {
            Self::Null => Ok(false),
            Self::Number(n) => Ok(n.sql_sign() != Ordering::Equal),
            _ => Err(Error::InvalidArguments(format!("truth value of {} is undefined", self))),
        }
    }

    fn to_unexpected_value_type_error(&self, expected: &'static str) -> Error {
        Error::UnexpectedValueType {
            expected,
            value: self.to_string(),
        }
    }
}

macro_rules! impl_try_from_value {
    ($T:ty, $name:expr) => {
        impl TryFrom<Value> for $T {
            type Error = Error;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                if let Value::Number(n) = value {
                    if let Ok(v) = n.try_into() {
                        return Ok(v);
                    }
                }
                Err(value.to_unexpected_value_type_error($name))
            }
        }

        impl TryFrom<Value> for Option<$T> {
            type Error = Error;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                match value {
                    Value::Null => return Ok(None),
                    Value::Number(n) => {
                        if let Ok(v) = n.try_into() {
                            return Ok(Some(v));
                        }
                    }
                    _ => {}
                }
                Err(value.to_unexpected_value_type_error(concat!("nullable ", $name)))
            }
        }
    };
}

impl_try_from_value!(u8, "8-bit unsigned integer");
impl_try_from_value!(u16, "16-bit unsigned integer");
impl_try_from_value!(u32, "32-bit unsigned integer");
impl_try_from_value!(u64, "64-bit unsigned integer");
impl_try_from_value!(usize, "unsigned integer");
impl_try_from_value!(i8, "8-bit signed integer");
impl_try_from_value!(i16, "16-bit signed integer");
impl_try_from_value!(i32, "32-bit signed integer");
impl_try_from_value!(i64, "64-bit signed integer");
impl_try_from_value!(i128, "signed integer");
impl_try_from_value!(isize, "signed integer");
impl_try_from_value!(f64, "floating point number");

impl TryFrom<Value> for Number {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Number(n) => Ok(n),
            _ => Err(value.to_unexpected_value_type_error("number")),
        }
    }
}

impl TryFrom<Value> for ByteString {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bytes(bytes) => Ok(bytes),
            _ => Err(value.to_unexpected_value_type_error("byte string")),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = Error;

    fn try_from(mut value: Value) -> Result<Self, Self::Error> {
        if let Value::Bytes(bytes) = value {
            match bytes.try_into() {
                Ok(s) => return Ok(s),
                Err(e) => value = Value::Bytes(e.0),
            }
        }
        Err(value.to_unexpected_value_type_error("string"))
    }
}

impl TryFrom<Value> for Vec<u8> {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bytes(bytes) => Ok(bytes.into_bytes()),
            _ => Err(value.to_unexpected_value_type_error("bytes")),
        }
    }
}

impl TryFrom<Value> for Option<bool> {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Null => Ok(None),
            Value::Number(n) => Ok(Some(n.sql_sign() != Ordering::Equal)),
            _ => Err(value.to_unexpected_value_type_error("nullable boolean")),
        }
    }
}

impl TryFrom<Value> for Arc<[Value]> {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Array(v) => Ok(v),
            _ => Err(value.to_unexpected_value_type_error("array")),
        }
    }
}

impl<T: Into<Number>> From<T> for Value {
    fn from(value: T) -> Self {
        Self::Number(value.into())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Bytes(value.into())
    }
}

impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Bytes(bytes.into())
    }
}

impl From<ByteString> for Value {
    fn from(b: ByteString) -> Self {
        Self::Bytes(b)
    }
}

impl From<EncodedString> for Value {
    fn from(result: EncodedString) -> Self {
        Self::Bytes(result.into())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, T::into)
    }
}
