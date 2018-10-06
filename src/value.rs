use num_traits::FromPrimitive;
use std::{
    cmp::Ordering,
    fmt,
    io::{self, Write},
    ops, slice,
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
}

impl From<u64> for Number {
    fn from(value: u64) -> Self {
        Number(N::U(value))
    }
}

impl From<i64> for Number {
    fn from(value: i64) -> Self {
        Number(N::I(value))
    }
}

impl From<f64> for Number {
    fn from(value: f64) -> Self {
        Number(N::F(value))
    }
}

impl ops::Neg for Number {
    type Output = Number;
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
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (N::U(a), N::U(b)) => a == b,
            (N::I(a), N::I(b)) => a == b,
            (N::F(a), N::F(b)) => a == b,

            (N::U(a), N::I(b)) => {
                if b < 0 {
                    false
                } else {
                    a == (b as u64)
                }
            }
            (N::I(a), N::U(b)) => {
                if a < 0 {
                    false
                } else {
                    (a as u64) == b
                }
            }

            (N::F(a), N::U(b)) => a == (b as f64),
            (N::F(a), N::I(b)) => a == (b as f64),
            (N::U(a), N::F(b)) => (a as f64) == b,
            (N::I(a), N::F(b)) => (a as f64) == b,
        }
    }
}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.0, other.0) {
            (N::U(a), N::U(b)) => a.partial_cmp(&b),
            (N::I(a), N::I(b)) => a.partial_cmp(&b),
            (N::F(a), N::F(b)) => a.partial_cmp(&b),

            (N::U(a), N::I(b)) => {
                if b < 0 {
                    Some(Ordering::Greater)
                } else {
                    let b = b as u64;
                    a.partial_cmp(&b)
                }
            }
            (N::I(a), N::U(b)) => {
                if a < 0 {
                    Some(Ordering::Less)
                } else {
                    let a = a as u64;
                    a.partial_cmp(&b)
                }
            }

            (N::F(a), N::U(b)) => {
                let b = b as f64;
                a.partial_cmp(&b)
            }
            (N::F(a), N::I(b)) => {
                let b = b as f64;
                a.partial_cmp(&b)
            }
            (N::U(a), N::F(b)) => {
                let a = a as f64;
                a.partial_cmp(&b)
            }
            (N::I(a), N::F(b)) => {
                let a = a as f64;
                a.partial_cmp(&b)
            }
        }
    }
}

#[derive(Clone, Debug)]
enum V {
    /// A number.
    Number(Number),
    /// A string.
    String(String),
    /// A byte string, guaranteed to be *not* containing UTF-8.
    Bytes(Vec<u8>),
}

/// A scalar value.
#[derive(Clone, Debug)]
pub struct Value(V);

impl Value {
    /// Writes the SQL representation of this value into a write stream.
    pub fn write_sql(&self, mut output: impl Write) -> Result<(), io::Error> {
        match &self.0 {
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
}

pub trait AsValue {
    fn as_value(&self) -> Option<&Value>;
}

impl AsValue for Value {
    fn as_value(&self) -> Option<&Value> {
        Some(self)
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
impl_try_from_value!(i8, "8-bit signed integer");
impl_try_from_value!(i16, "16-bit signed integer");
impl_try_from_value!(i32, "32-bit signed integer");
impl_try_from_value!(i64, "64-bit signed integer");
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

    fn try_from_value(value: &'s Value) -> Option<&'s str> {
        match &value.0 {
            V::String(s) => Some(s),
            _ => None,
        }
    }
}

impl From<Number> for Value {
    fn from(number: Number) -> Self {
        Value(V::Number(number))
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value(V::Number(value.into()))
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value(V::Number(value.into()))
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
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
