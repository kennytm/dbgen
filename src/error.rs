//! Error types for the `dbgen` library.

use crate::parser::Function;
use failure::{Backtrace, Context, Fail};
use std::fmt;

/// Kinds of errors produced by the `dbgen` library.
#[derive(Fail, Debug, Clone, PartialEq, Eq)]
//#[non_exhaustive]
pub enum ErrorKind {
    /// Failed to parse template.
    #[fail(display = "failed to parse template")]
    ParseTemplate,

    /// Unknown SQL function.
    #[fail(display = "unknown function '{}'", 0)]
    UnknownFunction(
        /// The name of the unknown SQL function.
        String,
    ),

    /// Integer is too big.
    #[fail(display = "integer '{}' is too big", 0)]
    IntegerOverflow(
        /// The string representation of the expression that produced the overflow.
        String,
    ),

    /// Not enough arguments provided to the SQL function.
    #[fail(display = "not enough arguments to function {}", 0)]
    NotEnoughArguments(
        /// The SQL function causing the error.
        Function,
    ),

    /// Invalid regex.
    #[fail(display = "invalid regex {}", 0)]
    InvalidRegex(
        /// The regex pattern.
        String,
    ),

    /// Unknown regex flag.
    #[fail(display = "unknown regex flag {}", 0)]
    UnknownRegexFlag(
        /// The regex flag.
        char,
    ),

    /// Unsupported regex element (e.g. `\b`)
    #[fail(display = "unsupported regex element: '{}'", 0)]
    UnsupportedRegexElement(
        /// The regex element.
        String,
    ),

    /// Invalid argument type.
    ///
    /// If this error is encountered during compilation phase, the error will be
    /// ignored and the function will be kept in raw form.
    #[fail(
        display = "invalid argument type: in function {}, argument #{} should be a {}",
        name,
        index,
        expected
    )]
    InvalidArgumentType {
        /// The SQL function causing the error.
        name: Function,
        /// Argument index.
        index: usize,
        /// The expected type.
        expected: &'static str,
    },

    /// Invalid arguments.
    #[fail(
        display = "invalid arguments: in function {}, assertion failed: {}",
        name,
        cause
    )]
    InvalidArguments {
        /// The SQL function causing the error.
        name: Function,
        /// Cause of the error.
        cause: String,
    },

    /// The timestamp string does not follow the ISO-8601 format.
    #[fail(display = "timestamp '{}' does not follow the ISO-8601 format", 0)]
    InvalidTimestampString(
        /// The literal which is in the wrong format.
        String,
    ),

    /// Failed to write the SQL `CREATE TABLE` schema file.
    #[fail(display = "failed to write SQL schema")]
    WriteSqlSchema,

    /// Failed to write the SQL data file.
    #[fail(display = "failed to write SQL data")]
    WriteSqlData,

    /// Failed to write an SQL value.
    #[fail(display = "failed to write SQL value")]
    WriteSqlValue,

    #[doc(hidden)]
    #[fail(display = "(placeholder)")]
    __NonExhaustive,
}

/// An error produced by the `dbgen` library.
#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

impl Fail for Error {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl Error {
    /// The kind of this error.
    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Self {
        Self { inner }
    }
}
