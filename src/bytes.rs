//! Byte string.

use std::{
    cmp::Ordering,
    convert::TryFrom,
    fmt, io,
    ops::Add,
    str::{from_utf8, Utf8Error},
    string::FromUtf8Error,
};

/// Describes how the input bytes failed the UTF-8 encoding.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TryIntoStringError {
    /// The bytes are valid until this position, which encountered the first
    /// non-UTF-8 bytes.
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

/// An SQL string (UTF-8 or byte-string).
#[derive(Clone, Eq, Debug, Default)]
pub struct Bytes {
    /// The raw bytes.
    bytes: Vec<u8>,
    /// If the bytes are not valid UTF-8, this field contains information about
    /// the failure.
    error: Option<TryIntoStringError>,
}

impl PartialEq for Bytes {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl PartialOrd for Bytes {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.bytes.partial_cmp(&other.bytes)
    }
}

impl Ord for Bytes {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bytes.cmp(&other.bytes)
    }
}

impl From<String> for Bytes {
    fn from(s: String) -> Self {
        Self {
            bytes: s.into_bytes(),
            error: None,
        }
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Self {
        String::from_utf8(bytes).into()
    }
}

impl From<FromUtf8Error> for Bytes {
    fn from(error: FromUtf8Error) -> Self {
        Self {
            error: Some(error.utf8_error().into()),
            bytes: error.into_bytes(),
        }
    }
}

impl From<Result<String, FromUtf8Error>> for Bytes {
    fn from(result: Result<String, FromUtf8Error>) -> Self {
        match result {
            Ok(s) => s.into(),
            Err(e) => e.into(),
        }
    }
}

impl From<Bytes> for Vec<u8> {
    fn from(bytes: Bytes) -> Self {
        bytes.bytes
    }
}

impl TryFrom<Bytes> for String {
    type Error = TryIntoStringError;
    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        if let Some(e) = bytes.error {
            Err(e)
        } else {
            Ok(unsafe { String::from_utf8_unchecked(bytes.bytes) })
        }
    }
}

impl io::Write for Bytes {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend_raw_bytes(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Write for Bytes {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.extend_str(s);
        Ok(())
    }
}

impl Bytes {
    /// Validates the invariant. Becomes no-op on release build.
    fn debug_validate(&self) {
        debug_assert_eq!(self.error, from_utf8(&self.bytes).err().map(TryIntoStringError::from));
    }

    /// Gets the total byte length.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the bytes are empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Gets the length where the bytes are valid UTF-8 up to this point.
    fn valid_len(&self) -> usize {
        self.error.map_or(self.bytes.len(), |e| e.valid_len)
    }

    /// Recomputes the `error` field, provided the bytes up to `old_valid_len`
    /// are valid UTF-8.
    fn recompute_error(&self, old_valid_len: usize) -> Option<TryIntoStringError> {
        from_utf8(&self.bytes[old_valid_len..])
            .err()
            .map(|e| TryIntoStringError::from(e) + old_valid_len)
    }

    /// Gets whether the bytes consists of valid UTF-8 content.
    pub fn is_utf8(&self) -> bool {
        self.error.is_none()
    }

    /// Gets whether the bytes contained non-UTF-8 content.
    pub fn is_binary(&self) -> bool {
        self.error.is_some()
    }

    /// Gets the byte content of this string.
    pub fn as_raw_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Extracts ownership of the byte content of this string.
    pub fn into_raw_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Extends a string to the end of these bytes.
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

    /// Extends some bytes to the end of these bytes.
    pub fn extend_raw_bytes(&mut self, b: &[u8]) {
        if b.is_empty() {
            return;
        }

        let old_valid_len = self.valid_len();
        self.bytes.extend_from_slice(b);
        if self.error.map_or(false, |e| !e.is_partial) {
            return;
        }

        self.error = self.recompute_error(old_valid_len);

        self.debug_validate();
    }

    /// Extends another bytes instance.
    pub fn extend_bytes(&mut self, other: Self) {
        if other.is_empty() {
            return;
        }

        let old_len = self.bytes.len();
        self.bytes.extend_from_slice(&other.bytes);
        self.error = match (self.error, other.error) {
            // Do nothing if we append a string to a string, or to binary which cannot be corrected.
            (e @ None, None) | (e @ Some(TryIntoStringError { is_partial: false, .. }), _) => e,
            // If we append binary to a string, inherit the binary error info.
            (None, Some(oe)) => Some(oe + old_len),
            // If we append start-with-binary to partial binary, recompute the info.
            (
                Some(TryIntoStringError {
                    is_partial: true,
                    valid_len,
                }),
                Some(TryIntoStringError { valid_len: 0, .. }),
            ) => self.recompute_error(valid_len),
            // If we append start-with-string to partial binary, make the existing error impartial.
            (Some(TryIntoStringError { valid_len, .. }), _) => Some(TryIntoStringError {
                valid_len,
                is_partial: false,
            }),
        };

        self.debug_validate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_str() {
        let test_cases: Vec<(Bytes, &str, bool, &[u8])> = vec![
            (Bytes::from("abc".to_owned()), "def", true, b"abcdef"),
            (Bytes::from("abc".to_owned()), "", true, b"abc"),
            (Bytes::from(b"abc\xc2".to_vec()), "def", false, b"abc\xc2def"),
            (Bytes::from(b"abc\xc2".to_vec()), "", false, b"abc\xc2"),
            (Bytes::from(b"abc\x80".to_vec()), "def", false, b"abc\x80def"),
            (Bytes::from(b"abc\x80".to_vec()), "", false, b"abc\x80"),
            (Bytes::default(), "def", true, b"def"),
            (Bytes::default(), "", true, b""),
            (Bytes::from(b"\xc2".to_vec()), "def", false, b"\xc2def"),
            (Bytes::from(b"\xc2".to_vec()), "", false, b"\xc2"),
            (Bytes::from(b"\x80".to_vec()), "def", false, b"\x80def"),
            (Bytes::from(b"\x80".to_vec()), "", false, b"\x80"),
        ];

        for (mut target, string, is_utf8, bytes) in test_cases {
            let mut target_clone = target.clone();
            let mut target_clone_2 = target.clone();
            let mut target_clone_3 = target.clone();

            target.extend_str(string);
            assert_eq!(target.is_utf8(), is_utf8);
            assert_eq!(target.as_raw_bytes(), bytes);

            target_clone.extend_bytes(string.to_owned().into());
            assert_eq!(target_clone.is_utf8(), is_utf8);
            assert_eq!(target_clone.as_raw_bytes(), bytes);

            {
                use std::io::Write;
                write!(&mut target_clone_2, "{}", string).unwrap();
            }
            assert_eq!(target_clone_2.is_utf8(), is_utf8);
            assert_eq!(target_clone_2.as_raw_bytes(), bytes);

            {
                use std::fmt::Write;
                write!(&mut target_clone_3, "{}", string).unwrap();
            }
            assert_eq!(target_clone_3.is_utf8(), is_utf8);
            assert_eq!(target_clone_3.as_raw_bytes(), bytes);
        }
    }

    #[test]
    fn test_push_bytes() {
        let test_cases: Vec<(Bytes, &[u8], bool, &[u8])> = vec![
            (Bytes::from("abc".to_owned()), b"def", true, b"abcdef"),
            (Bytes::from("abc".to_owned()), b"def\xc2", false, b"abcdef\xc2"),
            (Bytes::from("abc".to_owned()), b"def\x80", false, b"abcdef\x80"),
            (Bytes::from("abc".to_owned()), b"", true, b"abc"),
            (Bytes::from("abc".to_owned()), b"\xc2", false, b"abc\xc2"),
            (Bytes::from("abc".to_owned()), b"\x80", false, b"abc\x80"),
            (Bytes::from(b"abc\xc2".to_vec()), b"def", false, b"abc\xc2def"),
            (Bytes::from(b"abc\xc2".to_vec()), b"def\xc2", false, b"abc\xc2def\xc2"),
            (Bytes::from(b"abc\xc2".to_vec()), b"def\x80", false, b"abc\xc2def\x80"),
            (Bytes::from(b"abc\xc2".to_vec()), b"", false, b"abc\xc2"),
            (Bytes::from(b"abc\xc2".to_vec()), b"\xc2", false, b"abc\xc2\xc2"),
            (Bytes::from(b"abc\xc2".to_vec()), b"\x80", true, b"abc\xc2\x80"),
            (Bytes::from(b"abc\x80".to_vec()), b"def", false, b"abc\x80def"),
            (Bytes::from(b"abc\x80".to_vec()), b"def\xc2", false, b"abc\x80def\xc2"),
            (Bytes::from(b"abc\x80".to_vec()), b"def\x80", false, b"abc\x80def\x80"),
            (Bytes::from(b"abc\x80".to_vec()), b"", false, b"abc\x80"),
            (Bytes::from(b"abc\x80".to_vec()), b"\xc2", false, b"abc\x80\xc2"),
            (Bytes::from(b"abc\x80".to_vec()), b"\x80", false, b"abc\x80\x80"),
            (Bytes::default(), b"def", true, b"def"),
            (Bytes::default(), b"def\xc2", false, b"def\xc2"),
            (Bytes::default(), b"def\x80", false, b"def\x80"),
            (Bytes::default(), b"", true, b""),
            (Bytes::default(), b"\xc2", false, b"\xc2"),
            (Bytes::default(), b"\x80", false, b"\x80"),
            (Bytes::from(b"\xc2".to_vec()), b"def", false, b"\xc2def"),
            (Bytes::from(b"\xc2".to_vec()), b"def\xc2", false, b"\xc2def\xc2"),
            (Bytes::from(b"\xc2".to_vec()), b"def\x80", false, b"\xc2def\x80"),
            (Bytes::from(b"\xc2".to_vec()), b"", false, b"\xc2"),
            (Bytes::from(b"\xc2".to_vec()), b"\xc2", false, b"\xc2\xc2"),
            (Bytes::from(b"\xc2".to_vec()), b"\x80", true, b"\xc2\x80"),
            (Bytes::from(b"\x80".to_vec()), b"def", false, b"\x80def"),
            (Bytes::from(b"\x80".to_vec()), b"def\xc2", false, b"\x80def\xc2"),
            (Bytes::from(b"\x80".to_vec()), b"def\x80", false, b"\x80def\x80"),
            (Bytes::from(b"\x80".to_vec()), b"", false, b"\x80"),
            (Bytes::from(b"\x80".to_vec()), b"\xc2", false, b"\x80\xc2"),
            (Bytes::from(b"\x80".to_vec()), b"\x80", false, b"\x80\x80"),
        ];

        for (mut target, append, is_utf8, bytes) in test_cases {
            let mut target_clone = target.clone();
            target.extend_raw_bytes(append);
            assert_eq!(target.is_utf8(), is_utf8);
            assert_eq!(target.as_raw_bytes(), bytes);

            target_clone.extend_bytes(append.to_vec().into());
            assert_eq!(target_clone.is_utf8(), is_utf8);
            assert_eq!(target_clone.as_raw_bytes(), bytes);
        }
    }
}
