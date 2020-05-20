//! Number.

use num_cmp::NumCmp;
use std::{cmp::Ordering, convert::TryFrom, fmt, u64};

/// Implementation of a number.
#[derive(Copy, Clone, Debug)]
enum N {
    /// An integer.
    I(i128),
    /// A finite floating-point number.
    F(f64),
}

/// The error returned in numerical arithmetics.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NumberError {
    /// Computation result overflows the range.
    Overflow,
    /// Computation result is NaN.
    NaN,
}

macro_rules! impl_from_integer_for_number {
    ($($ty:ty),+) => {$(
        impl From<$ty> for Number {
            #[allow(trivial_numeric_casts)]
            fn from(v: $ty) -> Self {
                Self(N::I(v as _))
            }
        }
    )+}
}

// do not include u128 (we normally don't need u128 anyway).
impl_from_integer_for_number!(u8, u16, u32, u64, usize, i8, i16, i32, i64, i128, isize);

// we don't care about f32.
impl TryFrom<f64> for Number {
    type Error = NumberError;
    fn try_from(v: f64) -> Result<Self, Self::Error> {
        if v.is_finite() {
            Ok(Self(N::F(v)))
        } else if v.is_nan() {
            Err(NumberError::NaN)
        } else {
            Err(NumberError::Overflow)
        }
    }
}

impl From<bool> for Number {
    fn from(v: bool) -> Self {
        Self(N::I(if v { 1 } else { 0 }))
    }
}

impl From<Number> for f64 {
    #[allow(clippy::cast_precision_loss)]
    fn from(n: Number) -> Self {
        match n.0 {
            N::I(v) => v as _,
            N::F(v) => v,
        }
    }
}

macro_rules! impl_try_from_number_for_integer {
    ($($ty:ty),+) => {$(
        impl TryFrom<Number> for $ty {
            type Error = NumberError;
            fn try_from(n: Number) -> Result<Self, Self::Error> {
                match n.0 {
                    N::I(v) => Self::try_from(v).map_err(|_| NumberError::Overflow),
                    N::F(v) if Self::min_value() as f64 <= v && v <= Self::max_value() as f64 => Ok(v as _),
                    _ => Err(NumberError::Overflow),
                }
            }
        }
    )+}
}

// do not include u128
impl_try_from_number_for_integer!(u8, u16, u32, u64, usize, i8, i16, i32, i64, i128, isize);

/// An SQL number (could represent an integer or floating point number).
#[derive(Copy, Clone, Debug)]
pub struct Number(N);

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            N::I(v) => v.fmt(f),
            N::F(v) => {
                let mut output = ryu::Buffer::new();
                f.write_str(output.format(v))
            }
        }
    }
}

#[allow(clippy::should_implement_trait)]
impl Number {
    pub(crate) fn from_finite_f64(v: f64) -> Self {
        debug_assert!(v.is_finite(), "failed: ({:?}).is_finite()", v);
        Self(N::F(v))
    }

    /// Compares this value with zero.
    pub fn sql_sign(self) -> Ordering {
        match self.0 {
            N::I(v) => v.cmp(&0),
            N::F(v) => v.partial_cmp(&0.0).unwrap_or(Ordering::Equal),
        }
    }

    /// Adds this number with another number.
    pub fn add(self, other: Self) -> Result<Self, NumberError> {
        if let (N::I(a), N::I(b)) = (self.0, other.0) {
            if let Some(c) = a.checked_add(b) {
                return Ok(Self(N::I(c)));
            }
        }
        Self::try_from(f64::from(self) + f64::from(other))
    }

    /// Negates itself.
    pub fn neg(self) -> Self {
        if let N::I(a) = self.0 {
            if let Some(c) = a.checked_neg() {
                return Self(N::I(c));
            }
        }
        Self::from_finite_f64(-f64::from(self))
    }

    /// Subtracts this number from another number.
    pub fn sub(self, other: Self) -> Result<Self, NumberError> {
        if let (N::I(a), N::I(b)) = (self.0, other.0) {
            if let Some(c) = a.checked_sub(b) {
                return Ok(Self(N::I(c)));
            }
        }
        Self::try_from(f64::from(self) - f64::from(other))
    }

    /// Multiplies this number with another number.
    pub fn mul(self, other: Self) -> Result<Self, NumberError> {
        if let (N::I(a), N::I(b)) = (self.0, other.0) {
            if let Some(c) = a.checked_mul(b) {
                return Ok(Self(N::I(c)));
            }
        }
        Self::try_from(f64::from(self) * f64::from(other))
    }

    /// Divides this number with another number, truncated as an integer towards zero.
    pub fn div(self, other: Self) -> Result<Self, NumberError> {
        match (self.0, other.0) {
            // Divide by zero is _always_ NULL in SQL, never infinity.
            (_, N::I(0)) => return Err(NumberError::NaN),
            (_, N::F(v)) if v == 0.0 => return Err(NumberError::NaN),
            (N::I(a), N::I(b)) => {
                if let Some(c) = a.checked_div(b) {
                    return Ok(Self(N::I(c)));
                }
            }
            _ => {}
        }
        Self::try_from((f64::from(self) / f64::from(other)).trunc())
    }

    /// Computes the remainder (modulus) when this number is divided by another number.
    pub fn rem(self, other: Self) -> Result<Self, NumberError> {
        match (self.0, other.0) {
            // Divide by zero is _always_ NULL in SQL, never infinity.
            (_, N::I(0)) => Err(NumberError::NaN),
            (_, N::F(v)) if v == 0.0 => Err(NumberError::NaN),
            // Avoid Rust's special treatment of `-2^127 % -1`.
            (_, N::I(-1)) => Ok(Self(N::I(0))),
            // All other cases involving integer will never overflow.
            (N::I(a), N::I(b)) => Ok(Self(N::I(a % b))),
            // The result is always less than `other` and never overflows.
            _ => Ok(Self::from_finite_f64(f64::from(self) % f64::from(other))),
        }
    }

    /// Divides this number with another number using floating point arithmetic.
    pub fn float_div(self, other: Self) -> Result<Self, NumberError> {
        let a = f64::from(self);
        let b = f64::from(other);
        if b == 0.0 {
            Err(NumberError::NaN)
        } else {
            Self::try_from(a / b)
        }
    }
}

macro_rules! impl_partial_ord_method {
    ($(fn $fn_name:ident(...) -> $ret:ty = $method:ident;)+) => {
        $(fn $fn_name(&self, other: &Self) -> $ret {
            match (self.0, other.0) {
                (N::I(a), N::I(b)) => a.$method(b),
                (N::I(a), N::F(b)) => a.$method(b),
                (N::F(a), N::I(b)) => a.$method(b),
                (N::F(a), N::F(b)) => a.$method(b),
            }
        })+
    }
}

#[allow(clippy::partialeq_ne_impl)]
impl PartialEq for Number {
    impl_partial_ord_method! {
        fn eq(...) -> bool = num_eq;
        fn ne(...) -> bool = num_ne;
    }
}

impl Eq for Number {}

impl PartialOrd for Number {
    impl_partial_ord_method! {
        fn partial_cmp(...) -> Option<Ordering> = num_cmp;
        fn lt(...) -> bool = num_lt;
        fn gt(...) -> bool = num_gt;
        fn le(...) -> bool = num_le;
        fn ge(...) -> bool = num_ge;
    }
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{f64, i128};

    #[test]
    fn test_from_float() {
        assert_eq!(Number::try_from(2.5), Ok(Number(N::F(2.5))));
        assert_eq!(Number::try_from(0.0), Ok(Number(N::F(0.0))));
        assert_eq!(Number::try_from(f64::MAX), Ok(Number(N::F(f64::MAX))));
        assert_eq!(Number::try_from(f64::MIN), Ok(Number(N::F(f64::MIN))));
        assert_eq!(Number::try_from(-2.5), Ok(Number(N::F(-2.5))));
        assert_eq!(Number::try_from(-0.0), Ok(Number(N::F(-0.0))));
        assert_eq!(Number::try_from(-f64::MAX), Ok(Number(N::F(-f64::MAX))));
        assert_eq!(Number::try_from(-f64::MIN), Ok(Number(N::F(-f64::MIN))));
        assert_eq!(Number::try_from(f64::INFINITY), Err(NumberError::Overflow));
        assert_eq!(Number::try_from(f64::NAN), Err(NumberError::NaN));
        assert_eq!(Number::try_from(-f64::INFINITY), Err(NumberError::Overflow));
    }

    #[test]
    fn test_display() {
        assert_eq!(Number::from(123).to_string(), "123");
        assert_eq!(Number::from(-123).to_string(), "-123");
        assert_eq!(Number::from(0).to_string(), "0");
        assert_eq!(
            Number::from(i128::MAX).to_string(),
            "170141183460469231731687303715884105727"
        );
        assert_eq!(
            Number::from(i128::MIN).to_string(),
            "-170141183460469231731687303715884105728"
        );
        assert_eq!(Number::from_finite_f64(0.0).to_string(), "0.0");
        assert_eq!(Number::from_finite_f64(-1.2).to_string(), "-1.2");
        assert_eq!(Number::from_finite_f64(1.5e300).to_string(), "1.5e300");
        assert_eq!(Number::from_finite_f64(1e-200).to_string(), "1e-200");
    }

    #[test]
    fn test_sql_sign() {
        assert_eq!(Number::from(1).sql_sign(), Ordering::Greater);
        assert_eq!(Number::from(0).sql_sign(), Ordering::Equal);
        assert_eq!(Number::from(-1).sql_sign(), Ordering::Less);
        assert_eq!(Number::from_finite_f64(1.0).sql_sign(), Ordering::Greater);
        assert_eq!(Number::from_finite_f64(0.0).sql_sign(), Ordering::Equal);
        assert_eq!(Number::from_finite_f64(-0.0).sql_sign(), Ordering::Equal);
        assert_eq!(Number::from_finite_f64(-1.0).sql_sign(), Ordering::Less);
        assert_eq!(Number::from(i128::MAX).sql_sign(), Ordering::Greater);
        assert_eq!(Number::from(i128::MIN).sql_sign(), Ordering::Less);
        assert_eq!(Number::from_finite_f64(f64::MAX).sql_sign(), Ordering::Greater);
        assert_eq!(Number::from_finite_f64(-f64::MAX).sql_sign(), Ordering::Less);
    }

    #[test]
    fn test_eq() {
        assert_eq!(Number::from(1_u64), Number::from(1_i64));
        assert_eq!(Number::from(0_u64), Number::from(0_i64));
        assert_eq!(Number::from(i128::MAX), Number::from(i128::MAX));
        assert_eq!(Number::from(i128::MIN), Number::from(i128::MIN));
        assert_eq!(Number::from_finite_f64(2.5), Number::from_finite_f64(2.5));
        assert_eq!(Number::from_finite_f64(0.0), Number::from_finite_f64(-0.0));
        assert_eq!(Number::from_finite_f64(0.0), Number::from(0));
        assert_eq!(Number::from_finite_f64(5.0), Number::from(5));
        assert_eq!(Number::from_finite_f64(i128::MIN as f64), Number::from(i128::MIN));

        assert_ne!(Number::from(i128::MAX), Number::from(i128::MAX - 1));
        assert_ne!(Number::from(0_u64), Number::from_finite_f64(f64::MIN_POSITIVE));
        // i128::MAX never equal due to not enough precision.
        assert_ne!(Number::from_finite_f64(i128::MAX as f64), Number::from(i128::MAX));
        assert_ne!(Number::from_finite_f64(i128::MIN as f64), Number::from(i128::MIN + 1));
    }

    #[test]
    fn test_add() {
        assert_eq!(Number::from(3).add(Number::from(4)), Ok(Number::from(7)));
        assert_eq!(
            Number::from_finite_f64(3.5).add(Number::from(-4)),
            Ok(Number::from_finite_f64(-0.5))
        );
        assert_eq!(
            Number::from(i128::MAX).add(Number::from(i128::MAX)),
            Number::try_from(340282366920938463463374607431768211454.0)
        );
        assert_eq!(
            Number::from_finite_f64(f64::MAX).add(Number::from_finite_f64(f64::MAX)),
            Err(NumberError::Overflow)
        );
    }

    #[test]
    fn test_sub() {
        assert_eq!(Number::from(3).sub(Number::from(4)), Ok(Number::from(-1)));
        assert_eq!(
            Number::from_finite_f64(3.5).sub(Number::from(-4)),
            Ok(Number::from_finite_f64(7.5))
        );
        assert_eq!(
            Number::from(i128::MIN).sub(Number::from(i128::MAX)),
            Number::try_from(-340282366920938463463374607431768211455.0)
        );
        assert_eq!(
            Number::from_finite_f64(f64::MAX).sub(Number::from_finite_f64(f64::MIN)),
            Err(NumberError::Overflow)
        );
    }

    #[test]
    fn test_mul() {
        assert_eq!(Number::from(3).mul(Number::from(4)), Ok(Number::from(12)));
        assert_eq!(
            Number::from_finite_f64(3.5).mul(Number::from(-4)),
            Ok(Number::from_finite_f64(-14.0))
        );
        assert_eq!(
            Number::from(i128::MIN).mul(Number::from(i128::MAX)),
            Number::try_from(-28948022309329048855892746252171976963147354982949671778132708698262398304256.0)
        );
        assert_eq!(
            Number::from_finite_f64(f64::MAX).mul(Number::from_finite_f64(1.25)),
            Err(NumberError::Overflow)
        );
    }

    #[test]
    fn test_float_div() {
        assert_eq!(
            Number::from(3).float_div(Number::from(4)),
            Ok(Number::from_finite_f64(0.75))
        );
        assert_eq!(
            Number::from_finite_f64(3.5).float_div(Number::from(-4)),
            Ok(Number::from_finite_f64(-0.875))
        );
        assert_eq!(
            Number::from(i128::MIN).float_div(Number::from(i128::MAX)),
            Ok(Number::from_finite_f64(-1.0))
        );
        assert_eq!(
            Number::from_finite_f64(f64::MAX).float_div(Number::from_finite_f64(0.25)),
            Err(NumberError::Overflow)
        );
        assert_eq!(Number::from(1).float_div(Number::from(0)), Err(NumberError::NaN));
        assert_eq!(
            Number::from(1).float_div(Number::from_finite_f64(0.0)),
            Err(NumberError::NaN)
        );
    }

    #[test]
    fn test_div() {
        assert_eq!(Number::from(13).div(Number::from(4)), Ok(Number::from(3)));
        assert_eq!(
            Number::from_finite_f64(3.5).div(Number::from_finite_f64(0.3)),
            Ok(Number::from(11))
        );
        assert_eq!(
            Number::from(i128::MIN).div(Number::from(-1)),
            Ok(Number::from_finite_f64(170141183460469231731687303715884105728.0))
        );
        assert_eq!(
            Number::from_finite_f64(f64::MAX).div(Number::from_finite_f64(0.25)),
            Err(NumberError::Overflow)
        );
        assert_eq!(Number::from(1).div(Number::from(0)), Err(NumberError::NaN));
        assert_eq!(Number::from(1).div(Number::from_finite_f64(0.0)), Err(NumberError::NaN));
    }

    #[test]
    fn test_rem() {
        assert_eq!(Number::from(13).rem(Number::from(4)), Ok(Number::from(1)));
        assert_eq!(
            Number::from_finite_f64(3.5).rem(Number::from_finite_f64(0.75)),
            Ok(Number::from_finite_f64(0.5))
        );
        assert_eq!(Number::from(i128::MIN).rem(Number::from(-1)), Ok(Number::from(0)));
        assert_eq!(
            Number::from_finite_f64(f64::MAX).rem(Number::from_finite_f64(0.25)),
            Ok(Number::from_finite_f64(0.0))
        );
        assert_eq!(Number::from(1).rem(Number::from(0)), Err(NumberError::NaN));
        assert_eq!(Number::from(1).rem(Number::from_finite_f64(0.0)), Err(NumberError::NaN));
    }
}
