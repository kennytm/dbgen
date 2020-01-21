//! String functions.

use super::{args_1, args_3, args_4, Function};
use crate::{
    error::Error,
    eval::{CompileContext, Compiled, C},
    value::Value,
};
use std::{convert::TryInto, isize};

//------------------------------------------------------------------------------

/// The unit used to index a (byte) string.
#[derive(Debug, Copy, Clone)]
pub enum Unit {
    /// Index the string using characters (code points).
    Characters,
    /// Index the string using bytes (code units).
    Octets,
}

/// Whether the byte is a leading byte in UTF-8 (`0x00..=0x7F`, `0xC0..=0xFF`).
#[allow(clippy::cast_possible_wrap)] // the wrap is intentional.
fn is_utf8_leading_byte(b: u8) -> bool {
    (b as i8) >= -0x40
}

impl Unit {
    /// Extracts the arguments for the `substring` SQL functions.
    fn parse_sql_range(self, input: &[u8], mut start: isize, length: isize) -> (usize, usize) {
        // first convert SQL indices into Rust indices.
        start -= 1;
        let end = start.saturating_add(length.max(0));
        let start = start.try_into().unwrap_or(0_usize);
        let end = end.try_into().unwrap_or(start);

        // Translate character index into byte index
        match self {
            Self::Octets => (start.min(input.len()), end.min(input.len())),
            Self::Characters => {
                let len = (end - start).checked_sub(1);
                let mut it = input
                    .iter()
                    .enumerate()
                    .filter_map(|(i, b)| if is_utf8_leading_byte(*b) { Some(i) } else { None })
                    .fuse();
                #[allow(clippy::or_fun_call)]
                {
                    let byte_start = it.nth(start).unwrap_or(input.len());
                    let byte_end = len.map_or(byte_start, |len| it.nth(len).unwrap_or(input.len()));
                    (byte_start, byte_end)
                }
            }
        }
    }

    /// Computes the length of the input using this unit.
    fn length_of(self, input: &[u8]) -> usize {
        match self {
            Self::Octets => input.len(),
            Self::Characters => input.iter().filter(|b| is_utf8_leading_byte(**b)).count(),
        }
    }
}

#[test]
fn test_parse_sql_range() {
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 1, isize::MAX), (0, 9));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 0, isize::MAX), (0, 9));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", -100, isize::MAX), (0, 9));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 3, isize::MAX), (2, 9));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 9, isize::MAX), (8, 9));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 100, isize::MAX), (9, 9));

    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 1, 1), (0, 1));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 3, 5), (2, 7));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 5, 99), (4, 9));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 7, 0), (6, 6));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 9, -99), (8, 8));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 0, 5), (0, 4));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", -70, 77), (0, 6));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 70, 77), (9, 9));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", -70, -77), (0, 0));
    assert_eq!(Unit::Octets.parse_sql_range(b"123456789", 70, -77), (9, 9));

    let b = "ßs≠🥰".as_bytes();
    assert_eq!(Unit::Characters.parse_sql_range(b, 1, isize::MAX), (0, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, 2, isize::MAX), (2, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, 3, isize::MAX), (3, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, 4, isize::MAX), (6, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, 5, isize::MAX), (10, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, 0, isize::MAX), (0, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, 100, isize::MAX), (10, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, -100, isize::MAX), (0, 10));

    assert_eq!(Unit::Characters.parse_sql_range(b, 1, 1), (0, 2));
    assert_eq!(Unit::Characters.parse_sql_range(b, 2, 2), (2, 6));
    assert_eq!(Unit::Characters.parse_sql_range(b, 3, 99), (3, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, 4, 0), (6, 6));
    assert_eq!(Unit::Characters.parse_sql_range(b, 5, -99), (10, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, -70, 77), (0, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, 70, 77), (10, 10));
    assert_eq!(Unit::Characters.parse_sql_range(b, -70, -77), (0, 0));
    assert_eq!(Unit::Characters.parse_sql_range(b, 70, -77), (10, 10));
}

//------------------------------------------------------------------------------

/// The `substring` SQL function.
#[derive(Debug)]
pub struct Substring(
    /// The string unit used by the function.
    pub Unit,
);

impl Function for Substring {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = "substring";
        let (mut input, start, length) = args_3::<Vec<u8>, isize, Option<isize>>(name, args, None, None, Some(None))?;
        let (start, end) = self.0.parse_sql_range(&input, start, length.unwrap_or(0));
        if length.is_some() {
            input.truncate(end);
        }
        if start > 0 {
            input.drain(..start);
        }
        Ok(Compiled(C::Constant(input.into())))
    }
}

//------------------------------------------------------------------------------

/// The `char_length` and `octet_length` SQL functions.
#[derive(Debug)]
pub struct Length(
    /// The string unit used by the function.
    pub Unit,
);

impl Function for Length {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = match self.0 {
            Unit::Characters => "char_length",
            Unit::Octets => "octet_length",
        };
        let input = args_1::<Vec<u8>>(name, args, None)?;
        Ok(Compiled(C::Constant(self.0.length_of(&input).into())))
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
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let name = "overlay";
        let (mut input, placing, start, length) =
            args_4::<Vec<u8>, Vec<u8>, isize, Option<isize>>(name, args, None, None, None, Some(None))?;
        #[allow(clippy::cast_possible_wrap)] // length will never > isize::MAX.
        let length = length.unwrap_or_else(|| self.0.length_of(&placing) as isize);
        let (start, end) = self.0.parse_sql_range(&input, start, length);
        input.splice(start..end, placing);
        Ok(Compiled(C::Constant(input.into())))
    }
}

//------------------------------------------------------------------------------

/// The string concatenation (`||`) SQL function.
#[derive(Debug)]
pub struct Concat;

impl Function for Concat {
    fn compile(&self, _: &CompileContext, args: Vec<Value>) -> Result<Compiled, Error> {
        let result = Value::sql_concat(args.into_iter())?;
        Ok(Compiled(C::Constant(result)))
    }
}
