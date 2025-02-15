//! Random generator functions.

use super::{args_1, args_2, args_3, require, Arguments, Function};
use crate::{
    array::Array,
    error::Error,
    eval::{CompileContext, C},
    number::Number,
    span::{ResultExt, Span, SpanExt, S},
};
use std::convert::TryFrom as _;
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
        fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
            let (lower, upper) = args_2::<Number, Number>(span, args, None, None)?;
            require(span, lower $cmp upper, || format!("assertion failed: {} {} {}", lower, stringify!($cmp), upper))?;
            if let (Ok(a), Ok(b)) = (u64::try_from(lower), u64::try_from(upper)) {
                Ok(C::RandUniformU64(rand_distr::Uniform::$new(a, b)))
            } else if let (Ok(a), Ok(b)) = (i64::try_from(lower), i64::try_from(upper)) {
                Ok(C::RandUniformI64(rand_distr::Uniform::$new(a, b)))
            } else {
                Err(Error::IntegerOverflow(format!("{}({}, {})", $name, lower, upper)).span(span))
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
    ($cmp:tt, $new:ident) => {
        fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
            let (lower, upper) = args_2::<f64, f64>(span, args, None, None)?;
            require(span, lower $cmp upper, || format!("assertion failed: {} {} {}", lower, stringify!($cmp), upper))?;
            Ok(C::RandUniformF64(rand_distr::Uniform::$new(lower, upper)))
        }
    }
}

impl Function for Uniform {
    impl_rand_uniform!(<, new);
}

impl Function for UniformInclusive {
    impl_rand_uniform!(<=, new_inclusive);
}

//------------------------------------------------------------------------------

/// The `rand.zipf` SQL function.
#[derive(Debug)]
pub struct Zipf;

impl Function for Zipf {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let (count, exponent) = args_2(span, args, None, None)?;
        Ok(C::RandZipf(ZipfDistribution::new(count, exponent).map_err(|()| {
            Error::InvalidArguments(format!("count ({count}) and exponent ({exponent}) must be positive")).span(span)
        })?))
    }
}

//------------------------------------------------------------------------------

/// The `rand.log_normal` SQL function.
#[derive(Debug)]
pub struct LogNormal;

impl Function for LogNormal {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let (mean, std_dev) = args_2::<f64, f64>(span, args, None, None)?;
        let std_dev = std_dev.abs();
        Ok(C::RandLogNormal(rand_distr::LogNormal::new(mean, std_dev).map_err(
            |e| Error::InvalidArguments(format!("standard deviation ({std_dev}) {e}")).span(span),
        )?))
    }
}

//------------------------------------------------------------------------------

/// The `rand.bool` SQL function.
#[derive(Debug)]
pub struct Bool;

impl Function for Bool {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let p = args_1(span, args, None)?;
        Ok(C::RandBool(rand_distr::Bernoulli::new(p).map_err(|e| {
            Error::InvalidArguments(format!("probability ({p}) {e}")).span(span)
        })?))
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

/// The `rand.uuid` SQL function.
#[derive(Debug)]
pub struct Uuid;

impl Function for FiniteF32 {
    fn compile(&self, _: &CompileContext, _: Span, _: Arguments) -> Result<C, S<Error>> {
        Ok(C::RandFiniteF32(rand_distr::Uniform::new(0, 0xff00_0000)))
    }
}

impl Function for FiniteF64 {
    fn compile(&self, _: &CompileContext, _: Span, _: Arguments) -> Result<C, S<Error>> {
        Ok(C::RandFiniteF64(rand_distr::Uniform::new(0, 0xffe0_0000_0000_0000)))
    }
}

impl Function for U31Timestamp {
    fn compile(&self, _: &CompileContext, _: Span, _: Arguments) -> Result<C, S<Error>> {
        Ok(C::RandU31Timestamp(rand_distr::Uniform::new(1, 0x8000_0000)))
    }
}

impl Function for Uuid {
    fn compile(&self, _: &CompileContext, _: Span, _: Arguments) -> Result<C, S<Error>> {
        Ok(C::RandUuid)
    }
}

//------------------------------------------------------------------------------

/// The `rand.regex` SQL function.
#[derive(Debug)]
pub struct Regex;

impl Function for Regex {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let (regex, flags, max_repeat) = args_3::<String, String, _>(span, args, None, Some(String::new()), Some(100))?;
        let generator = compile_regex_generator(&regex, &flags, max_repeat).span_err(span)?;
        Ok(C::RandRegex(generator))
    }
}

fn compile_regex_generator(regex: &str, flags: &str, max_repeat: u32) -> Result<rand_regex::Regex, Error> {
    let mut parser = regex_syntax::ParserBuilder::new();
    for flag in flags.chars() {
        match flag {
            'o' => parser.octal(true),
            'a' => parser.utf8(false).unicode(false),
            'u' => parser.utf8(true).unicode(true),
            'x' => parser.ignore_whitespace(true),
            'i' => parser.case_insensitive(true),
            'm' => parser.multi_line(true),
            's' => parser.dot_matches_new_line(true),
            'U' => parser.swap_greed(true),
            _ => return Err(Error::UnknownRegexFlag(flag)),
        };
    }

    let hir = parser.build().parse(regex)?;
    Ok(rand_regex::Regex::with_hir(hir, max_repeat)?)
}

//------------------------------------------------------------------------------

/// The `rand.weighted` SQL function.
#[derive(Debug)]
pub struct Weighted;

impl Function for Weighted {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let weights = args_1::<Array>(span, args, None)?
            .iter()
            .map(|v| v.try_into())
            .collect::<Result<_, _>>()
            .span_err(span)?;
        Ok(C::RandWeighted(
            rand_distr::weighted_alias::WeightedAliasIndex::new(weights)
                .map_err(|e| Error::InvalidArguments(e.to_string()).span(span))?,
        ))
    }
}

//------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn test_compile_checks() {
        struct TestCase {
            name: &'static str,
            function: &'static dyn Function,
            args: Vec<Value>,
            error: &'static str,
        }

        fn array_of_numbers(values: &[i64]) -> Value {
            Value::Array(Array::from_values(values.iter().map(|v| Value::Number((*v).into()))))
        }

        let test_cases = vec![
            TestCase {
                name: "rand.weighted expected an array",
                function: &Weighted,
                args: vec![30.into()],
                error: "cannot convert 30 into array",
            },
            TestCase {
                name: "rand.weighted must not be an empty array",
                function: &Weighted,
                args: vec![array_of_numbers(&[])],
                error: "No weights provided in distribution",
            },
            TestCase {
                name: "rand.weighted must not have negative weight",
                function: &Weighted,
                args: vec![array_of_numbers(&[55, -5])],
                error: "A weight is invalid in distribution",
            },
        ];

        let ctx = CompileContext::new(0);
        let span = Span::default();
        for tc in test_cases {
            let args = tc.args.into_iter().map(|v| v.no_span()).collect();
            let actual_error = tc
                .function
                .compile(&ctx, span, args)
                .expect_err(tc.name)
                .inner
                .to_string();
            assert_eq!(tc.error, actual_error, "{}", tc.name);
        }
    }
}
