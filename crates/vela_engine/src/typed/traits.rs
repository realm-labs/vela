use vela_host::path::HostPath;
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::context::NativeCallContext;

pub trait TypedNativeFunction<Args>: Send + Sync + 'static {
    fn call(&self, args: &[OwnedValue]) -> VmResult<OwnedValue>;
}

pub trait TypedContextHostNativeFunction<Args>: Send + Sync + 'static {
    fn call_context(
        &self,
        args: &[OwnedValue],
        ctx: &mut NativeCallContext<'_, '_>,
    ) -> VmResult<OwnedValue>;
}

pub trait TypedHostNativeFunction<Args>: Send + Sync + 'static {
    fn call_host(&self, args: &[OwnedValue], host: &mut HostExecution<'_>) -> VmResult<OwnedValue>;
}

pub trait TypedNativeMethodFunction<Args>: Send + Sync + 'static {
    fn call_method(
        &self,
        receiver: &HostPath,
        args: &[OwnedValue],
        host: &mut HostExecution<'_>,
    ) -> VmResult<OwnedValue>;
}
