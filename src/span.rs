//! Span of substrings from the template file, for error reporting.

use crate::parser::Rule;
use pest::error::{Error, ErrorVariant};

/// The span of an object, indicating the start and end offsets where the
/// object was parsed from the template file.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Span(usize);

impl Default for Span {
    fn default() -> Self {
        Self(usize::MAX)
    }
}

/// Registry of spans.
#[derive(Default, Debug, Clone)]
pub struct Registry(Vec<Error<Rule>>);

impl Registry {
    /// Registers a span represented by a Pest span.
    pub fn register(&mut self, span: pest::Span<'_>) -> Span {
        let res = Span(self.0.len());
        self.0.push(Error::new_from_span(
            ErrorVariant::CustomError { message: "".to_owned() },
            span,
        ));
        res
    }

    /// Describes a spanned error as a human-readable string.
    pub fn describe<E: std::error::Error + 'static>(&self, err: &S<E>) -> String {
        use std::fmt::Write;
        let mut buf = format!("Error: {}\n", err.inner);

        if let Some(e) = self.0.get(err.span.0) {
            writeln!(&mut buf, "{}\n", e).unwrap();
        }

        let mut err: &(dyn std::error::Error + 'static) = &err.inner;
        while let Some(source) = err.source() {
            writeln!(&mut buf, "Cause: {}", source).unwrap();
            err = source;
        }

        buf
    }
}

/// A wrapper of around object, annotating it with a span.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct S<T> {
    /// The object itself.
    pub inner: T,
    /// The span associated with the object.
    pub span: Span,
}

/// Extension trait for all values for associating it with a span.
pub trait SpanExt: Sized {
    /// Associates this value with a span.
    fn span(self, span: Span) -> S<Self>;

    /// Associates this value with the default (null) span.
    fn no_span(self) -> S<Self>;
}

impl<T> SpanExt for T {
    fn span(self, span: Span) -> S<Self> {
        S { span, inner: self }
    }

    fn no_span(self) -> S<Self> {
        self.span(Span::default())
    }
}

/// Extension trait for `Result` for associating part of it with a span.
pub trait ResultExt {
    /// The ok type of the result.
    type Ok;
    /// The error type of the result.
    type Err;

    /// Associates the same span to both the ok and error part of the result.
    fn span_ok_err<U: From<Self::Err>>(self, span: Span) -> Result<S<Self::Ok>, S<U>>;
    /// Associates the span to the error part of the result.
    fn span_err<U: From<Self::Err>>(self, span: Span) -> Result<Self::Ok, S<U>>;

    /// Associates the default (null) span to the error part of the result.
    fn no_span_err<U: From<Self::Err>>(self) -> Result<Self::Ok, S<U>>;
}

impl<T, E> ResultExt for Result<T, E> {
    type Ok = T;
    type Err = E;

    fn span_ok_err<U: From<E>>(self, span: Span) -> Result<S<T>, S<U>> {
        match self {
            Ok(t) => Ok(S { span, inner: t }),
            Err(e) => Err(S { span, inner: e.into() }),
        }
    }

    fn span_err<U: From<E>>(self, span: Span) -> Result<T, S<U>> {
        self.map_err(|e| S { span, inner: e.into() })
    }

    fn no_span_err<U: From<E>>(self) -> Result<T, S<U>> {
        self.span_err(Span::default())
    }
}
