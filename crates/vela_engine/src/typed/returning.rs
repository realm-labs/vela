use vela_host::error::HostResult;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::value::Value;

use crate::args::IntoScriptArg;

pub trait IntoNativeReturn {
    fn into_native_return(self) -> VmResult<Value>;
}

impl<T> IntoNativeReturn for T
where
    T: IntoScriptArg,
{
    fn into_native_return(self) -> VmResult<Value> {
        Ok(self.into_script_arg())
    }
}

impl<T> IntoNativeReturn for VmResult<T>
where
    T: IntoScriptArg,
{
    fn into_native_return(self) -> VmResult<Value> {
        self.map(IntoScriptArg::into_script_arg)
    }
}

impl<T> IntoNativeReturn for HostResult<T>
where
    T: IntoScriptArg,
{
    fn into_native_return(self) -> VmResult<Value> {
        self.map(IntoScriptArg::into_script_arg)
            .map_err(|error| VmError {
                kind: VmErrorKind::Host(error.kind),
                source_span: error.source_span,
                call_stack: Default::default(),
            })
    }
}
