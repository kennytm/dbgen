//! Debug functions.

use super::{Arguments, Function};
use crate::{
    error::Error,
    eval::{CompileContext, C},
    span::{Span, SpanExt, S},
};

/// The `debug.panic` function.
#[derive(Debug)]
pub struct Panic;

impl Function for Panic {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        use std::fmt::Write;
        let mut message = String::new();
        for (arg, i) in args.into_iter().zip(1..) {
            write!(&mut message, "\n {}. {}", i, arg.inner).unwrap();
        }
        Err(Error::Panic { message }.span(span))
    }
}
