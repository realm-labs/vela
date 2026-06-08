use vela_host::error::HostResult;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::args::IntoScriptArg;

pub trait IntoNativeReturn {
    fn into_native_return(self) -> VmResult<OwnedValue>;
}

impl<T> IntoNativeReturn for T
where
    T: IntoScriptArg,
{
    fn into_native_return(self) -> VmResult<OwnedValue> {
        Ok(self.into_script_arg())
    }
}

impl<T> IntoNativeReturn for VmResult<T>
where
    T: IntoScriptArg,
{
    fn into_native_return(self) -> VmResult<OwnedValue> {
        self.map(IntoScriptArg::into_script_arg)
    }
}

impl<T> IntoNativeReturn for HostResult<T>
where
    T: IntoScriptArg,
{
    fn into_native_return(self) -> VmResult<OwnedValue> {
        self.map(IntoScriptArg::into_script_arg).map_err(|error| {
            VmError::new(VmErrorKind::Host(error.kind)).with_source_span(error.source_span)
        })
    }
}
