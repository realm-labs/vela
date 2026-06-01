use std::fmt;

use crate::{ReflectCandidate, ReflectPermission};
use vela_common::HostTypeId;

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
    },
    FunctionNotReflectVisible {
        function: String,
    },
    FunctionNotReflectCallable {
        function: String,
    },
    MethodPermissionDenied {
        method: String,
        permission: String,
    },
    MethodEffectPermissionDenied {
        method: String,
        permission: ReflectPermission,
    },
    FunctionEffectPermissionDenied {
        function: String,
        permission: ReflectPermission,
    },
    FunctionPermissionDenied {
        function: String,
        permission: String,
    },
    FieldPermissionDenied {
        type_name: String,
        field: String,
        permission: String,
    },
    LookupBudgetExceeded {
        limit: u64,
    },
    FieldNotWritable {
        type_name: String,
        field: String,
    },
    FieldNotReflectReadable {
        type_name: String,
        field: String,
    },
    FieldNotReflectWritable {
        type_name: String,
        field: String,
    },
    InvalidTarget,
    InvalidValue,
    Host(String),
}

pub type ReflectResult<T> = Result<T, ReflectError>;
