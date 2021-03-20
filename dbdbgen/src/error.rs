use std::{fmt, io};
use thiserror::Error as ThisError;

/// The purpose of Jsonnet evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Produce argument specifications as CLI for itself.
    Arguments,
    /// Produce configurations for dbgen execution.
    Execution { step: usize },
}

impl fmt::Display for Purpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arguments => f.write_str("arguments"),
            Self::Execution { step } => write!(f, "execution (index={})", step),
        }
    }
}

#[derive(ThisError, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("failed to evaluate Jsonnet template for {}:\n{}", .purpose, .message)]
    Jsonnet { purpose: Purpose, message: String },

    #[error("cannot deserialize Jsonnet output for {}\n\n{}", .purpose, .src)]
    Serde {
        purpose: Purpose,
        src: String,
        #[source]
        error: serde_json::Error,
    },

    #[error("cannot execute dbgen (index={}):\n{}", .step, .message)]
    Dbgen { step: usize, message: String },
}
