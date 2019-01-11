#![cfg_attr(
    feature = "cargo-clippy",
    warn(
        clippy::pedantic,
        missing_debug_implementations,
        trivial_casts,
        trivial_numeric_casts,
        unreachable_pub,
        variant_size_differences,
        missing_docs,
        rust_2018_idioms
    )
)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::module_name_repetitions))]

//! The reusable library powering `dbgen`.

pub mod cli;
pub mod error;
pub mod eval;
pub mod format;
pub mod parser;
pub mod schemagen_cli;
pub mod value;
