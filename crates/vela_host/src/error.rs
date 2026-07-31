use std::fmt;

use vela_common::{HostMethodId, HostObjectId, HostTypeId, Span};

use crate::path::{HostPath, HostSlotRef};
use crate::protocol::{HostCollectionMutationKind, HostCollectionQuery};

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
    MissingCollectionEntry {
        path: HostPath,
    },
    MissingExternState {
        name: String,
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
    UnsupportedCollectionQuery {
        query: HostCollectionQuery,
    },
    UnsupportedCollectionMutation {
        mutation: HostCollectionMutationKind,
    },
    HostObjectBusy {
        path: HostPath,
    },
    HostLeaseUnsupported {
        path: HostPath,
    },
    InvalidHostSlot {
        handle: HostSlotRef,
    },
    HostSlotStorageUnsupported,
    OwnedHostStorageUnsupported,
    CallScopedHostStorageUnsupported,
    NotScopedBorrow {
        path: HostPath,
    },
    ExpiredBorrowedHostRef {
        path: HostPath,
    },
    BorrowStillInUse {
        path: HostPath,
    },
    UnreleasedScopedResourcesAtAwait {
        resources: Vec<UnreleasedScopedResource>,
    },
    BorrowedHostRefEscape {
        path: HostPath,
        boundary: HostRefLifetimeBoundary,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopedResourceKind {
    View,
    MutView,
    Iterator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnreleasedScopedResource {
    pub path: HostPath,
    pub kind: ScopedResourceKind,
    pub parent: Option<HostPath>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostRefLifetimeBoundary {
    PersistentState,
    RootReturn,
    AsyncSuspend,
}

pub type HostResult<T> = Result<T, HostError>;
