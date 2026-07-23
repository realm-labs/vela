use vela_common::HostMethodId;

use crate::{
    error::{HostError, HostErrorKind},
    target::HostTargetInstance,
};

pub(super) fn invalid_arg(expected: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument { expected },
        source_span: None,
    }
}

pub(super) fn missing_target(target: HostTargetInstance<'_>) -> HostError {
    HostError {
        kind: HostErrorKind::MissingPath {
            path: target.to_diagnostic_path().to_host_path(),
        },
        source_span: None,
    }
}

pub(super) fn missing_collection_entry(target: HostTargetInstance<'_>) -> HostError {
    HostError {
        kind: HostErrorKind::MissingCollectionEntry {
            path: target.to_diagnostic_path().to_host_path(),
        },
        source_span: None,
    }
}

pub(super) fn permission_denied(target: HostTargetInstance<'_>, action: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::PermissionDenied {
            path: target.to_diagnostic_path().to_host_path(),
            action,
        },
        source_span: None,
    }
}

pub(super) fn unsupported_method(method: HostMethodId) -> HostError {
    HostError {
        kind: HostErrorKind::UnsupportedMethod { method },
        source_span: None,
    }
}
