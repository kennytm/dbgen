//! Error types for the `dbgen` library.

use crate::parser::Rule;
use std::{fmt, path::PathBuf};
use thiserror::Error as ThisError;

/// The error returned for `impl TryFrom<Value>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryFromValueError(pub(crate) &'static str);

impl fmt::Display for TryFromValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for TryFromValueError {}

/// Errors produced by the `dbgen` library.
#[derive(ThisError, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Failed to parse template.
    #[error("failed to parse template")]
    ParseTemplate {
        /// Cause of template error.
        #[from]
        source: pest::error::Error<Rule>,
    },

    /// Unknown SQL function.
    #[error("unknown function '{0}'")]
    UnknownFunction(
        /// The name of the unknown SQL function.
        String,
    ),

    /// Integer is too big.
    #[error("integer '{0}' is too big")]
    IntegerOverflow(
        /// The string representation of the expression that produced the overflow.
        String,
    ),

    /// Not enough arguments provided to the SQL function.
    #[error("not enough arguments to function {0}")]
    NotEnoughArguments(
        /// The SQL function causing the error.
        &'static str,
    ),

    /// Invalid regex.
    #[error("invalid regex {pattern}")]
    InvalidRegex {
        /// The regex pattern.
        pattern: String,
        /// Source of error
        source: rand_regex::Error,
    },

    /// Unknown regex flag.
    #[error("unknown regex flag {0}")]
    UnknownRegexFlag(
        /// The regex flag.
        char,
    ),

    /// Invalid argument type.
    ///
    /// If this error is encountered during compilation phase, the error will be
    /// ignored and the function will be kept in raw form.
    #[error("invalid argument type: in function {name}, argument #{index} should be a {expected}")]
    InvalidArgumentType {
        /// The SQL function causing the error.
        name: &'static str,
        /// Argument index.
        index: usize,
        /// The expected type.
        expected: String,
    },

    /// Invalid arguments.
    #[error("invalid arguments: in function {name}, assertion failed: {cause}")]
    InvalidArguments {
        /// The SQL function causing the error.
        name: &'static str,
        /// Cause of the error.
        cause: String,
    },

    /// The timestamp string is invalid
    #[error("invalid timestamp '{timestamp}'")]
    InvalidTimestampString {
        /// The literal which is in the wrong format.
        timestamp: String,
        /// Source of the error.
        source: chrono::format::ParseError,
    },

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

    /// Number of rows generated for the table is not an integer.
    #[error("number of rows to generate for {table} is not an integer")]
    NonIntegralGeneratedNumberOfRows {
        /// Qualified name of the derived table.
        table: String,
        /// Source of error.
        source: TryFromValueError,
    },

    /// Generic IO error.
    #[error("failed to read {path}")]
    Io {
        /// File path causing the I/O error.
        path: PathBuf,
        /// Source of error.
        source: eieio::Error,
    },

    /// Invalid time zone file.
    #[error("failed to parse time zone file ({time_zone})")]
    InvalidTimeZone {
        /// Time zone name.
        time_zone: String,
        /// Source of error.
        source: tzfile::Error,
    },
}
