//! Random generator functions.

use super::{args_1, args_2, args_3, require, Function};
use crate::{
    error::Error,
    eval::{CompileContext, Compiled, C},
    value::{Number, Value},
};
use rand::distributions::BernoulliError;
use rand_distr::NormalError;
use zipf::ZipfDistribution;

//------------------------------------------------------------------------------

/// The `rand.range` SQL function.
#[derive(Debug)]
pub struct Range;

/// The `rand.range_inclusive` SQL function.
#[derive(Debug)]
pub struct RangeInclusive;

macro_rules! impl_rand_range {
    ($name:expr, $cmp:tt, $new:ident) => {
        fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
            let (lower, upper) = args_2::<Number, Number>($name, args, None, None)?;
            require($name, lower $cmp upper, || format!("{} {} {}", lower, stringify!($cmp), upper))?;
            if let (Some(a), Some(b)) = (lower.to::<u64>(), upper.to::<u64>()) {
                Ok(Compiled(C::RandUniformU64(rand_distr::Uniform::$new(a, b))))
            } else if let (Some(a), Some(b)) = (lower.to::<i64>(), upper.to::<i64>()) {
                Ok(Compiled(C::RandUniformI64(rand_distr::Uniform::$new(a, b))))
            } else {
                Err(Error::IntegerOverflow(format!("{}({}, {})", $name, lower, upper)))
            }
        }
    }
}

impl Function for Range {
    impl_rand_range!("rand.range", <, new);
}

impl Function for RangeInclusive {
    impl_rand_range!("rand.range_inclusive", <=, new_inclusive);
}

//------------------------------------------------------------------------------

/// The `rand.uniform` SQL function.
#[derive(Debug)]
pub struct Uniform;

/// The `rand.uniform_inclusive` SQL function.
#[derive(Debug)]
pub struct UniformInclusive;

macro_rules! impl_rand_uniform {
    ($name:expr, $cmp:tt, $new:ident) => {
        fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
            let (lower, upper) = args_2::<f64, f64>($name, args, None, None)?;
            require($name, lower $cmp upper, || format!("{} {} {}", lower, stringify!($cmp), upper))?;
            Ok(Compiled(C::RandUniformF64(rand_distr::Uniform::$new(lower, upper))))
        }
    }
}

impl Function for Uniform {
    impl_rand_uniform!("rand.uniform", <, new);
}

impl Function for UniformInclusive {
    impl_rand_uniform!("rand.uniform_inclusive", <=, new_inclusive);
}

//------------------------------------------------------------------------------

/// The `rand.zipf` SQL function.
#[derive(Debug)]
pub struct Zipf;

impl Function for Zipf {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = "rand.zipf";
        let (count, exponent) = args_2(name, args, None, None)?;
        Ok(Compiled(C::RandZipf(ZipfDistribution::new(count, exponent).map_err(
            |_| Error::InvalidArguments {
                name,
                cause: format!("count ({}) and exponent ({}) must be positive", count, exponent),
            },
        )?)))
    }
}

//------------------------------------------------------------------------------

/// The `rand.log_normal` SQL function.
#[derive(Debug)]
pub struct LogNormal;

impl Function for LogNormal {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = "rand.log_normal";
        let (mean, std_dev) = args_2::<f64, f64>(name, args, None, None)?;
        let std_dev = std_dev.abs();
        Ok(Compiled(C::RandLogNormal(
            rand_distr::LogNormal::new(mean, std_dev).map_err(|NormalError::StdDevTooSmall| {
                Error::InvalidArguments {
                    name,
                    cause: format!("standard deviation ({}) must >= 0", std_dev),
                }
            })?,
        )))
    }
}

//------------------------------------------------------------------------------

/// The `rand.bool` SQL function.
#[derive(Debug)]
pub struct Bool;

impl Function for Bool {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = "rand.bool";
        let p = args_1(name, args, None)?;
        Ok(Compiled(C::RandBool(rand_distr::Bernoulli::new(p).map_err(
            |BernoulliError::InvalidProbability| Error::InvalidArguments {
                name,
                cause: format!("probability ({}) must be inside [0, 1]", p),
            },
        )?)))
    }
}

//------------------------------------------------------------------------------

/// The `rand.finite_f32` SQL function.
#[derive(Debug)]
pub struct FiniteF32;

/// The `rand.finite_f64` SQL function.
#[derive(Debug)]
pub struct FiniteF64;

/// The `rand.u31_timestamp` SQL function.
#[derive(Debug)]
pub struct U31Timestamp;

impl Function for FiniteF32 {
    fn compile(&self, _: &CompileContext, _: Vec<Value>) -> Result<Compiled, Error> {
        Ok(Compiled(C::RandFiniteF32(rand_distr::Uniform::new(0, 0xff00_0000))))
    }
}

impl Function for FiniteF64 {
    fn compile(&self, _: &CompileContext, _: Vec<Value>) -> Result<Compiled, Error> {
        Ok(Compiled(C::RandFiniteF64(rand_distr::Uniform::new(
            0,
            0xffe0_0000_0000_0000,
        ))))
    }
}

impl Function for U31Timestamp {
    fn compile(&self, _: &CompileContext, _: Vec<Value>) -> Result<Compiled, Error> {
        Ok(Compiled(C::RandU31Timestamp(rand_distr::Uniform::new(1, 0x8000_0000))))
    }
}

//------------------------------------------------------------------------------

/// The `rand.regex` SQL function.
#[derive(Debug)]
pub struct Regex;

impl Function for Regex {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = "rand.regex";
        let (regex, flags, max_repeat) =
            args_3::<String, String, u32>(name, args, None, Some("".to_owned()), Some(100))?;
        let generator = compile_regex_generator(&regex, &flags, max_repeat)?;
        Ok(Compiled(C::RandRegex(generator)))
    }
}

fn compile_regex_generator(regex: &str, flags: &str, max_repeat: u32) -> Result<rand_regex::Regex, Error> {
    let mut parser = regex_syntax::ParserBuilder::new();
    for flag in flags.chars() {
        match flag {
            'o' => parser.octal(true),
            'a' => parser.allow_invalid_utf8(true).unicode(false),
            'u' => parser.allow_invalid_utf8(false).unicode(true),
            'x' => parser.ignore_whitespace(true),
            'i' => parser.case_insensitive(true),
            'm' => parser.multi_line(true),
            's' => parser.dot_matches_new_line(true),
            'U' => parser.swap_greed(true),
            _ => return Err(Error::UnknownRegexFlag(flag)),
        };
    }

    let hir = parser.build().parse(regex).map_err(|source| Error::InvalidRegex {
        pattern: regex.to_owned(),
        source: source.into(),
    })?;
    rand_regex::Regex::with_hir(hir, max_repeat).map_err(|source| Error::InvalidRegex {
        pattern: regex.to_owned(),
        source,
    })
}
