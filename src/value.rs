//! Values

use chrono::{Datelike, Duration, NaiveDateTime, Timelike};
use num_traits::FromPrimitive;
use std::{
    cmp::Ordering,
    fmt,
    io::{self, Write},
    ops, slice,
    str::{from_utf8, from_utf8_unchecked},
};

use crate::{
    error::{Error, ErrorKind},
    parser::Function,
};

/// The string format of an SQL timestamp.
pub const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";

/// Implementation of a number.
#[derive(Copy, Clone, Debug)]
enum N {
    Int(i128),
    Float(f64),
}

/// An SQL number (could represent an integer or floating point number).
#[derive(Copy, Clone, Debug)]
pub struct Number(N);

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            N::Int(v) => v.fmt(f),
            N::Float(v) => {
                let mut output = ryu::Buffer::new();
                f.write_str(output.format(v))
            }
        }
    }
}

impl Number {
    /// Tries to convert this number into a primitive.
    pub fn to<P: FromPrimitive>(&self) -> Option<P> {
        match self.0 {
            N::Int(v) => P::from_i128(v),
            N::Float(v) => P::from_f64(v),
        }
    }

    /// Converts this number into a nullable boolean using SQL rule.
    pub fn to_sql_bool(&self) -> Option<bool> {
        match self.0 {
            N::Int(v) => Some(v != 0),
            N::Float(v) if v.is_nan() => None,
            N::Float(v) => Some(v != 0.0),
        }
    }
}

macro_rules! impl_from_int_for_number {
    ($($ty:ty),*) => {
        $(impl From<$ty> for Number {
            fn from(value: $ty) -> Self {
                Number(N::Int(value.into()))
            }
        })*
    }
}
impl_from_int_for_number!(u8, u16, u32, u64, i8, i16, i32, i64, bool);

impl From<f32> for Number {
    fn from(value: f32) -> Self {
        Number(N::Float(value.into()))
    }
}
impl From<f64> for Number {
    fn from(value: f64) -> Self {
        Number(N::Float(value))
    }
}
impl From<N> for f64 {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_precision_loss))]
    fn from(n: N) -> Self {
        match n {
            N::Int(i) => i as Self,
            N::Float(f) => f,
        }
    }
}

impl ops::Neg for Number {
    type Output = Self;
    fn neg(self) -> Self {
        Number(match self.0 {
            N::Int(i) => N::Int(i.wrapping_neg()),
            N::Float(f) => N::Float(-f),
        })
    }
}

macro_rules! impl_number_bin_op {
    ($trait:ident, $fname:ident, $checked:ident) => {
        impl ops::$trait for Number {
            type Output = Self;
            fn $fname(self, other: Self) -> Self {
                if let (N::Int(a), N::Int(b)) = (self.0, other.0) {
                    if let Some(c) = a.$checked(b) {
                        return Number(N::Int(c));
                    }
                }
                Number(N::Float(f64::from(self.0).$fname(f64::from(other.0))))
            }
        }
    };
}

impl_number_bin_op!(Add, add, checked_add);
impl_number_bin_op!(Sub, sub, checked_sub);
impl_number_bin_op!(Mul, mul, checked_mul);

impl ops::Div for Number {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        Number(N::Float(f64::from(self.0) / f64::from(other.0)))
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (N::Int(a), N::Int(b)) => a == b,
            (a, b) => f64::from(a) == f64::from(b),
        }
    }
}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.0, other.0) {
            (N::Int(a), N::Int(b)) => a.partial_cmp(&b),
            (a, b) => f64::from(a).partial_cmp(&f64::from(b)),
        }
    }
}

/// An SQL string (UTF-8 or byte-string).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Bytes {
    /// The raw bytes.
    bytes: Vec<u8>,
    /// Whether the bytes contained non-UTF-8 content.
    is_binary: bool,
}

impl Bytes {
    /// Writes the content using SQL format.
    pub fn write_sql(&self, mut output: impl Write) -> Result<(), io::Error> {
        if self.is_binary {
            output.write_all(b"X'")?;
            for b in &self.bytes {
                write!(output, "{:02X}", b)?;
            }
        } else {
            output.write_all(b"'")?;
            for b in &self.bytes {
                output.write_all(if *b == b'\'' { b"''" } else { slice::from_ref(b) })?;
            }
        }
        output.write_all(b"'")?;
        Ok(())
    }
}

/// A scalar value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Null.
    Null,
    /// A number.
    Number(Number),
    /// A string or byte string.
    Bytes(Bytes),
    /// A timestamp without timezone.
    Timestamp(NaiveDateTime),
    /// A time interval, as multiple of microseconds.
    Interval(i64),

    #[doc(hidden)]
    __NonExhaustive,
}

macro_rules! try_or_overflow {
    ($e:expr, $($fmt:tt)+) => {
        if let Some(e) = $e {
            e
        } else {
            return Err(ErrorKind::IntegerOverflow(format!($($fmt)+)).into());
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = Vec::new();
        self.write_sql(&mut output).map_err(|_| fmt::Error)?;
        let s = String::from_utf8(output).map_err(|_| fmt::Error)?;
        f.write_str(&s)
    }
}

impl Value {
    /// Writes the SQL representation of this value into a write stream.
    pub fn write_sql(&self, mut output: impl Write) -> Result<(), io::Error> {
        match self {
            Value::Null => {
                output.write_all(b"NULL")?;
            }
            Value::Number(number) => {
                write!(output, "{}", number)?;
            }
            Value::Bytes(bytes) => {
                bytes.write_sql(output)?;
            }
            Value::Timestamp(timestamp) => {
                // write!(output, "'{}'", timestamp.format(TIMESTAMP_FORMAT))?;
                write!(
                    output,
                    "'{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    timestamp.year(),
                    timestamp.month(),
                    timestamp.day(),
                    timestamp.hour(),
                    timestamp.minute(),
                    timestamp.second(),
                )?;
                let ns = timestamp.nanosecond();
                if ns != 0 {
                    write!(output, ".{:06}", ns / 1000)?;
                }
                output.write_all(b"'")?;
            }
            Value::Interval(interval) => {
                write!(output, "INTERVAL {} MICROSECOND", interval)?;
            }
            Value::__NonExhaustive => {}
        }
        Ok(())
    }

    /// Compares two values using the rules common among SQL implementations.
    ///
    /// * Comparing with NULL always return `None`.
    /// * Numbers, timestamps and intervals are ordered by value.
    /// * Strings are ordered by UTF-8 binary collation.
    /// * Comparing between different types are inconsistent among database
    ///     engines, thus this function will just error with `InvalidArguments`.
    pub fn sql_cmp(&self, other: &Self, name: Function) -> Result<Option<Ordering>, Error> {
        Ok(match (self, other) {
            (Value::Null, _) | (_, Value::Null) => None,
            (Value::Number(a), Value::Number(b)) => a.partial_cmp(b),
            (Value::Bytes(a), Value::Bytes(b)) => a.bytes.partial_cmp(&b.bytes),
            (Value::Timestamp(a), Value::Timestamp(b)) => a.partial_cmp(b),
            (Value::Interval(a), Value::Interval(b)) => a.partial_cmp(b),
            _ => {
                return Err(ErrorKind::InvalidArguments {
                    name,
                    cause: format!("cannot compare {} with {}", self, other),
                }
                .into())
            }
        })
    }

    /// Adds two values using the rules common among SQL implementations.
    pub fn sql_add(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Value::Number(lhs), Value::Number(rhs)) => (*lhs + *rhs).into(),
            (Value::Timestamp(ts), Value::Interval(dur)) | (Value::Interval(dur), Value::Timestamp(ts)) => {
                Value::Timestamp(try_or_overflow!(
                    ts.checked_add_signed(Duration::microseconds(*dur)),
                    "{} + {}us",
                    ts,
                    dur
                ))
            }
            (Value::Interval(a), Value::Interval(b)) => {
                Value::Interval(try_or_overflow!(a.checked_add(*b), "{} + {}", a, b))
            }
            _ => {
                return Err(ErrorKind::InvalidArguments {
                    name: Function::Add,
                    cause: format!("cannot add {} to {}", self, other),
                }
                .into())
            }
        })
    }

    /// Subtracts two values using the rules common among SQL implementations.
    pub fn sql_sub(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Value::Number(lhs), Value::Number(rhs)) => (*lhs - *rhs).into(),
            (Value::Timestamp(ts), Value::Interval(dur)) => Value::Timestamp(try_or_overflow!(
                ts.checked_sub_signed(Duration::microseconds(*dur)),
                "{} - {}us",
                ts,
                dur
            )),
            (Value::Interval(a), Value::Interval(b)) => {
                Value::Interval(try_or_overflow!(a.checked_sub(*b), "{} + {}", a, b))
            }
            _ => {
                return Err(ErrorKind::InvalidArguments {
                    name: Function::Sub,
                    cause: format!("cannot subtract {} from {}", self, other),
                }
                .into())
            }
        })
    }

    /// Multiplies two values using the rules common among SQL implementations.
    pub fn sql_mul(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Value::Number(lhs), Value::Number(rhs)) => (*lhs * *rhs).into(),
            (Value::Number(m), Value::Interval(dur)) | (Value::Interval(dur), Value::Number(m)) => {
                let mult_res = *m * Number::from(*dur);
                Value::Interval(try_or_overflow!(mult_res.to::<i64>(), "{} microseconds", mult_res))
            }
            _ => {
                return Err(ErrorKind::InvalidArguments {
                    name: Function::Mul,
                    cause: format!("cannot multiply {} with {}", self, other),
                }
                .into())
            }
        })
    }

    /// Divides two values using the rules common among SQL implementations.
    pub fn sql_float_div(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Value::Number(_), Value::Number(rhs)) | (Value::Interval(_), Value::Number(rhs))
                if rhs.to_sql_bool() == Some(false) =>
            {
                Value::Null
            }
            (Value::Number(lhs), Value::Number(rhs)) => (*lhs / *rhs).into(),
            (Value::Interval(dur), Value::Number(d)) => {
                let mult_res = Number::from(*dur) / *d;
                Value::Interval(try_or_overflow!(mult_res.to::<i64>(), "{} microseconds", mult_res))
            }
            _ => {
                return Err(ErrorKind::InvalidArguments {
                    name: Function::FloatDiv,
                    cause: format!("cannot divide {} by {}", self, other),
                }
                .into())
            }
        })
    }

    /// Concatenates multiple values into a string.
    pub fn try_sql_concat(values: impl Iterator<Item = Result<Self, Error>>) -> Result<Self, Error> {
        let mut res = Bytes::default();
        let mut should_check_binary = false;
        for item in values {
            match item? {
                Value::Null => {
                    return Ok(Value::Null);
                }
                Value::Number(n) => {
                    write!(&mut res.bytes, "{}", n);
                }
                Value::Bytes(mut b) => {
                    res.bytes.append(&mut b.bytes);
                    if b.is_binary {
                        if res.is_binary {
                            should_check_binary = true;
                        } else {
                            res.is_binary = true;
                        }
                    }
                }
                Value::Timestamp(timestamp) => {
                    write!(&mut res.bytes, "{}", timestamp.format(TIMESTAMP_FORMAT));
                }
                Value::Interval(interval) => {
                    write!(&mut res.bytes, "INTERVAL {} MICROSECOND", interval);
                }
                Value::__NonExhaustive => {}
            }
        }

        if should_check_binary {
            res.is_binary = from_utf8(&res.bytes).is_err();
        }
        Ok(Value::Bytes(res))
    }
}

/// Types which can be extracted out of a [`Value`].
pub trait TryFromValue<'s>: Sized {
    /// The name of the type, used when an error happens.
    const NAME: &'static str;
    /// Converts a [`Value`] into the required type.
    fn try_from_value(value: &'s Value) -> Option<Self>;
}

macro_rules! impl_try_from_value {
    ($T:ty, $name:expr) => {
        impl<'s> TryFromValue<'s> for $T {
            const NAME: &'static str = $name;

            fn try_from_value(value: &'s Value) -> Option<Self> {
                Number::try_from_value(value)?.to::<$T>()
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
impl_try_from_value!(isize, "signed integer");
impl_try_from_value!(f32, "floating point number");
impl_try_from_value!(f64, "floating point number");

impl<'s> TryFromValue<'s> for Number {
    const NAME: &'static str = "number";

    fn try_from_value(value: &'s Value) -> Option<Self> {
        match value {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }
}

impl<'s> TryFromValue<'s> for &'s str {
    const NAME: &'static str = "string";

    fn try_from_value(value: &'s Value) -> Option<Self> {
        match value {
            Value::Bytes(Bytes {
                is_binary: false,
                bytes,
            }) => Some(unsafe { from_utf8_unchecked(bytes) }),
            _ => None,
        }
    }
}

impl<'s> TryFromValue<'s> for &'s Value {
    const NAME: &'static str = "value";

    fn try_from_value(value: &'s Value) -> Option<Self> {
        Some(value)
    }
}

#[cfg_attr(feature = "cargo-clippy", allow(clippy::use_self))] // rust-lang-nursery/rust-clippy#1993
impl<'s> TryFromValue<'s> for Option<bool> {
    const NAME: &'static str = "nullable boolean";

    fn try_from_value(value: &'s Value) -> Option<Self> {
        match value {
            Value::Null => Some(None),
            Value::Number(n) => Some(n.to_sql_bool()),
            _ => None,
        }
    }
}

impl<T: Into<Number>> From<T> for Value {
    fn from(value: T) -> Self {
        Value::Number(value.into())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Bytes(Bytes {
            is_binary: false,
            bytes: value.into_bytes(),
        })
    }
}

impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Value::Bytes(Bytes {
            is_binary: from_utf8(&bytes).is_err(),
            bytes,
        })
    }
}

impl From<Bytes> for Value {
    fn from(b: Bytes) -> Self {
        Value::Bytes(b)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Value::Null, T::into)
    }
}
