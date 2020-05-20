//! Byte string.

use std::{
    cmp::Ordering,
    convert::TryFrom,
    fmt, io,
    ops::{Add, Range},
    str::{from_utf8, Utf8Error},
    string::FromUtf8Error,
};

/// Describes how the input byte-string failed the UTF-8 encoding.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TryIntoStringError {
    /// The bytes are valid UTF-8 until this position, which encountered the
    /// first non-UTF-8 bytes.
    valid_len: usize,
    /// Whether the error is caused by a partial UTF-8 code point at the end of
    /// the sequence, and can be corrected by appending continuation bytes.
    is_partial: bool,
}

impl From<Utf8Error> for TryIntoStringError {
    fn from(error: Utf8Error) -> Self {
        Self {
            valid_len: error.valid_up_to(),
            is_partial: error.error_len().is_none(),
        }
    }
}

impl Add<usize> for TryIntoStringError {
    type Output = Self;
    fn add(self, extra_len: usize) -> Self {
        Self {
            valid_len: self.valid_len + extra_len,
            is_partial: self.is_partial,
        }
    }
}

/// A string which potentially contains invalid UTF-8.
#[derive(Clone, Eq, Debug, Default)]
pub struct ByteString {
    /// The raw bytes.
    bytes: Vec<u8>,
    /// If the bytes are not valid UTF-8, this field contains information about
    /// the failure.
    error: Option<TryIntoStringError>,
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
            bytes: s.into_bytes(),
            error: None,
        }
    }
}

impl From<Vec<u8>> for ByteString {
    fn from(bytes: Vec<u8>) -> Self {
        String::from_utf8(bytes).into()
    }
}

impl From<FromUtf8Error> for ByteString {
    fn from(error: FromUtf8Error) -> Self {
        Self {
            error: Some(error.utf8_error().into()),
            bytes: error.into_bytes(),
        }
    }
}

impl From<Result<String, FromUtf8Error>> for ByteString {
    fn from(result: Result<String, FromUtf8Error>) -> Self {
        match result {
            Ok(s) => s.into(),
            Err(e) => e.into(),
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
        if let Some(e) = bytes.error {
            Err(e)
        } else {
            Ok(unsafe { Self::from_utf8_unchecked(bytes.bytes) })
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

impl ByteString {
    /// Validates the invariant. Becomes no-op on release build.
    fn debug_validate(&self) {
        debug_assert_eq!(self.error, from_utf8(&self.bytes).err().map(TryIntoStringError::from));
    }

    /// Gets the total byte length.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the byte string is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Gets the length where the byte string is valid UTF-8 up to this point.
    fn valid_len(&self) -> usize {
        self.error.map_or(self.bytes.len(), |e| e.valid_len)
    }

    /// Computes the length where `self.bytes[..end]` is valid UTF-8 up to this
    /// point. It is expected `end <= self.len()`.
    fn valid_len_before(&self, end: usize) -> usize {
        let valid_len = self.valid_len();
        if end >= valid_len {
            return valid_len;
        }
        // end < valid_len <= self.len() at this point.
        self.bytes[..=end]
            .iter()
            .rposition(|b| is_utf8_leading_byte(*b))
            .unwrap_or(0)
    }

    /// Recomputes the `error` field, provided the bytes up to `old_valid_len`
    /// are valid UTF-8.
    fn recompute_error(&mut self, old_valid_len: usize) {
        self.error = from_utf8(&self.bytes[old_valid_len..])
            .err()
            .map(|e| TryIntoStringError::from(e) + old_valid_len)
    }

    /// Gets whether the byte string consists of valid UTF-8 content.
    pub fn is_utf8(&self) -> bool {
        self.error.is_none()
    }

    /// Gets whether the byte string contained non-UTF-8 content.
    pub fn is_binary(&self) -> bool {
        self.error.is_some()
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
        if s.is_empty() {
            return;
        }
        if let Some(error) = &mut self.error {
            error.is_partial = false;
        }
        self.bytes.extend_from_slice(s.as_bytes());
        self.debug_validate();
    }

    /// Extends some bytes to the end of this byte string.
    pub fn extend_bytes(&mut self, b: &[u8]) {
        if b.is_empty() {
            return;
        }

        let old_valid_len = self.valid_len();
        self.bytes.extend_from_slice(b);
        if self.error.map_or(false, |e| !e.is_partial) {
            return;
        }

        self.recompute_error(old_valid_len);
        self.debug_validate();
    }

    /// Extends another byte string instance to the end.
    pub fn extend_byte_string(&mut self, other: &Self) {
        if other.is_empty() {
            return;
        }

        let old_len = self.bytes.len();
        self.bytes.extend_from_slice(&other.bytes);
        match (self.error, other.error) {
            // Do nothing if we append a string to a string, or to binary which cannot be corrected.
            (None, None) | (Some(TryIntoStringError { is_partial: false, .. }), _) => {}
            // If we append binary to a string, inherit the binary error info.
            (None, Some(oe)) => self.error = Some(oe + old_len),
            // If we append start-with-binary to partial binary, recompute the info.
            (
                Some(TryIntoStringError {
                    is_partial: true,
                    valid_len,
                }),
                Some(TryIntoStringError { valid_len: 0, .. }),
            ) => self.recompute_error(valid_len),
            // If we append start-with-string to partial binary, make the existing error impartial.
            (Some(TryIntoStringError { valid_len, .. }), _) => {
                self.error = Some(TryIntoStringError {
                    valid_len,
                    is_partial: false,
                })
            }
        };

        self.debug_validate();
    }

    /// Gets the total number of code points, if the byte string is valid UTF-8.
    ///
    /// If the byte string is not valid UTF-8, this method counts the number of
    /// leading UTF-8 code units.
    pub fn char_len(&self) -> usize {
        self.bytes.iter().filter(|b| is_utf8_leading_byte(**b)).count()
    }

    /// Truncates the byte string to the given length.
    pub fn truncate(&mut self, len: usize) {
        if len == self.len() {
            return;
        }

        let new_valid_len = self.valid_len_before(len);
        self.bytes.truncate(len);
        if new_valid_len == len {
            self.error = None;
        } else {
            self.recompute_error(new_valid_len);
        }
        self.debug_validate();
    }

    /// Drops the first few bytes from the byte string.
    pub fn drain_init(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let valid_len = self.valid_len();
        self.bytes.drain(..len);
        if len < valid_len && is_utf8_leading_byte(self.bytes[0]) {
            if let Some(e) = &mut self.error {
                e.valid_len = valid_len - len;
            }
        } else {
            self.recompute_error(0);
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
        let range_len = (r.end - r.start).checked_sub(1);
        let input_len = self.len();
        let mut it = self
            .bytes
            .iter()
            .enumerate()
            .filter_map(|(i, b)| if is_utf8_leading_byte(*b) { Some(i) } else { None })
            .fuse();
        let start = it.nth(r.start).unwrap_or(input_len);
        let end = range_len.map_or(start, |len| it.nth(len).unwrap_or(input_len));
        start..end
    }

    /// Replaces the bytes in the given range by the replacement byte string.
    pub fn splice(&mut self, range: Range<usize>, replacement: ByteString) {
        let mut valid_len = self.valid_len_before(range.start);
        if valid_len == range.start {
            valid_len += replacement.valid_len();
        }

        self.bytes.splice(range, replacement.bytes);
        self.recompute_error(valid_len);

        self.debug_validate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_str() {
        let test_cases: Vec<(ByteString, &str, bool, &[u8])> = vec![
            ("abc".to_owned().into(), "def", true, b"abcdef"),
            ("abc".to_owned().into(), "", true, b"abc"),
            (b"abc\xc2".to_vec().into(), "def", false, b"abc\xc2def"),
            (b"abc\xc2".to_vec().into(), "", false, b"abc\xc2"),
            (b"abc\x80".to_vec().into(), "def", false, b"abc\x80def"),
            (b"abc\x80".to_vec().into(), "", false, b"abc\x80"),
            (ByteString::default(), "def", true, b"def"),
            (ByteString::default(), "", true, b""),
            (b"\xc2".to_vec().into(), "def", false, b"\xc2def"),
            (b"\xc2".to_vec().into(), "", false, b"\xc2"),
            (b"\x80".to_vec().into(), "def", false, b"\x80def"),
            (b"\x80".to_vec().into(), "", false, b"\x80"),
        ];

        for (mut target, string, is_utf8, bytes) in test_cases {
            let mut target_clone = target.clone();
            let mut target_clone_2 = target.clone();
            let mut target_clone_3 = target.clone();

            target.extend_str(string);
            assert_eq!(target.is_utf8(), is_utf8);
            assert_eq!(target.as_bytes(), bytes);

            target_clone.extend_byte_string(&string.to_owned().into());
            assert_eq!(target_clone.is_utf8(), is_utf8);
            assert_eq!(target_clone.as_bytes(), bytes);

            {
                use std::io::Write;
                write!(&mut target_clone_2, "{}", string).unwrap();
            }
            assert_eq!(target_clone_2.is_utf8(), is_utf8);
            assert_eq!(target_clone_2.as_bytes(), bytes);

            {
                use std::fmt::Write;
                write!(&mut target_clone_3, "{}", string).unwrap();
            }
            assert_eq!(target_clone_3.is_utf8(), is_utf8);
            assert_eq!(target_clone_3.as_bytes(), bytes);
        }
    }

    #[test]
    fn test_push_bytes() {
        let test_cases: Vec<(ByteString, &[u8], bool, &[u8])> = vec![
            ("abc".to_owned().into(), b"def", true, b"abcdef"),
            ("abc".to_owned().into(), b"def\xc2", false, b"abcdef\xc2"),
            ("abc".to_owned().into(), b"def\x80", false, b"abcdef\x80"),
            ("abc".to_owned().into(), b"", true, b"abc"),
            ("abc".to_owned().into(), b"\xc2", false, b"abc\xc2"),
            ("abc".to_owned().into(), b"\x80", false, b"abc\x80"),
            (b"abc\xc2".to_vec().into(), b"def", false, b"abc\xc2def"),
            (b"abc\xc2".to_vec().into(), b"def\xc2", false, b"abc\xc2def\xc2"),
            (b"abc\xc2".to_vec().into(), b"def\x80", false, b"abc\xc2def\x80"),
            (b"abc\xc2".to_vec().into(), b"", false, b"abc\xc2"),
            (b"abc\xc2".to_vec().into(), b"\xc2", false, b"abc\xc2\xc2"),
            (b"abc\xc2".to_vec().into(), b"\x80", true, b"abc\xc2\x80"),
            (b"abc\x80".to_vec().into(), b"def", false, b"abc\x80def"),
            (b"abc\x80".to_vec().into(), b"def\xc2", false, b"abc\x80def\xc2"),
            (b"abc\x80".to_vec().into(), b"def\x80", false, b"abc\x80def\x80"),
            (b"abc\x80".to_vec().into(), b"", false, b"abc\x80"),
            (b"abc\x80".to_vec().into(), b"\xc2", false, b"abc\x80\xc2"),
            (b"abc\x80".to_vec().into(), b"\x80", false, b"abc\x80\x80"),
            (ByteString::default(), b"def", true, b"def"),
            (ByteString::default(), b"def\xc2", false, b"def\xc2"),
            (ByteString::default(), b"def\x80", false, b"def\x80"),
            (ByteString::default(), b"", true, b""),
            (ByteString::default(), b"\xc2", false, b"\xc2"),
            (ByteString::default(), b"\x80", false, b"\x80"),
            (b"\xc2".to_vec().into(), b"def", false, b"\xc2def"),
            (b"\xc2".to_vec().into(), b"def\xc2", false, b"\xc2def\xc2"),
            (b"\xc2".to_vec().into(), b"def\x80", false, b"\xc2def\x80"),
            (b"\xc2".to_vec().into(), b"", false, b"\xc2"),
            (b"\xc2".to_vec().into(), b"\xc2", false, b"\xc2\xc2"),
            (b"\xc2".to_vec().into(), b"\x80", true, b"\xc2\x80"),
            (b"\x80".to_vec().into(), b"def", false, b"\x80def"),
            (b"\x80".to_vec().into(), b"def\xc2", false, b"\x80def\xc2"),
            (b"\x80".to_vec().into(), b"def\x80", false, b"\x80def\x80"),
            (b"\x80".to_vec().into(), b"", false, b"\x80"),
            (b"\x80".to_vec().into(), b"\xc2", false, b"\x80\xc2"),
            (b"\x80".to_vec().into(), b"\x80", false, b"\x80\x80"),
        ];

        for (mut target, append, is_utf8, bytes) in test_cases {
            let mut target_clone = target.clone();
            target.extend_bytes(append);
            assert_eq!(target.is_utf8(), is_utf8);
            assert_eq!(target.as_bytes(), bytes);

            target_clone.extend_byte_string(&append.to_vec().into());
            assert_eq!(target_clone.is_utf8(), is_utf8);
            assert_eq!(target_clone.as_bytes(), bytes);
        }
    }

    #[test]
    fn test_truncate() {
        let test_cases: Vec<(ByteString, usize, bool, &[u8], bool, &[u8])> = vec![
            ("abc".to_owned().into(), 2, true, b"ab", false, b"ab\x80"),
            ("abc".to_owned().into(), 3, true, b"abc", false, b"abc\x80"),
            ("abc".to_owned().into(), 0, true, b"", false, b"\x80"),
            (
                b"abc\xc2\x80".to_vec().into(),
                4,
                false,
                b"abc\xc2",
                true,
                b"abc\xc2\x80",
            ),
            (
                b"abc\xf0\x80".to_vec().into(),
                4,
                false,
                b"abc\xf0",
                false,
                b"abc\xf0\x80",
            ),
            (
                b"abc\x80\x80".to_vec().into(),
                4,
                false,
                b"abc\x80",
                false,
                b"abc\x80\x80",
            ),
        ];

        for (mut target, trunc_len, is_utf8, bytes, is_utf8_after, bytes_after) in test_cases {
            target.truncate(trunc_len);
            assert_eq!(target.is_utf8(), is_utf8);
            assert_eq!(target.as_bytes(), bytes);

            target.extend_bytes(b"\x80");
            assert_eq!(target.is_utf8(), is_utf8_after);
            assert_eq!(target.as_bytes(), bytes_after);
        }
    }

    #[test]
    fn test_drain_init() {
        let test_cases: Vec<(ByteString, usize, bool, &[u8])> = vec![
            ("abc".to_owned().into(), 2, true, b"c"),
            ("abc".to_owned().into(), 3, true, b""),
            (b"\xc2\x80".to_vec().into(), 1, false, b"\x80"),
            (b"\xc2\x80".to_vec().into(), 2, true, b""),
            (b"\x80\xc2".to_vec().into(), 1, false, b"\xc2"),
            (b"\x80\xc2\x80".to_vec().into(), 1, true, b"\xc2\x80"),
        ];

        for (mut target, drain_len, is_utf8, bytes) in test_cases {
            target.drain_init(drain_len);
            assert_eq!(target.is_utf8(), is_utf8);
            assert_eq!(target.as_bytes(), bytes);
        }
    }
}
