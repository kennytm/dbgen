//! Values

use chrono::{Duration, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use num_traits::FromPrimitive;
use std::{cmp::Ordering, convert::TryFrom, fmt, io::Write, ops, str::from_utf8, sync::Arc};

use crate::error::Error;

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

    /// Divides this number by the other number, and truncates towards zero.
    pub fn div(&self, other: &Self) -> Option<Self> {
        Some(Self(match (self.0, other.0) {
            (N::Int(n), N::Int(d)) => N::Int(n.checked_div(d)?),
            (n, d) => {
                let d = f64::from(d);
                if d == 0.0 || d.is_nan() {
                    return None;
                }
                N::Float((f64::from(n) / d).trunc())
            }
        }))
    }

    /// Computes the remainder when this number is divided by the other number.
    pub fn rem(&self, other: &Self) -> Option<Self> {
        Some(Self(match (self.0, other.0) {
            (N::Int(n), N::Int(d)) => N::Int(n.checked_rem(d)?),
            (n, d) => {
                let d = f64::from(d);
                if d == 0.0 || d.is_nan() {
                    return None;
                }
                N::Float(f64::from(n) % d)
            }
        }))
    }
}

macro_rules! impl_from_int_for_number {
    ($($ty:ty),*) => {
        $(impl From<$ty> for Number {
            fn from(value: $ty) -> Self {
                Self(N::Int(value as _))
            }
        })*
    }
}
impl_from_int_for_number!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, bool);

impl From<f32> for Number {
    fn from(value: f32) -> Self {
        Self(N::Float(value.into()))
    }
}
impl From<f64> for Number {
    fn from(value: f64) -> Self {
        Self(N::Float(value))
    }
}
impl From<N> for f64 {
    #[allow(clippy::cast_precision_loss)]
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
        Self(match self.0 {
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
                        return Self(N::Int(c));
                    }
                }
                Self(N::Float(f64::from(self.0).$fname(f64::from(other.0))))
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
        Self(N::Float(f64::from(self.0) / f64::from(other.0)))
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
    /// Gets whether the bytes contained non-UTF-8 content.
    pub fn is_binary(&self) -> bool {
        self.is_binary
    }

    /// Gets the byte content of this string.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A scalar value.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// Null.
    Null,
    /// A number.
    Number(Number),
    /// A string or byte string.
    Bytes(Bytes),
    /// A timestamp. The `NaiveDateTime` field must be in the UTC time zone.
    Timestamp(NaiveDateTime, Tz),
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
    pub fn new_timestamp(ts: NaiveDateTime, tz: Tz) -> Self {
        Self::Timestamp(ts, tz)
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
    pub fn sql_cmp(&self, other: &Self, name: &'static str) -> Result<Option<Ordering>, Error> {
        Ok(match (self, other) {
            (Self::Null, _) | (_, Self::Null) => None,
            (Self::Number(a), Self::Number(b)) => a.partial_cmp(b),
            (Self::Bytes(a), Self::Bytes(b)) => a.bytes.partial_cmp(&b.bytes),
            (Self::Timestamp(a, _), Self::Timestamp(b, _)) => a.partial_cmp(b),
            (Self::Interval(a), Self::Interval(b)) => a.partial_cmp(b),
            (Self::Array(a), Self::Array(b)) => try_partial_cmp_by(a.iter(), b.iter(), |a, b| a.sql_cmp(b, name))?,
            _ => {
                return Err(Error::InvalidArguments {
                    name,
                    cause: format!("cannot compare {} with {}", self, other),
                });
            }
        })
    }

    /// Adds two values using the rules common among SQL implementations.
    pub fn sql_add(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Self::Number(lhs), Self::Number(rhs)) => (*lhs + *rhs).into(),
            (Self::Timestamp(ts, tz), Self::Interval(dur)) | (Self::Interval(dur), Self::Timestamp(ts, tz)) => {
                Self::Timestamp(
                    try_or_overflow!(
                        ts.checked_add_signed(Duration::microseconds(*dur)),
                        "{} + {}us",
                        ts,
                        dur
                    ),
                    *tz,
                )
            }
            (Self::Interval(a), Self::Interval(b)) => {
                Self::Interval(try_or_overflow!(a.checked_add(*b), "{} + {}", a, b))
            }
            _ => {
                return Err(Error::InvalidArguments {
                    name: "+",
                    cause: format!("cannot add {} to {}", self, other),
                });
            }
        })
    }

    /// Subtracts two values using the rules common among SQL implementations.
    pub fn sql_sub(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Self::Number(lhs), Self::Number(rhs)) => (*lhs - *rhs).into(),
            (Self::Timestamp(ts, tz), Self::Interval(dur)) => Self::Timestamp(
                try_or_overflow!(
                    ts.checked_sub_signed(Duration::microseconds(*dur)),
                    "{} - {}us",
                    ts,
                    dur
                ),
                *tz,
            ),
            (Self::Interval(a), Self::Interval(b)) => {
                Self::Interval(try_or_overflow!(a.checked_sub(*b), "{} + {}", a, b))
            }
            _ => {
                return Err(Error::InvalidArguments {
                    name: "-",
                    cause: format!("cannot subtract {} from {}", self, other),
                });
            }
        })
    }

    /// Multiplies two values using the rules common among SQL implementations.
    pub fn sql_mul(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Self::Number(lhs), Self::Number(rhs)) => (*lhs * *rhs).into(),
            (Self::Number(m), Self::Interval(dur)) | (Self::Interval(dur), Self::Number(m)) => {
                let mult_res = *m * Number::from(*dur);
                Self::Interval(try_or_overflow!(mult_res.to::<i64>(), "{} microseconds", mult_res))
            }
            _ => {
                return Err(Error::InvalidArguments {
                    name: "*",
                    cause: format!("cannot multiply {} with {}", self, other),
                });
            }
        })
    }

    /// Divides two values using the rules common among SQL implementations.
    pub fn sql_float_div(&self, other: &Self) -> Result<Self, Error> {
        Ok(match (self, other) {
            (Self::Number(_), Self::Number(rhs)) | (Self::Interval(_), Self::Number(rhs))
                if rhs.to_sql_bool() == Some(false) =>
            {
                Self::Null
            }
            (Self::Number(lhs), Self::Number(rhs)) => (*lhs / *rhs).into(),
            (Self::Interval(dur), Self::Number(d)) => {
                let mult_res = Number::from(*dur) / *d;
                Self::Interval(try_or_overflow!(mult_res.to::<i64>(), "{} microseconds", mult_res))
            }
            _ => {
                return Err(Error::InvalidArguments {
                    name: "/",
                    cause: format!("cannot divide {} by {}", self, other),
                });
            }
        })
    }

    /// Concatenates multiple values into a string.
    pub fn sql_concat(values: impl Iterator<Item = Self>) -> Result<Self, Error> {
        let mut res = Bytes::default();
        let mut should_check_binary = false;
        for item in values {
            match item {
                Self::Null => {
                    return Ok(Self::Null);
                }
                Self::Number(n) => {
                    write!(&mut res.bytes, "{}", n).unwrap();
                }
                Self::Bytes(mut b) => {
                    res.bytes.append(&mut b.bytes);
                    if b.is_binary {
                        if res.is_binary {
                            should_check_binary = true;
                        } else {
                            res.is_binary = true;
                        }
                    }
                }
                Self::Timestamp(timestamp, tz) => {
                    write!(
                        &mut res.bytes,
                        "{}",
                        tz.from_utc_datetime(&timestamp).format(TIMESTAMP_FORMAT)
                    )
                    .unwrap();
                }
                Self::Interval(interval) => {
                    write!(&mut res.bytes, "INTERVAL {} MICROSECOND", interval).unwrap();
                }
                Self::Array(_) => {
                    return Err(Error::InvalidArguments {
                        name: "||",
                        cause: format!("cannot concatenate arrays using || operator"),
                    });
                }
            }
        }

        if should_check_binary {
            res.is_binary = from_utf8(&res.bytes).is_err();
        }
        Ok(Self::Bytes(res))
    }

    /// Checks whether this value is truthy in SQL sense.
    ///
    /// All nonzero numbers are considered "true", and both NULL and zero are
    /// considered "false". All other types cause the `InvalidArguments` error.
    pub fn is_sql_true(&self, name: &'static str) -> Result<bool, Error> {
        match self {
            Self::Null => Ok(false),
            Self::Number(n) => Ok(n.to_sql_bool() == Some(true)),
            _ => Err(Error::InvalidArguments {
                name,
                cause: format!("truth value of {} is undefined", self),
            }),
        }
    }
}

/// The error indicating the expected type.
#[derive(Debug)]
pub struct TryFromValueError(&'static str);

impl fmt::Display for TryFromValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

macro_rules! impl_try_from_value {
    ($T:ty, $name:expr) => {
        impl TryFrom<Value> for $T {
            type Error = TryFromValueError;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                Number::try_from(value)?
                    .to::<Self>()
                    .ok_or(TryFromValueError($name))
            }
        }

        impl TryFrom<Value> for Option<$T> {
            type Error = TryFromValueError;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                match value {
                    Value::Null => return Ok(None),
                    Value::Number(n) => {
                        if let Some(v) = n.to::<$T>() {
                            return Ok(Some(v));
                        }
                    }
                    _ => {}
                }
                Err(TryFromValueError(concat!("nullable ", $name)))
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

impl TryFrom<Value> for Number {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Number(n) => Ok(n),
            _ => Err(TryFromValueError("number")),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bytes(Bytes {
                is_binary: false,
                bytes,
            }) => Ok(unsafe { Self::from_utf8_unchecked(bytes) }),
            _ => Err(TryFromValueError("string")),
        }
    }
}

impl TryFrom<Value> for Vec<u8> {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bytes(Bytes { bytes, .. }) => Ok(bytes),
            _ => Err(TryFromValueError("bytes string")),
        }
    }
}

impl TryFrom<Value> for Option<bool> {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Null => Ok(None),
            Value::Number(n) => Ok(n.to_sql_bool()),
            _ => Err(TryFromValueError("nullable boolean")),
        }
    }
}

impl TryFrom<Value> for Arc<[Value]> {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Array(v) => Ok(v),
            _ => Err(TryFromValueError("array")),
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
        Self::Bytes(Bytes {
            is_binary: false,
            bytes: value.into_bytes(),
        })
    }
}

impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Bytes(Bytes {
            is_binary: from_utf8(&bytes).is_err(),
            bytes,
        })
    }
}

impl From<Bytes> for Value {
    fn from(b: Bytes) -> Self {
        Self::Bytes(b)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, T::into)
    }
}
