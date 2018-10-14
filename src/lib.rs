#![cfg_attr(
    feature = "cargo-clippy",
    warn(
        clippy::pedantic,
        missing_debug_implementations,
        trivial_casts,
        trivial_numeric_casts,
        unreachable_pub,
        variant_size_differences,
        rust_2018_idioms
    )
)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::stutter, unused_extern_crates))]

// TODO remove these `extern crate` once racer-rust/racer#916 is closed.
extern crate chrono;
extern crate data_encoding;
extern crate failure;
extern crate num_traits;
extern crate pest;
extern crate rand;
extern crate regex_syntax;
extern crate ryu;
extern crate structopt;
extern crate zipf;

pub mod cli;
pub mod error;
pub mod eval;
pub mod gen;
pub mod parser;
pub mod regex;
pub mod value;
