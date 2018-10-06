// TODO remove all #[cfg_attr(feature = "cargo-clippy")] once tool_lints is stabilized.
#![cfg_attr(feature = "cargo-clippy", feature(tool_lints))]
#![cfg_attr(feature = "cargo-clippy", warn(clippy::pedantic))]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::stutter))]

// TODO remove these `extern crate` once RLS understands these are not needed.
extern crate data_encoding;
extern crate failure;
extern crate num_traits;
extern crate pest;
extern crate rand;
extern crate regex_syntax;
extern crate structopt;

pub mod error;
pub mod eval;
pub mod gen;
pub mod parser;
pub mod regex;
pub mod value;
