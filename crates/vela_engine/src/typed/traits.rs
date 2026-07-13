use vela_host::path::HostPath;
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::context::NativeCallContext;
use crate::native::NativeCallFuture;

pub trait TypedAsyncNativeFunction<Args>: Send + Sync + 'static {
    fn call_async<'call>(&self, args: &'call [OwnedValue]) -> NativeCallFuture<'call>;
}

pub trait TypedAsyncContextHostNativeFunction<Args>: Send + Sync + 'static {
    fn call_async_context<'call, 'host>(
        &self,
        args: &'call [OwnedValue],
        ctx: &'call mut NativeCallContext<'call, 'host>,
    ) -> NativeCallFuture<'call>;
}

pub trait TypedAsyncHostNativeFunction<Args>: Send + Sync + 'static {
    fn call_async_host<'call, 'host>(
        &self,
        args: &'call [OwnedValue],
        host: &'call mut HostExecution<'host>,
    ) -> NativeCallFuture<'call>;
}

pub trait TypedAsyncNativeMethodFunction<Args>: Send + Sync + 'static {
    fn call_async_method<'call, 'host>(
        &self,
        receiver: &'call HostPath,
        args: &'call [OwnedValue],
        host: &'call mut HostExecution<'host>,
    ) -> NativeCallFuture<'call>;
}

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
