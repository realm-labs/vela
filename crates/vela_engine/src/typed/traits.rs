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
        ctx: &mut NativeCallContext<'_>,
    ) -> VmResult<OwnedValue>;
}
