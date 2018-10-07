use num_traits::{FromPrimitive, ToPrimitive};
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
struct I65 {
    lsbit: bool,
    msb: i64,
}

impl From<I65> for i128 {
    fn from(value: I65) -> Self {
        Self::from(value.msb) << 1 | Self::from(value.lsbit)
    }
}
impl From<I65> for f64 {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_precision_loss))]
    fn from(value: I65) -> Self {
        (value.msb as Self) * 2.0 + Self::from(u8::from(value.lsbit))
    }
}

impl I65 {
    fn wrapping_neg(self) -> Self {
        Self {
            lsbit: self.lsbit,
            msb: i64::from(self.lsbit).wrapping_add(self.msb).wrapping_neg(),
        }
    }

    fn try_from_i128(v: i128) -> Option<Self> {
        Some(Self {
            lsbit: (v & 1) != 0,
            msb: (v >> 1).to_i64()?,
        })
    }
}

#[derive(Copy, Clone, Debug)]
enum N {
    Int(I65),
    Float(f64),
}

#[derive(Copy, Clone, Debug)]
pub struct Number(N);

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            N::Int(v) => i128::from(v).fmt(f),
            N::Float(v) => v.fmt(f),
        }
    }
}

impl Number {
    pub fn to<P: FromPrimitive>(&self) -> Option<P> {
        match self.0 {
            N::Int(v) => P::from_i128(v.into()),
            N::Float(v) => P::from_f64(v),
        }
    }

    pub fn to_sql_bool(&self) -> Option<bool> {
        match self.0 {
            N::Int(v) => Some(v != I65::default()),
            N::Float(v) if v.is_nan() => None,
            N::Float(v) => Some(v != 0.0),
        }
    }
}

macro_rules! impl_from_int_for_number {
    ($($ty:ty),*) => {
        $(impl From<$ty> for Number {
            #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_possible_wrap))] // u63 to i64 won't wrap.
            fn from(value: $ty) -> Self {
                Number(N::Int(I65 {
                    lsbit: (value & 1) != 0,
                    msb: (value >> 1) as i64,
                }))
            }
        })*
    }
}
impl_from_int_for_number!(u8, u16, u32, u64, i8, i16, i32, i64);

impl From<bool> for Number {
    fn from(value: bool) -> Self {
        Number(N::Int(I65 { lsbit: value, msb: 0 }))
    }
}
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
    fn from(n: N) -> Self {
        match n {
            N::Int(i) => i.into(),
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
                    if let Some(c) = i128::from(a).$checked(i128::from(b)).and_then(I65::try_from_i128) {
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

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Bytes {
    bytes: Vec<u8>,
    is_binary: bool,
}

impl Bytes {
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

    #[doc(hidden)]
    __NonExhaustive,
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
            Value::__NonExhaustive => {}
        }
        Ok(())
    }

    /// Compares two values using the rules common among SQL implementations.
    ///
    /// * Comparing with NULL always return `None`.
    /// * Numbers are ordered by value.
    /// * Strings are ordered by UTF-8 binary collation.
    /// * Comparing between different types are inconsistent among database
    ///     engines, thus this function will just error with `InvalidArguments`.
    pub fn sql_cmp(&self, other: &Self, name: Function) -> Result<Option<Ordering>, Error> {
        Ok(match (self, other) {
            (Value::Null, _) | (_, Value::Null) => None,
            (Value::Number(a), Value::Number(b)) => a.partial_cmp(b),
            (Value::Bytes(a), Value::Bytes(b)) => a.bytes.partial_cmp(&b.bytes),
            _ => {
                return Err(ErrorKind::InvalidArguments {
                    name,
                    cause: format!("comparing values of different types"),
                }
                .into())
            }
        })
    }

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
                Value::__NonExhaustive => {}
            }
        }

        if should_check_binary {
            res.is_binary = from_utf8(&res.bytes).is_err();
        }
        Ok(Value::Bytes(res))
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

impl<'s> TryFromValue<'s> for Option<bool> {
    const NAME: &'static str = "nullable boolean";

    #[cfg_attr(feature = "cargo-clippy", allow(clippy::use_self))] // rust-lang-nursery/rust-clippy#1993
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
