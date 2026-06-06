use std::fmt;

use vela_common::{HostMethodId, HostObjectId, HostTypeId, Span};

use crate::path::HostPath;

#[derive(Clone, Debug, PartialEq)]
pub struct HostError {
    pub kind: HostErrorKind,
    pub source_span: Option<Span>,
}

impl HostError {
    pub(crate) fn new(kind: HostErrorKind) -> Self {
        Self {
            kind,
            source_span: None,
        }
    }

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
    StaleGeneration {
        expected: u32,
        actual: u32,
    },
    ObjectMismatch {
        expected: HostObjectId,
        actual: HostObjectId,
    },
    TypeMismatch {
        expected: HostTypeId,
        actual: HostTypeId,
    },
    MissingPath {
        path: HostPath,
    },
    PermissionDenied {
        path: HostPath,
        action: &'static str,
    },
    InvalidAdd {
        path: HostPath,
    },
    InvalidSub {
        path: HostPath,
    },
    InvalidMul {
        path: HostPath,
    },
    InvalidDiv {
        path: HostPath,
    },
    InvalidRem {
        path: HostPath,
    },
    InvalidPush {
        path: HostPath,
    },
    InvalidArgument {
        expected: &'static str,
    },
    UnsupportedMethod {
        method: HostMethodId,
    },
}

pub type HostResult<T> = Result<T, HostError>;
