use std::fmt;

use crate::candidates::ReflectCandidate;
use crate::permissions::ReflectPermission;
use vela_common::{HostTypeId, Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectError {
    pub kind: ReflectErrorKind,
}

impl ReflectError {
    pub(crate) fn new(kind: ReflectErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for ReflectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for ReflectError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReflectErrorKind {
    UnknownType {
        host_type_id: HostTypeId,
    },
    UnknownTypeName {
        type_name: String,
        candidates: Vec<String>,
        related: Vec<ReflectCandidate>,
    },
    UnknownField {
        type_name: String,
        field: String,
        candidates: Vec<String>,
        related: Vec<ReflectCandidate>,
    },
    UnknownMethod {
        type_name: String,
        method: String,
        candidates: Vec<String>,
        related: Vec<ReflectCandidate>,
    },
    UnknownVariant {
        type_name: String,
        variant: String,
        candidates: Vec<String>,
        related: Vec<ReflectCandidate>,
    },
    UnknownTrait {
        trait_name: String,
        candidates: Vec<String>,
        related: Vec<ReflectCandidate>,
    },
    UnknownModule {
        module: String,
        candidates: Vec<String>,
        related: Vec<ReflectCandidate>,
    },
    UnknownFunction {
        function: String,
        candidates: Vec<String>,
        related: Vec<ReflectCandidate>,
    },
    UnknownPermission {
        permission: String,
        candidates: Vec<String>,
    },
    PermissionDenied {
        permission: ReflectPermission,
    },
    MethodNotReflectCallable {
        type_name: String,
        method: String,
        source_span: Option<Span>,
    },
    FunctionNotReflectVisible {
        function: String,
        source_span: Option<Span>,
    },
    FunctionNotReflectCallable {
        function: String,
        source_span: Option<Span>,
    },
    MethodPermissionDenied {
        method: String,
        permission: String,
        source_span: Option<Span>,
    },
    MethodEffectPermissionDenied {
        method: String,
        permission: ReflectPermission,
        source_span: Option<Span>,
    },
    FunctionEffectPermissionDenied {
        function: String,
        permission: ReflectPermission,
        source_span: Option<Span>,
    },
    FunctionPermissionDenied {
        function: String,
        permission: String,
        source_span: Option<Span>,
    },
    FieldPermissionDenied {
        type_name: String,
        field: String,
        permission: String,
        source_span: Option<Span>,
    },
    LookupBudgetExceeded {
        limit: u64,
    },
    FieldNotWritable {
        type_name: String,
        field: String,
        source_span: Option<Span>,
    },
    FieldNotReflectReadable {
        type_name: String,
        field: String,
        source_span: Option<Span>,
    },
    FieldNotReflectWritable {
        type_name: String,
        field: String,
        source_span: Option<Span>,
    },
    InvalidTarget,
    InvalidValue,
    Host(String),
}

pub type ReflectResult<T> = Result<T, ReflectError>;
