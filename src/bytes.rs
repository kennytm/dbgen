//! Byte string.

use crate::number::Number;
use rand_regex::{EncodedString, Encoding};
use std::{
    cmp::Ordering,
    convert::TryFrom,
    fmt, io,
    ops::Range,
    str::{from_utf8, from_utf8_unchecked},
};

/// Describes how the input byte-string failed the UTF-8 encoding.
#[derive(Debug)]
pub struct TryIntoStringError(pub ByteString);

/// A string which potentially contains invalid UTF-8.
#[derive(Clone, Eq, Debug)]
pub struct ByteString {
    /// The raw bytes.
    bytes: Vec<u8>,
    /// The bytes are valid ASCII until this position, which encountered the
    /// first non-ASCII bytes. This means `bytes[..ascii_len]` is entirely ASCII.
    /// Recording the ASCII length speeds up computations which uses character
    /// indices.
    ascii_len: usize,
    /// Whether the entire string is UTF-8.
    is_utf8: bool,
}

impl Default for ByteString {
    fn default() -> Self {
        Self {
            bytes: Vec::new(),
            ascii_len: 0,
            is_utf8: true,
        }
    }
}

impl PartialEq for ByteString {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl PartialOrd for ByteString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.bytes.partial_cmp(&other.bytes)
    }
}

impl Ord for ByteString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bytes.cmp(&other.bytes)
    }
}

impl From<String> for ByteString {
    fn from(s: String) -> Self {
        Self {
            ascii_len: compute_ascii_len(s.as_bytes()),
            is_utf8: true,
            bytes: s.into_bytes(),
        }
    }
}

impl From<Vec<u8>> for ByteString {
    fn from(bytes: Vec<u8>) -> Self {
        let ascii_len = compute_ascii_len(&bytes);
        Self {
            ascii_len,
            is_utf8: from_utf8(&bytes[ascii_len..]).is_ok(),
            bytes,
        }
    }
}

impl From<EncodedString> for ByteString {
    fn from(es: EncodedString) -> Self {
        let encoding = es.encoding();
        let bytes: Vec<u8> = es.into();
        Self {
            ascii_len: if encoding == Encoding::Ascii {
                bytes.len()
            } else {
                compute_ascii_len(&bytes)
            },
            is_utf8: encoding <= Encoding::Utf8,
            bytes,
        }
    }
}

impl From<ByteString> for Vec<u8> {
    fn from(bytes: ByteString) -> Self {
        bytes.bytes
    }
}

impl TryFrom<ByteString> for String {
    type Error = TryIntoStringError;
    fn try_from(bytes: ByteString) -> Result<Self, Self::Error> {
        if bytes.is_utf8 {
            Ok(unsafe { Self::from_utf8_unchecked(bytes.bytes) })
        } else {
            Err(TryIntoStringError(bytes))
        }
    }
}

impl io::Write for ByteString {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend_bytes(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Write for ByteString {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.extend_str(s);
        Ok(())
    }
}

/// Whether the byte is a leading byte in UTF-8 (`0x00..=0x7F`, `0xC0..=0xFF`).
#[allow(clippy::cast_possible_wrap)] // the wrap is intentional.
fn is_utf8_leading_byte(b: u8) -> bool {
    (b as i8) >= -0x40
}

/// Computes the expected `ascii_len` from the bytes.
fn compute_ascii_len(bytes: &[u8]) -> usize {
    bytes.iter().position(|b| !b.is_ascii()).unwrap_or(bytes.len())
}

impl ByteString {
    /// Validates the invariant. Becomes no-op on release build.
    fn debug_validate(&self) {
        debug_assert_eq!(self.is_utf8, from_utf8(&self.bytes).is_ok(), "{:x?}", self.bytes);
        debug_assert_eq!(self.ascii_len, compute_ascii_len(&self.bytes), "{:x?}", self.bytes);
    }

    /// Gets the total byte length.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the byte string is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Returns the narrowest encoding of the entire byte string.
    pub fn encoding(&self) -> Encoding {
        if self.bytes.len() == self.ascii_len {
            Encoding::Ascii
        } else if self.is_utf8 {
            Encoding::Utf8
        } else {
            Encoding::Binary
        }
    }

    /// Gets the slice of bytes of this byte string.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Extracts ownership of the vector of bytes from this byte string.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Extends a string to the end of this byte string.
    pub fn extend_str(&mut self, s: &str) {
        if self.ascii_len == self.len() {
            self.ascii_len += compute_ascii_len(s.as_bytes());
        }
        self.bytes.extend_from_slice(s.as_bytes());
        self.debug_validate();
    }

    /// Formats a number a string and extends to the end of this byte string.
    pub fn extend_number(&mut self, n: &Number) {
        let s = n.to_string();
        if self.ascii_len == self.len() {
            self.ascii_len += s.len();
        }
        self.bytes.extend_from_slice(s.as_bytes());
        self.debug_validate();
    }

    /// Extends some bytes to the end of this byte string.
    pub fn extend_bytes(&mut self, b: &[u8]) {
        if self.ascii_len == self.len() {
            self.ascii_len += compute_ascii_len(b);
        }
        self.bytes.extend_from_slice(b);
        self.is_utf8 = from_utf8(&self.bytes[self.ascii_len..]).is_ok();
    }

    /// Extends another byte string instance to the end.
    pub fn extend_byte_string(&mut self, other: &Self) {
        if self.ascii_len == self.len() {
            self.ascii_len += other.ascii_len;
        }
        self.bytes.extend_from_slice(&other.bytes);
        self.is_utf8 = match (self.is_utf8, other.is_utf8) {
            (true, true) => true,
            (true, false) | (false, true) => false,
            (false, false) => from_utf8(&self.bytes[self.ascii_len..]).is_ok(),
        };
        self.debug_validate();
    }

    /// Gets the total number of code points, if the byte string is valid UTF-8.
    ///
    /// If the byte string is not valid UTF-8, this method counts the number of
    /// leading UTF-8 code units.
    pub fn char_len(&self) -> usize {
        self.ascii_len
            + self.bytes[self.ascii_len..]
                .iter()
                .filter(|b| is_utf8_leading_byte(**b))
                .count()
    }
    /// Clears the entire content of the byte string.
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.ascii_len = 0;
        self.is_utf8 = true;
        self.debug_validate();
    }

    /// Truncates the byte string to the given length.
    pub fn truncate(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }
        if len == 0 {
            self.clear();
            return;
        }

        if len <= self.ascii_len {
            self.ascii_len = len;
            self.is_utf8 = true;
        } else {
            self.is_utf8 = if self.is_utf8 {
                is_utf8_leading_byte(self.bytes[len])
            } else {
                from_utf8(&self.bytes[..len]).is_ok()
            };
        }
        self.bytes.truncate(len);
        self.debug_validate();
    }

    /// Drops the first `len` bytes from the byte string.
    pub fn drain_init(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        if len >= self.len() {
            self.clear();
            return;
        }

        self.bytes.drain(..len);

        if len < self.ascii_len {
            self.ascii_len -= len;
        } else {
            self.is_utf8 = if self.is_utf8 {
                is_utf8_leading_byte(self.bytes[0])
            } else {
                from_utf8(&self.bytes).is_ok()
            };
            self.ascii_len = compute_ascii_len(&self.bytes);
        }

        self.debug_validate();
    }

    /// Clamps the byte range by the input length.
    pub fn clamp_range(&self, r: Range<usize>) -> Range<usize> {
        let input_len = self.len();
        r.start.min(input_len)..r.end.min(input_len)
    }

    /// Translates a range of characters into range of bytes.
    ///
    /// If the input overflows the input length, it will be clamped so the ends
    /// never exceed `self.len()`.
    pub fn char_range(&self, r: Range<usize>) -> Range<usize> {
        if r.end <= self.ascii_len {
            // short-circuit if the range is entirely ASCII.
            return r;
        }

        let input_len = self.bytes.len();
        // `it` is an iterator of byte indices pointing to start of each
        // character appearing after `ascii_len`.
        let mut it = self.bytes[self.ascii_len..]
            .iter()
            .zip(self.ascii_len..)
            .filter_map(|(b, i)| is_utf8_leading_byte(*b).then_some(i))
            .fuse();

        let start;
        let end;
        if let Some(n) = r.start.checked_sub(self.ascii_len) {
            // this branch means `r.start > self.ascii_len`.
            start = it.nth(n).unwrap_or(input_len);
            // note that `it.nth(n)` positions `it` at index `n+1` after execution.
            // therefore, we want `it.nth(r.end - r.start - 1)` to read the end index.
            let range_len = r.len().checked_sub(1);
            end = range_len.map_or(start, |len| it.nth(len).unwrap_or(input_len));
        } else {
            start = r.start;
            end = it.nth(r.end - self.ascii_len).unwrap_or(input_len);
        }
        start..end
    }

    /// Replaces the substring at `range` by a `replacement` string.
    pub fn splice(&mut self, range: Range<usize>, replacement: ByteString) {
        let is_splice_ascii_into_ascii = range.end <= self.ascii_len && replacement.ascii_len == replacement.len();
        if is_splice_ascii_into_ascii {
            // splice a full ASCII string into middle of ASCII region.
            self.ascii_len = self.ascii_len - range.len() + replacement.ascii_len;
        } else if range.start <= self.ascii_len {
            // splice replacement ends with non-ASCII text, but it overlaps with ASCII region.
            self.ascii_len = range.start + replacement.ascii_len;
        }
        // in other cases the ASCII region is untouched, thus not changing ascii_len.

        let is_splice_utf8_into_utf8 = self.is_utf8 && replacement.is_utf8;
        if is_splice_utf8_into_utf8 && !is_splice_ascii_into_ascii {
            // splicing UTF-8 text just requires the range to be at character boundaries.
            let s = unsafe { from_utf8_unchecked(&self.bytes) };
            self.is_utf8 = s.is_char_boundary(range.start) && s.is_char_boundary(range.end);
        }
        // if not splicing UTF-8 into UTF-8, just recompute validity afterwards.

        self.bytes.splice(range, replacement.bytes);

        if !is_splice_utf8_into_utf8 {
            self.is_utf8 = from_utf8(&self.bytes[self.ascii_len..]).is_ok();
        }

        self.debug_validate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_str() {
        let test_cases: Vec<(ByteString, &str, Encoding, &[u8])> = vec![
            ("abc".to_owned().into(), "def", Encoding::Ascii, b"abcdef"),
            ("abc".to_owned().into(), "", Encoding::Ascii, b"abc"),
            (b"abc\xc2".to_vec().into(), "def", Encoding::Binary, b"abc\xc2def"),
            (b"abc\xc2".to_vec().into(), "", Encoding::Binary, b"abc\xc2"),
            (b"abc\x80".to_vec().into(), "def", Encoding::Binary, b"abc\x80def"),
            (b"abc\x80".to_vec().into(), "", Encoding::Binary, b"abc\x80"),
            (ByteString::default(), "def", Encoding::Ascii, b"def"),
            (ByteString::default(), "", Encoding::Ascii, b""),
            (b"\xc2".to_vec().into(), "def", Encoding::Binary, b"\xc2def"),
            (b"\xc2".to_vec().into(), "", Encoding::Binary, b"\xc2"),
            (b"\x80".to_vec().into(), "def", Encoding::Binary, b"\x80def"),
            (b"\x80".to_vec().into(), "", Encoding::Binary, b"\x80"),
            (b"\xc2\x80".to_vec().into(), "def", Encoding::Utf8, b"\xc2\x80def"),
            (b"\xc2\x80".to_vec().into(), "", Encoding::Utf8, b"\xc2\x80"),
        ];

        for (mut target, string, encoding, bytes) in test_cases {
            let mut target_clone = target.clone();
            let mut target_clone_2 = target.clone();
            let mut target_clone_3 = target.clone();

            target.extend_str(string);
            assert_eq!(target.encoding(), encoding);
            assert_eq!(target.as_bytes(), bytes);

            target_clone.extend_byte_string(&string.to_owned().into());
            assert_eq!(target_clone.encoding(), encoding);
            assert_eq!(target_clone.as_bytes(), bytes);

            {
                use std::io::Write;
                write!(&mut target_clone_2, "{}", string).unwrap();
            }
            assert_eq!(target_clone_2.encoding(), encoding);
            assert_eq!(target_clone_2.as_bytes(), bytes);

            {
                use std::fmt::Write;
                write!(&mut target_clone_3, "{}", string).unwrap();
            }
            assert_eq!(target_clone_3.encoding(), encoding);
            assert_eq!(target_clone_3.as_bytes(), bytes);
        }
    }

    #[test]
    fn test_push_bytes() {
        let test_cases: Vec<(ByteString, &[u8], Encoding, &[u8])> = vec![
            ("abc".to_owned().into(), b"def", Encoding::Ascii, b"abcdef"),
            ("abc".to_owned().into(), b"def\xc2", Encoding::Binary, b"abcdef\xc2"),
            ("abc".to_owned().into(), b"def\x80", Encoding::Binary, b"abcdef\x80"),
            ("abc".to_owned().into(), b"", Encoding::Ascii, b"abc"),
            ("abc".to_owned().into(), b"\xc2", Encoding::Binary, b"abc\xc2"),
            ("abc".to_owned().into(), b"\x80", Encoding::Binary, b"abc\x80"),
            (b"abc\xc2".to_vec().into(), b"def", Encoding::Binary, b"abc\xc2def"),
            (
                b"abc\xc2".to_vec().into(),
                b"def\xc2",
                Encoding::Binary,
                b"abc\xc2def\xc2",
            ),
            (
                b"abc\xc2".to_vec().into(),
                b"def\x80",
                Encoding::Binary,
                b"abc\xc2def\x80",
            ),
            (b"abc\xc2".to_vec().into(), b"", Encoding::Binary, b"abc\xc2"),
            (b"abc\xc2".to_vec().into(), b"\xc2", Encoding::Binary, b"abc\xc2\xc2"),
            (b"abc\xc2".to_vec().into(), b"\x80", Encoding::Utf8, b"abc\xc2\x80"),
            (b"abc\x80".to_vec().into(), b"def", Encoding::Binary, b"abc\x80def"),
            (
                b"abc\x80".to_vec().into(),
                b"def\xc2",
                Encoding::Binary,
                b"abc\x80def\xc2",
            ),
            (
                b"abc\x80".to_vec().into(),
                b"def\x80",
                Encoding::Binary,
                b"abc\x80def\x80",
            ),
            (b"abc\x80".to_vec().into(), b"", Encoding::Binary, b"abc\x80"),
            (b"abc\x80".to_vec().into(), b"\xc2", Encoding::Binary, b"abc\x80\xc2"),
            (b"abc\x80".to_vec().into(), b"\x80", Encoding::Binary, b"abc\x80\x80"),
            (ByteString::default(), b"def", Encoding::Ascii, b"def"),
            (ByteString::default(), b"def\xc2", Encoding::Binary, b"def\xc2"),
            (ByteString::default(), b"def\x80", Encoding::Binary, b"def\x80"),
            (ByteString::default(), b"", Encoding::Ascii, b""),
            (ByteString::default(), b"\xc2", Encoding::Binary, b"\xc2"),
            (ByteString::default(), b"\x80", Encoding::Binary, b"\x80"),
            (b"\xc2".to_vec().into(), b"def", Encoding::Binary, b"\xc2def"),
            (b"\xc2".to_vec().into(), b"def\xc2", Encoding::Binary, b"\xc2def\xc2"),
            (b"\xc2".to_vec().into(), b"def\x80", Encoding::Binary, b"\xc2def\x80"),
            (b"\xc2".to_vec().into(), b"", Encoding::Binary, b"\xc2"),
            (b"\xc2".to_vec().into(), b"\xc2", Encoding::Binary, b"\xc2\xc2"),
            (b"\xc2".to_vec().into(), b"\x80", Encoding::Utf8, b"\xc2\x80"),
            (b"\x80".to_vec().into(), b"def", Encoding::Binary, b"\x80def"),
            (b"\x80".to_vec().into(), b"def\xc2", Encoding::Binary, b"\x80def\xc2"),
            (b"\x80".to_vec().into(), b"def\x80", Encoding::Binary, b"\x80def\x80"),
            (b"\x80".to_vec().into(), b"", Encoding::Binary, b"\x80"),
            (b"\x80".to_vec().into(), b"\xc2", Encoding::Binary, b"\x80\xc2"),
            (b"\x80".to_vec().into(), b"\x80", Encoding::Binary, b"\x80\x80"),
        ];

        for (mut target, append, encoding, bytes) in test_cases {
            let mut target_clone = target.clone();
            target.extend_bytes(append);
            assert_eq!(target.encoding(), encoding);
            assert_eq!(target.as_bytes(), bytes);

            target_clone.extend_byte_string(&append.to_vec().into());
            assert_eq!(target_clone.encoding(), encoding);
            assert_eq!(target_clone.as_bytes(), bytes);
        }
    }

    #[test]
    fn test_truncate() {
        let test_cases: Vec<(ByteString, usize, Encoding, &[u8], Encoding, &[u8])> = vec![
            (
                "abc".to_owned().into(),
                2,
                Encoding::Ascii,
                b"ab",
                Encoding::Binary,
                b"ab\x80",
            ),
            (
                "abc".to_owned().into(),
                3,
                Encoding::Ascii,
                b"abc",
                Encoding::Binary,
                b"abc\x80",
            ),
            (
                "abc".to_owned().into(),
                0,
                Encoding::Ascii,
                b"",
                Encoding::Binary,
                b"\x80",
            ),
            (
                b"abc\xc2\x80".to_vec().into(),
                4,
                Encoding::Binary,
                b"abc\xc2",
                Encoding::Utf8,
                b"abc\xc2\x80",
            ),
            (
                b"abc\xf0\x80".to_vec().into(),
                4,
                Encoding::Binary,
                b"abc\xf0",
                Encoding::Binary,
                b"abc\xf0\x80",
            ),
            (
                b"abc\x80\x80".to_vec().into(),
                4,
                Encoding::Binary,
                b"abc\x80",
                Encoding::Binary,
                b"abc\x80\x80",
            ),
        ];

        for (mut target, trunc_len, encoding, bytes, encoding_after, bytes_after) in test_cases {
            target.truncate(trunc_len);
            assert_eq!(target.encoding(), encoding);
            assert_eq!(target.as_bytes(), bytes);

            target.extend_bytes(b"\x80");
            assert_eq!(target.encoding(), encoding_after);
            assert_eq!(target.as_bytes(), bytes_after);
        }
    }

    #[test]
    fn test_drain_init() {
        let test_cases: Vec<(ByteString, usize, Encoding, &[u8])> = vec![
            ("abc".to_owned().into(), 2, Encoding::Ascii, b"c"),
            ("abc".to_owned().into(), 3, Encoding::Ascii, b""),
            (b"\xc2\x80".to_vec().into(), 0, Encoding::Utf8, b"\xc2\x80"),
            (b"\xc2\x80".to_vec().into(), 1, Encoding::Binary, b"\x80"),
            (b"\xc2\x80".to_vec().into(), 2, Encoding::Ascii, b""),
            (b"\x80\xc2".to_vec().into(), 1, Encoding::Binary, b"\xc2"),
            (b"\x80\xc2\x80".to_vec().into(), 1, Encoding::Utf8, b"\xc2\x80"),
        ];

        for (mut target, drain_len, encoding, bytes) in test_cases {
            target.drain_init(drain_len);
            assert_eq!(target.encoding(), encoding);
            assert_eq!(target.as_bytes(), bytes);
        }
    }

    #[test]
    fn test_splice() {
        let test_cases: Vec<(ByteString, Range<usize>, ByteString, Encoding, &[u8])> = vec![
            (
                "abcdef".to_owned().into(),
                2..4,
                "XYZ".to_owned().into(),
                Encoding::Ascii,
                b"abXYZef",
            ),
            (
                "ghíj́ḱ".to_owned().into(),
                1..4,
                "lmnóṕ".to_owned().into(),
                Encoding::Utf8,
                "glmnóṕj́ḱ".as_bytes(),
            ),
            (
                b"abc\xf1\xf2\xf3".to_vec().into(),
                3..3,
                "d".to_owned().into(),
                Encoding::Binary,
                b"abcd\xf1\xf2\xf3",
            ),
            (
                b"abc\xf1\xf2\xf3".to_vec().into(),
                4..4,
                "d".to_owned().into(),
                Encoding::Binary,
                b"abc\xf1d\xf2\xf3",
            ),
            (
                b"abc\xf1\xf2\xf3".to_vec().into(),
                3..6,
                "d".to_owned().into(),
                Encoding::Ascii,
                b"abcd",
            ),
            (
                b"\xc2\x80\xc3\x81".to_vec().into(),
                2..2,
                b"\xc4\x82".to_vec().into(),
                Encoding::Utf8,
                b"\xc2\x80\xc4\x82\xc3\x81",
            ),
            (
                b"\xc2\x80\xc3\x81".to_vec().into(),
                1..3,
                b"\xc4\x82".to_vec().into(),
                Encoding::Binary,
                b"\xc2\xc4\x82\x81",
            ),
            (
                b"\xc2\x80\xc3\x81".to_vec().into(),
                1..1,
                b"\x82\xc4".to_vec().into(),
                Encoding::Utf8,
                b"\xc2\x82\xc4\x80\xc3\x81",
            ),
        ];

        for (mut target, range, replacement, encoding, bytes) in test_cases {
            target.splice(range, replacement);
            assert_eq!(target.encoding(), encoding);
            assert_eq!(target.as_bytes(), bytes);
        }
    }
}
