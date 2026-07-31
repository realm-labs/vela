use vela_host::{adapter::ScriptStateAdapter, error::HostRefLifetimeBoundary};
use vela_vm::error::VmResult;
use vela_vm::heap::ScriptHeap;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;
use vela_vm::{LinkedExecutionSession, validate_persistent_value_host_refs};

use super::{DirectHostIdentity, ExecutionHost, ServiceScopedReturn, ServiceScopedReturnEnvelope};

pub(super) fn validate_root_return(
    value: &Value,
    heap: &ScriptHeap,
    host: &ExecutionHost<'_, '_>,
) -> VmResult<()> {
    validate_persistent_value_host_refs(value, heap, host, HostRefLifetimeBoundary::RootReturn)
}

pub(super) fn validate_async_suspend(
    session: &LinkedExecutionSession,
    heap: &ScriptHeap,
    host: &ExecutionHost<'_, '_>,
) -> VmResult<()> {
    host.validate_scoped_resources_before_await()?;
    session.validate_host_ref_lifetime(heap, host, HostRefLifetimeBoundary::AsyncSuspend)
}

pub(super) fn validate_service_scoped_return(
    value: OwnedValue,
    identity: &DirectHostIdentity,
    envelope: ServiceScopedReturnEnvelope,
) -> VmResult<ServiceScopedReturn> {
    let expected = identity.host_ref().ok_or_else(scoped_return_mismatch)?;
    match (envelope, value) {
        (ServiceScopedReturnEnvelope::Direct, OwnedValue::HostRef(returned))
            if returned == expected =>
        {
            Ok(ServiceScopedReturn::Borrowed)
        }
        (
            ServiceScopedReturnEnvelope::Option,
            OwnedValue::Enum {
                enum_name,
                variant,
                fields,
            },
        ) if is_standard_enum(&enum_name, "Option") => match variant.as_str() {
            "Some" if fields.get("0") == Some(&OwnedValue::HostRef(expected)) => {
                Ok(ServiceScopedReturn::Borrowed)
            }
            "None" if fields.is_empty() => Ok(ServiceScopedReturn::Empty),
            _ => Err(scoped_return_mismatch()),
        },
        (
            ServiceScopedReturnEnvelope::Result,
            OwnedValue::Enum {
                enum_name,
                variant,
                fields,
            },
        ) if is_standard_enum(&enum_name, "Result") => match variant.as_str() {
            "Ok" if fields.get("0") == Some(&OwnedValue::HostRef(expected)) => {
                Ok(ServiceScopedReturn::Borrowed)
            }
            "Err" => fields
                .get("0")
                .cloned()
                .map(ServiceScopedReturn::Error)
                .ok_or_else(scoped_return_mismatch),
            _ => Err(scoped_return_mismatch()),
        },
        _ => Err(scoped_return_mismatch()),
    }
}

fn is_standard_enum(name: &str, expected: &str) -> bool {
    name == expected || name.rsplit("::").next() == Some(expected)
}

fn scoped_return_mismatch() -> vela_vm::error::VmError {
    vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
        operation: "service borrowed return provenance",
    })
}
