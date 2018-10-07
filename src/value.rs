use num_traits::{FromPrimitive, ToPrimitive};
use std::{
    cmp::Ordering,
    fmt,
    io::{self, Write},
    ops, slice,
};

use crate::{
    error::{Error, ErrorKind},
    parser::Function,
};

#[derive(Copy, Clone, Debug)]
enum N {
    U(u64),
    I(i64),
    F(f64),
}

#[derive(Copy, Clone, Debug)]
pub struct Number(N);

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            N::U(v) => v.fmt(f),
            N::I(v) => v.fmt(f),
            N::F(v) => v.fmt(f),
        }
    }
}

impl Number {
    pub fn to<P: FromPrimitive>(&self) -> Option<P> {
        match self.0 {
            N::U(v) => P::from_u64(v),
            N::I(v) => P::from_i64(v),
            N::F(v) => P::from_f64(v),
        }
    }

    pub fn to_sql_bool(&self) -> Option<bool> {
        match self.0 {
            N::U(v) => Some(v != 0),
            N::I(v) => Some(v != 0),
            N::F(v) if v.is_nan() => None,
            N::F(v) => Some(v != 0.0),
        }
    }
}

macro_rules! impl_from_number {
    ($variant:ident: $($ty:ty),*) => {
        $(impl From<$ty> for Number {
            fn from(value: $ty) -> Self {
                Number(N::$variant(value.into()))
            }
        })*
    }
}

impl_from_number!(U: u8, u16, u32, u64, bool);
impl_from_number!(I: i8, i16, i32, i64);
impl_from_number!(F: f32, f64);

impl ops::Neg for Number {
    type Output = Self;
    #[cfg_attr(
        feature = "cargo-clippy",
        allow(
            clippy::cast_possible_wrap,
            clippy::cast_precision_loss,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss
        )
    )] // by design
    fn neg(self) -> Self {
        match self.0 {
            N::U(u) if u <= 0x8000_0000_0000_0000 => Number(N::I((u as i64).wrapping_neg())),
            N::U(u) => Number(N::F(-(u as f64))),
            N::I(i) if i < 0 => Number(N::U(i.wrapping_neg() as u64)),
            N::I(i) => Number(N::I(-i)),
            N::F(f) => Number(N::F(-f)),
        }
    }
}

impl PartialEq for Number {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_precision_loss))] // by design
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (N::U(a), N::U(b)) => a == b,
            (N::I(a), N::I(b)) => a == b,
            (N::F(a), N::F(b)) => a == b,

            (N::U(a), N::I(b)) => Some(a) == b.to_u64(),
            (N::I(a), N::U(b)) => a.to_u64() == Some(b),

            (N::F(a), N::U(b)) => a == (b as f64),
            (N::F(a), N::I(b)) => a == (b as f64),
            (N::U(a), N::F(b)) => (a as f64) == b,
            (N::I(a), N::F(b)) => (a as f64) == b,
        }
    }
}

impl PartialOrd for Number {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_precision_loss))] // by design
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.0, other.0) {
            (N::U(a), N::U(b)) => a.partial_cmp(&b),
            (N::I(a), N::I(b)) => a.partial_cmp(&b),
            (N::F(a), N::F(b)) => a.partial_cmp(&b),

            (N::U(a), N::I(b)) => Some(a).partial_cmp(&b.to_u64()),
            (N::I(a), N::U(b)) => a.to_u64().partial_cmp(&Some(b)),

            (N::F(a), N::U(b)) => a.partial_cmp(&(b as f64)),
            (N::F(a), N::I(b)) => a.partial_cmp(&(b as f64)),
            (N::U(a), N::F(b)) => (a as f64).partial_cmp(&b),
            (N::I(a), N::F(b)) => (a as f64).partial_cmp(&b),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum V {
    /// Null.
    Null,
    /// A number.
    Number(Number),
    /// A string.
    String(String),
    /// A byte string, guaranteed to be *not* containing UTF-8.
    Bytes(Vec<u8>),
}

/// A scalar value.
#[derive(Clone, Debug, PartialEq)]
pub struct Value(V);

impl Value {
    /// Writes the SQL representation of this value into a write stream.
    pub fn write_sql(&self, mut output: impl Write) -> Result<(), io::Error> {
        match &self.0 {
            V::Null => {
                output.write_all(b"NULL")?;
            }
            V::Number(number) => {
                write!(output, "{}", number)?;
            }
            V::String(s) => {
                output.write_all(b"'")?;
                for b in s.as_bytes() {
                    output.write_all(if *b == b'\'' { b"''" } else { slice::from_ref(b) })?;
                }
                output.write_all(b"'")?;
            }
            V::Bytes(bytes) => {
                output.write_all(b"x'")?;
                for b in bytes {
                    write!(output, "{:02X}", b)?;
                }
                output.write_all(b"'")?;
            }
        }
        Ok(())
    }

    /// Obtains the null value.
    pub fn null() -> Self {
        Value(V::Null)
    }

    /// Compares two values using the rules common among SQL implementations.
    ///
    /// * Comparing with NULL always return `None`.
    /// * Numbers are ordered by value.
    /// * Strings are ordered by UTF-8 binary collation.
    /// * Comparing between different types are inconsistent among database
    ///     engines, thus this function will just error with `InvalidArguments`.
    pub fn sql_cmp(&self, other: &Self, name: Function) -> Result<Option<Ordering>, Error> {
        Ok(match (&self.0, &other.0) {
            (V::Null, _) | (_, V::Null) => None,
            (V::Number(a), V::Number(b)) => a.partial_cmp(b),
            (V::String(a), V::String(b)) => a.partial_cmp(b),
            (V::String(a), V::Bytes(b)) => a.as_bytes().partial_cmp(b),
            (V::Bytes(a), V::String(b)) => (&**a).partial_cmp(b.as_bytes()),
            (V::Bytes(a), V::Bytes(b)) => a.partial_cmp(b),
            _ => {
                return Err(ErrorKind::InvalidArguments {
                    name,
                    cause: format!("comparing values of different types"),
                }
                .into())
            }
        })
    }
}

pub trait TryFromValue<'s>: Sized {
    const NAME: &'static str;
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
        match value.0 {
            V::Number(n) => Some(n),
            _ => None,
        }
    }
}

impl<'s> TryFromValue<'s> for &'s str {
    const NAME: &'static str = "string";

    fn try_from_value(value: &'s Value) -> Option<Self> {
        match &value.0 {
            V::String(s) => Some(s),
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

impl<'s> TryFromValue<'s> for Option<bool> {
    const NAME: &'static str = "nullable boolean";

    fn try_from_value(value: &'s Value) -> Option<Self> {
        match value.0 {
            V::Null => Some(None),
            V::Number(n) => Some(n.to_sql_bool()),
            _ => None,
        }
    }
}

impl<T: Into<Number>> From<T> for Value {
    fn from(value: T) -> Self {
        Value(V::Number(value.into()))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value(V::String(value))
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        match String::from_utf8(value) {
            Ok(s) => Value(V::String(s)),
            Err(e) => Value(V::Bytes(e.into_bytes())),
        }
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Value::null(), T::into)
    }
}
