//! String functions.

use super::{args_1, args_3, args_4, Arguments, Function};
use crate::{
    bytes::ByteString,
    error::Error,
    eval::{CompileContext, C},
    span::{Span, SpanExt, S},
    value::Value,
};
use std::{convert::TryInto, isize, ops::Range};

//------------------------------------------------------------------------------

/// Converts the SQL "start, length" representation of a range of characters to
/// Rust's range representation:
///
///  * the index is converted from 1-based to 0-based.
///  * negative length is treated as the same as zero length.
///  * the range is clamped within `0..=isize::MAX`.
fn sql_start_length_to_range(start: isize, length: isize) -> Range<usize> {
    let start = start - 1;
    let end = start.saturating_add(length.max(0));
    let start = start.try_into().unwrap_or(0_usize);
    let end = end.try_into().unwrap_or(start);
    start..end
}

/// The unit used to index a (byte) string.
#[derive(Debug, Copy, Clone)]
pub enum Unit {
    /// Index the string using characters (code points).
    Characters,
    /// Index the string using bytes (code units).
    Octets,
}

impl Unit {
    fn parse_sql_range(self, input: &ByteString, start: isize, length: isize) -> Range<usize> {
        let range = sql_start_length_to_range(start, length);
        match self {
            Self::Octets => input.clamp_range(range),
            Self::Characters => input.char_range(range),
        }
    }

    fn length_of(self, input: &ByteString) -> usize {
        match self {
            Self::Octets => input.len(),
            Self::Characters => input.char_len(),
        }
    }
}

#[test]
fn test_parse_sql_range() {
    let b = ByteString::from("123456789".to_owned());
    assert_eq!(Unit::Octets.parse_sql_range(&b, 1, isize::MAX), 0..9);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 0, isize::MAX), 0..9);
    assert_eq!(Unit::Octets.parse_sql_range(&b, -100, isize::MAX), 0..9);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 3, isize::MAX), 2..9);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 9, isize::MAX), 8..9);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 100, isize::MAX), 9..9);

    assert_eq!(Unit::Octets.parse_sql_range(&b, 1, 1), 0..1);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 3, 5), 2..7);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 5, 99), 4..9);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 7, 0), 6..6);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 9, -99), 8..8);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 0, 5), 0..4);
    assert_eq!(Unit::Octets.parse_sql_range(&b, -70, 77), 0..6);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 70, 77), 9..9);
    assert_eq!(Unit::Octets.parse_sql_range(&b, -70, -77), 0..0);
    assert_eq!(Unit::Octets.parse_sql_range(&b, 70, -77), 9..9);

    let b = ByteString::from("ßs≠🥰".to_owned());
    assert_eq!(Unit::Characters.parse_sql_range(&b, 1, isize::MAX), 0..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 2, isize::MAX), 2..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 3, isize::MAX), 3..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 4, isize::MAX), 6..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 5, isize::MAX), 10..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 0, isize::MAX), 0..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 100, isize::MAX), 10..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, -100, isize::MAX), 0..10);

    assert_eq!(Unit::Characters.parse_sql_range(&b, 1, 1), 0..2);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 2, 2), 2..6);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 3, 99), 3..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 4, 0), 6..6);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 5, -99), 10..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, -70, 77), 0..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 70, 77), 10..10);
    assert_eq!(Unit::Characters.parse_sql_range(&b, -70, -77), 0..0);
    assert_eq!(Unit::Characters.parse_sql_range(&b, 70, -77), 10..10);
}

//------------------------------------------------------------------------------

/// The `substring` SQL function.
#[derive(Debug)]
pub struct Substring(
    /// The string unit used by the function.
    pub Unit,
);

impl Function for Substring {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let (mut input, start, length) = args_3(span, args, None, None, Some(None))?;
        let range = self.0.parse_sql_range(&input, start, length.unwrap_or(0));
        if length.is_some() {
            input.truncate(range.end);
        }
        if range.start > 0 {
            input.drain_init(range.start);
        }
        Ok(C::Constant(input.into()))
    }
}

//------------------------------------------------------------------------------

/// The `char_length` SQL function.
#[derive(Debug)]
pub struct CharLength;

/// The `octet_length` SQL function.
#[derive(Debug)]
pub struct OctetLength;

impl Function for CharLength {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let input = args_1::<ByteString>(span, args, None)?;
        Ok(C::Constant(input.char_len().into()))
    }
}

impl Function for OctetLength {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let input = args_1::<ByteString>(span, args, None)?;
        Ok(C::Constant(input.len().into()))
    }
}

//------------------------------------------------------------------------------

/// The `overlay` SQL function.
#[derive(Debug)]
pub struct Overlay(
    /// The string unit used by the function.
    pub Unit,
);

impl Function for Overlay {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let (mut input, placing, start, length) = args_4(span, args, None, None, None, Some(None))?;
        #[allow(clippy::cast_possible_wrap)] // length will never > isize::MAX.
        let length = length.unwrap_or_else(|| self.0.length_of(&placing) as isize);
        let range = self.0.parse_sql_range(&input, start, length);
        input.splice(range, placing);
        Ok(C::Constant(input.into()))
    }
}

//------------------------------------------------------------------------------

/// The string concatenation (`||`) SQL function.
#[derive(Debug)]
pub struct Concat;

impl Function for Concat {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        match Value::sql_concat(args.iter().map(|arg| &arg.inner)) {
            Ok(result) => Ok(C::Constant(result)),
            Err(e) => Err(e.span(span)),
        }
    }
}
