#![warn(
    clippy::pedantic,
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    variant_size_differences,
    missing_docs,
    rust_2024_compatibility,
    deprecated_in_future,
    future_incompatible,
    let_underscore,
    clippy::undocumented_unsafe_blocks,
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    clippy::branches_sharing_code,
    clippy::cognitive_complexity,
    clippy::collection_is_never_read,
    clippy::dbg_macro,
    clippy::debug_assert_with_mut_call,
    clippy::derive_partial_eq_without_eq,
    clippy::format_push_string,
    clippy::if_then_some_else_none,
    clippy::imprecise_flops,
    clippy::infinite_loop,
    clippy::iter_on_empty_collections,
    clippy::iter_on_single_items,
    clippy::iter_with_drain,
    clippy::large_stack_frames,
    clippy::let_underscore_must_use,
    clippy::let_underscore_untyped,
    clippy::lossy_float_literal,
    clippy::mixed_read_write_in_expression,
    clippy::multiple_unsafe_ops_per_block,
    clippy::mutex_atomic,
    clippy::mutex_integer,
    clippy::needless_collect,
    clippy::needless_pass_by_ref_mut,
    clippy::or_fun_call,
    clippy::rc_buffer,
    clippy::read_zero_byte_vec,
    clippy::redundant_clone,
    clippy::redundant_pub_crate,
    clippy::redundant_type_annotations,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::str_to_string,
    clippy::string_lit_as_bytes,
    clippy::string_to_string,
    clippy::suspicious_operation_groupings,
    clippy::todo,
    clippy::trivial_regex,
    clippy::try_err,
    clippy::tuple_array_conversions,
    clippy::unimplemented,
    clippy::use_self,
    clippy::useless_let_if_seq,
    clippy::verbose_file_reads
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::option_if_let_else,
    clippy::missing_panics_doc
)]

//! The reusable library powering `dbgen`.

/// The full version of this library, for use in the CLI
pub const FULL_VERSION: &str = concat!(
    "\nVersion: v",
    env!("CARGO_PKG_VERSION"),
    "\nCommit:  ",
    env!("VERGEN_GIT_SHA"),
    "\nTarget:  ",
    env!("VERGEN_CARGO_TARGET_TRIPLE"),
);

pub mod array;
pub mod bytes;
#[cfg(feature = "cli")]
pub mod cli;
pub mod error;
pub mod eval;
pub mod format;
pub mod functions;
pub mod lexctr;
pub mod number;
pub mod parser;
#[cfg(feature = "cli")]
pub mod schemagen_cli;
pub mod span;
pub mod value;
pub mod writer;
