#![warn(
    clippy::pedantic,
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    variant_size_differences,
    missing_docs,
    rust_2018_idioms
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::missing_errors_doc
)]

//! The reusable library powering `dbgen`.

/// The full version of this library, for use in the CLI
pub const FULL_VERSION: &str = concat!(
    "\nVersion: v",
    env!("CARGO_PKG_VERSION"),
    "\nCommit:  ",
    env!("VERGEN_SHA"),
    "\nTarget:  ",
    env!("VERGEN_TARGET_TRIPLE"),
);

pub mod bytes;
#[cfg(feature = "cli")]
pub mod cli;
pub mod error;
pub mod eval;
pub mod format;
pub mod functions;
pub mod number;
pub mod parser;
#[cfg(feature = "cli")]
pub mod schemagen_cli;
pub mod span;
pub mod value;
pub mod writer;
