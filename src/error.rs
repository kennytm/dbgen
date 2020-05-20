//! Error types for the `dbgen` library.

#![allow(clippy::used_underscore_binding)]

use crate::{parser::Rule, span::S};
use std::{convert::Infallible, fmt, path::PathBuf};
use thiserror::Error as ThisError;

/// Errors produced by the `dbgen` library.
#[derive(ThisError, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Failed to parse template.
    #[error("failed to parse template")]
    ParseTemplate(#[from] pest::error::Error<Rule>),

    /// Unknown SQL function.
    #[error("unknown function")]
    UnknownFunction,

    /// Integer is too big.
    #[error("integer '{0}' is too big")]
    IntegerOverflow(
        /// The string representation of the expression that produced the overflow.
        String,
    ),

    /// Not enough arguments provided to the SQL function.
    #[error("not enough arguments")]
    NotEnoughArguments,

    /// Invalid regex.
    #[error("invalid regex")]
    InvalidRegex(#[from] rand_regex::Error),

    /// Unknown regex flag.
    #[error("unknown regex flag '{0}'")]
    UnknownRegexFlag(
        /// The regex flag.
        char,
    ),

    /// Invalid arguments.
    #[error("{0}")]
    InvalidArguments(
        /// Cause of the error.
        String,
    ),

    /// The timestamp string is invalid
    #[error("invalid timestamp")]
    InvalidTimestampString(#[from] chrono::format::ParseError),

    /// Cannot find parent table for derived table directive.
    #[error("cannot find parent table {parent} to generate derived rows")]
    UnknownParentTable {
        /// Expected parent table name.
        parent: String,
    },

    /// Derived table name does not match that of the derived table directive.
    #[error("derived table name in the FOR EACH ROW and CREATE TABLE statements do not match ({for_each_row} vs {create_table})")]
    DerivedTableNameMismatch {
        /// The table name in the FOR EACH ROW statement
        for_each_row: String,
        /// The table name in the CREATE TABLE statement
        create_table: String,
    },

    /// Unexpected value type.
    #[error("cannot convert {value} into {expected}")]
    UnexpectedValueType {
        /// The expected value type.
        expected: &'static str,
        /// The actual value.
        value: String,
    },

    /// Generic IO error.
    #[error("failed to {action} at {path}")]
    Io {
        /// Action causing the error.
        action: &'static str,
        /// File path causing the I/O error.
        path: PathBuf,
        /// Source of error.
        source: std::io::Error,
    },

    /// Invalid time zone file.
    #[error("failed to parse time zone file ({time_zone})")]
    InvalidTimeZone {
        /// Time zone name.
        time_zone: String,
        /// Source of error.
        source: tzfile::Error,
    },

    /// Failed to configure a Rayon thread pool.
    #[cfg(feature = "cli")]
    #[error("failed to configure thread pool")]
    Rayon(#[from] rayon::ThreadPoolBuildError),

    /// Cannot use `--table-name` when template contains multiple tables.
    #[error("cannot use --table-name when template contains multiple tables")]
    CannotUseTableNameForMultipleTables,

    /// Unsupported CLI parameter.
    #[error("unsupported {kind} {value}")]
    UnsupportedCliParameter {
        /// The parameter name.
        kind: &'static str,
        /// Value provided by user.
        value: String,
    },
}

impl fmt::Display for S<Error> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for S<Error> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl From<Infallible> for Error {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}

impl From<regex_syntax::Error> for Error {
    fn from(e: regex_syntax::Error) -> Self {
        Self::InvalidRegex(e.into())
    }
}
