//! Array.

use crate::value::Value;
use std::{
    array::from_fn,
    iter::successors,
    num::{NonZeroU32, NonZeroU64},
    sync::Arc,
};

use auto_enums::auto_enum;
use rand::{seq::SliceRandom as _, Rng as _, RngCore};

/// Parameters for a balanced numerical Feistel network.
//
// A Feistel network can be used to generate a permutation by "encrypting a number". Suppose the
// domain we are working on $ℤ_n$, where $n$ is the length of the array we should be shuffling. We
// split every number $i ∈ ℤ_n$ by div-rem into a pair $(a, b) ∈ (ℤ_m)^2$, where $i = am + b$. The
// split-size $m$ to chosen to be $⌈√n⌉$ to maximize the acceptance rate $r = |ℤ_n| / |ℤ_m|^2$. The
// pair $(a, b)$ is encrypted through the round function:
//
// $$ (a', b') ← (b, a ⊞ f(k, b)) $$
//
// where $k$ is the encryption key for this round, $f$ is a pseudo-random function, and ⊞ is
// addition modulo $m$. It is clear that such mapping is reversible[^1], and thus a bijection i.e.
// permutation on $(ℤ_m)^2$. The round function is repeated 8 times with 8 different values of $k$
// to improve randomness.
//
// The encryption result will be in $(ℤ_m)^2$, where only a ratio $r$ of all values can be mapped
// back to $ℤ_n$. For those rejected $(a', b')$, we can use "cycle walking" i.e. repeatedly encrypt
// until it reaches back within $ℤ_n$. This works because the domain $(ℤ_m)^2$ is finite so
// repeated permutation will form a cycle.
//
// [^1]: We can get back the original value by $ (a, b) ← (b' ⊟ f(k, a'), a') $.
#[derive(Clone, Debug)]
struct Feistel {
    /// The seed for Feistel key scheduling.
    seed: [u64; Self::ROUNDS],

    /// The modulus (m) used to split the number into two parts.
    ///
    /// We require `modulo * modulo >= len`.
    ///
    /// A modulo of `None` means 2<sup>32</sup> exactly.
    modulo: Option<NonZeroU32>,

    /// The base-2 mask computed from the modulus allowing us to compute `x % modulo` without
    /// actually invoking the `%` operator.
    ///
    /// We require `mask + 1 >= modulo > mask/2 + 1` and must be all-1-bits.
    mask: u32,

    /// Maximum accepted number after being split by `modulo`.
    ///
    /// We require `max.0 * modulo + max.1 == len`, and both fields being less than `modulo`.
    max: (u32, u32),
}

impl Feistel {
    /// Default number of rounds in the Feistel network.
    const ROUNDS: usize = 8;

    /// Splits a 64-bit number into two 32-bit numbers separated by the given modulus.
    fn split_number(i: u64, modulo: Option<NonZeroU32>) -> (u32, u32) {
        let divmod = if let Some(modulo) = modulo {
            let modulo = NonZeroU64::from(modulo);
            (i / modulo, i % modulo)
        } else {
            (i >> 32, i & 0xffff_ffff)
        };
        (divmod.0 as u32, divmod.1 as u32)
    }

    /// Constructs a new Feistel network with the given domain size.
    ///
    /// The result is *not yet ready to use*. The `seed` has to be explicitly randomized to fully
    /// initialize the network.
    fn prepare(len: u64) -> Self {
        let max = len - 1;
        // let sqrt = max.isqrt() as u32;
        let sqrt = (max as f64).sqrt() as u32;
        let modulo = sqrt.checked_add(1).and_then(NonZeroU32::new);

        Self {
            seed: [0; Self::ROUNDS],
            modulo,
            mask: !0_u32 >> sqrt.leading_zeros(),
            max: Self::split_number(max, modulo),
        }
    }

    /// Re-seed the Feistel network.
    fn shuffle(&mut self, rng: &mut dyn RngCore) {
        rng.fill(&mut self.seed);
    }

    /// Permutes a number.
    ///
    /// It is expected both input and output to be less than `len`.
    fn get(&self, i: u64) -> u64 {
        use fastrand::Rng;

        let (mut a, mut b) = Self::split_number(i, self.modulo);
        loop {
            for key in &self.seed {
                let c = Rng::with_seed(key.wrapping_add(b as _)).u32(..) & self.mask;
                (a, b) = (b, c.wrapping_add(a));
                if let Some(modulo) = self.modulo {
                    let modulo = modulo.get();
                    // we knew 0 ≤ c < 2^⌈log₂m⌉ < 2m, so c + a < 3m, so we at most need to subtract twice.
                    if b >= modulo {
                        b -= modulo;
                        if b >= modulo {
                            b -= modulo;
                        }
                    }
                }
            }
            if (a, b) <= self.max {
                return if let Some(modulo) = self.modulo {
                    u64::from(a) * u64::from(modulo.get()) + u64::from(b)
                } else {
                    u64::from(a) << 32 | u64::from(b)
                };
            }
        }
    }
}

#[derive(Clone, Debug)]
enum P {
    /// Pre-computed index permutation for short arrays.
    Simple([u8; Permutation::SHORT_ARRAY_LEN]),
    /// Feistel-based permutation for long arrays.
    Feistel(Feistel),
}

/// A permutation of array indices.
#[derive(Clone, Debug)]
pub struct Permutation(P);

impl Permutation {
    const SHORT_ARRAY_LEN: usize = 96;

    /// Creates a new permutation.
    ///
    /// The result is *not yet ready to use*. One must explicitly call [`self.shuffle()`] later to
    /// initialize the permutation.
    pub fn prepare(len: u64) -> Self {
        if len <= Self::SHORT_ARRAY_LEN as u64 {
            Self(P::Simple(from_fn(|i| u8::try_from(i).unwrap())))
        } else {
            Self(P::Feistel(Feistel::prepare(len)))
        }
    }

    /// Get the permuted index at original index `i`.
    pub fn get(&self, i: u64) -> u64 {
        match &self.0 {
            P::Simple(permutation) => permutation[i as usize].into(),
            P::Feistel(feistel) => feistel.get(i),
        }
    }

    /// Iterates the permutation.
    #[auto_enum(Iterator)]
    pub fn iter(&self, len: u64) -> impl Iterator<Item = u64> + '_ {
        match &self.0 {
            P::Simple(permutation) => permutation[..(len as usize)].iter().map(|i| (*i).into()),
            P::Feistel(feistel) => (0..len).map(|i| feistel.get(i)),
        }
    }

    /// Shuffles (reseeds) the permutation.
    ///
    /// The `len` provided must be the same in every call of `shuffle`.
    /// Otherwise
    pub fn shuffle(&mut self, len: u64, rng: &mut dyn RngCore) {
        match &mut self.0 {
            P::Simple(permutation) => permutation[..(len as usize)].shuffle(rng),
            P::Feistel(feistel) => feistel.shuffle(rng),
        }
    }
}

#[derive(Clone, Debug)]
enum A {
    /// An concrete array of values.
    Array(Box<[Value]>),

    /// A series of numbers.
    Series {
        /// The start value of the series.
        start: Value,
        /// The step size.
        step: Value,
        /// Expected length of the series.
        len: u64,
    },

    /// An already-shuffled array.
    Permuted {
        /// The index permutation.
        permutation: Permutation,
        /// The pre-shuffled array.
        inner: Array,
    },
}

/// An array, which may be lazily evaluated.
///
/// This type only guarantees O(1) random access. The actual content is not necessarily a continuous
/// storage of values.
#[derive(Clone, Debug)]
pub struct Array(Arc<A>);

impl Array {
    /// Iterates the content of the array.
    #[auto_enum(Iterator)]
    pub fn iter(&self) -> impl Iterator<Item = Value> + '_ {
        match &*self.0 {
            A::Array(values) => values.iter().cloned(),
            A::Series { start, step, len } => successors(
                len.checked_sub(1).map(|remaining| (remaining, start.clone())),
                |(remaining, cur)| {
                    let remaining = remaining.checked_sub(1)?;
                    let next = cur.sql_add(step).ok()?;
                    Some((remaining, next))
                },
            )
            .map(|(_, value)| value),
            A::Permuted { permutation, inner } => permutation.iter(inner.len()).map(|i| inner.get(i)),
        }
    }

    /// Gets the value at the given *0-based* index.
    ///
    /// # Panics
    ///
    /// This method *may* panic when `index >= self.len()`, or it may return some garbage value.
    /// We assume the bounds checking is already done previously.
    pub fn get(&self, index: u64) -> Value {
        match &*self.0 {
            A::Array(values) => values[index as usize].clone(),
            A::Series { start, step, .. } => step
                .sql_mul(&Value::Number(index.into()))
                .unwrap()
                .sql_add(start)
                .unwrap(),
            A::Permuted { permutation, inner } => inner.get(permutation.get(index)),
        }
    }

    /// Gets the length of the array.
    pub fn len(&self) -> u64 {
        match &*self.0 {
            A::Array(values) => values.len() as u64,
            A::Series { len, .. } => *len,
            A::Permuted { inner, .. } => inner.len(),
        }
    }

    /// Checks if the array is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Constructs an array from concrete values.
    pub fn from_values(values: impl IntoIterator<Item = Value>) -> Self {
        Self(Arc::new(A::Array(values.into_iter().collect())))
    }

    /// Constructs an array of a generated series.
    pub fn new_series(start: Value, step: Value, len: u64) -> Self {
        Self(Arc::new(A::Series { start, step, len }))
    }

    /// Applies permutation to the array.
    #[must_use]
    pub fn add_permutation(&self, permutation: Permutation) -> Self {
        Self(Arc::new(A::Permuted {
            permutation,
            inner: self.clone(),
        }))
    }
}

impl PartialEq for Array {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feistel_is_permutation() {
        let mut feistel = Feistel::prepare(256);
        feistel.seed = [
            0x09f7ee6201f67de4,
            0x536db2a4c7976eb7,
            0x15640fedcdd650fe,
            0x764ba03cbe3bccc8,
            0xcdca39b28fa0e573,
            0x57e9d5fffeb5f4e4,
            0xac82463f11dcfe32,
            0x820461c4207b305b,
        ];
        let shuffled = (0..256).map(|i| feistel.get(i)).collect::<Vec<_>>();
        let mut sorted = shuffled.clone();
        sorted.sort();
        assert!(sorted.iter().copied().eq(0..256), "{sorted:?}");
        assert_ne!(shuffled, sorted);
    }
}
