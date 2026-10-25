use std::fmt;

use vela_common::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct HostError {
    pub kind: HostErrorKind,
    pub source_span: Option<Span>,
}

impl HostError {
    #[must_use]
    pub fn with_source_span(mut self, source_span: Option<Span>) -> Self {
        self.source_span = source_span;
        self
    }

    #[must_use]
    pub fn with_source_span_if_absent(mut self, source_span: Option<Span>) -> Self {
        if self.source_span.is_none() {
            self.source_span = source_span;
        }
        self
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for HostError {}

#[derive(Clone, Debug, PartialEq)]
pub enum HostErrorKind {
    InvalidArgument { expected: &'static str },
}

pub type HostResult<T> = Result<T, HostError>;
