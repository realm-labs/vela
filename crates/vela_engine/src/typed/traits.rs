use vela_host::HostPath;
use vela_vm::{HostExecution, Value, VmResult};

use crate::NativeCallContext;

pub trait TypedNativeFunction<Args>: Send + Sync + 'static {
    fn call(&self, args: &[Value]) -> VmResult<Value>;
}

pub trait TypedContextHostNativeFunction<Args>: Send + Sync + 'static {
    fn call_context(&self, args: &[Value], ctx: &mut NativeCallContext<'_, '_>) -> VmResult<Value>;
}

pub trait TypedHostNativeFunction<Args>: Send + Sync + 'static {
    fn call_host(&self, args: &[Value], host: &mut HostExecution<'_>) -> VmResult<Value>;
}

pub trait TypedNativeMethodFunction<Args>: Send + Sync + 'static {
    fn call_method(
        &self,
        receiver: &HostPath,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value>;
}
