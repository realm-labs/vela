use vela_host::error::HostRefLifetimeBoundary;
use vela_vm::error::VmResult;
use vela_vm::heap::ScriptHeap;
use vela_vm::value::Value;
use vela_vm::{LinkedExecutionSession, validate_persistent_value_host_refs};

use super::ExecutionHost;

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
    session.validate_host_ref_lifetime(heap, host, HostRefLifetimeBoundary::AsyncSuspend)
}
