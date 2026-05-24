use std::fmt;

use vela_common::{HostMethodId, HostObjectId, HostTypeId};

use crate::HostPath;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostError {
    pub kind: HostErrorKind,
}

impl HostError {
    pub(crate) fn new(kind: HostErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for HostError {}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    MissingOverlay {
        path: HostPath,
    },
    MissingPath {
        path: HostPath,
    },
    InvalidAdd {
        path: HostPath,
    },
    UnsupportedPatch {
        op: &'static str,
    },
    UnsupportedMethod {
        method: HostMethodId,
    },
}

pub type HostResult<T> = Result<T, HostError>;
