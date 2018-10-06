use crate::parser::Function;
use failure::{Backtrace, Context, Fail};
use std::fmt;

#[derive(Fail, Debug, Clone, PartialEq, Eq)]
//#[non_exhaustive]
pub enum ErrorKind {
    #[fail(display = "failed to parse template")]
    ParseTemplate,

    #[fail(display = "unknown function '{}'", 0)]
    UnknownFunction(String),

    #[fail(display = "integer '{}' is too big", 0)]
    IntegerOverflow(String),

    #[fail(display = "not enough arguments to function {}", 0)]
    NotEnoughArguments(Function),

    #[fail(display = "invalid regex {}", 0)]
    InvalidRegex(String),

    #[fail(display = "unknown regex flag {}", 0)]
    UnknownRegexFlag(char),

    #[fail(display = "unsupported regex element: '{}'", 0)]
    UnsupportedRegexElement(String),

    #[fail(
        display = "invalid argument type: in function {}, argument #{} should be a {}",
        name,
        index,
        expected
    )]
    InvalidArgumentType {
        name: Function,
        index: usize,
        expected: &'static str,
    },

    #[fail(
        display = "invalid arguments: in function {}, assertion failed: {}",
        name,
        cause
    )]
    InvalidArguments { name: Function, cause: String },

    #[fail(display = "failed to write SQL schema")]
    WriteSqlSchema,

    #[fail(display = "failed to write SQL data")]
    WriteSqlData,

    #[fail(display = "failed to write SQL value")]
    WriteSqlValue,

    #[doc(hidden)]
    #[fail(display = "(placeholder)")]
    __NonExhaustive,
}

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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl Error {
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
