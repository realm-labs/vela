use vela_host::object::ScriptHostObject;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::args::FromScriptArg;
use crate::runtime::DirectHostIdentity;

/// Selects conversion for an ordinary infallible Rust return.
pub struct ValueReturn;

/// Selects conversion for a boundary-safe business `Result`.
pub struct BusinessResultReturn;

/// Selects conversion for a runtime-fallible `VmResult`.
pub struct RuntimeResultReturn;

/// Converts one Vela override result through the authored Rust return family.
///
/// The mode parameter keeps ordinary values, business errors, and VM failures
/// distinct without specialization or a blanket `VmResult<T>` restriction.
pub trait FromDispatchReturn<Mode>: Sized {
    fn from_dispatch_return(result: VmResult<OwnedValue>) -> Self;
}

impl<T> FromDispatchReturn<ValueReturn> for T
where
    T: FromScriptArg,
{
    fn from_dispatch_return(result: VmResult<OwnedValue>) -> Self {
        let value = result.unwrap_or_else(|error| {
            panic!("Vela override for an infallible Rust entry failed: {error}")
        });
        T::from_script_arg(&value)
            .unwrap_or_else(|error| panic!("Vela override returned an incompatible value: {error}"))
    }
}

impl<T, E> FromDispatchReturn<BusinessResultReturn> for Result<T, E>
where
    T: FromScriptArg,
    E: FromScriptArg + From<VmError>,
{
    fn from_dispatch_return(result: VmResult<OwnedValue>) -> Self {
        let value = result.map_err(E::from)?;
        Result::<T, E>::from_script_arg(&value).map_err(E::from)?
    }
}

impl<T> FromDispatchReturn<RuntimeResultReturn> for VmResult<T>
where
    T: FromScriptArg,
{
    fn from_dispatch_return(result: VmResult<OwnedValue>) -> Self {
        let value = result?;
        T::from_script_arg(&value)
    }
}

pub fn scoped_shared_origin<'origin, T>(
    result: VmResult<OwnedValue>,
    identity: &DirectHostIdentity,
    origin: &'origin T,
) -> VmResult<&'origin T>
where
    T: ScriptHostObject + Sync,
{
    validate_direct_origin(result, identity)?;
    Ok(origin)
}

pub fn scoped_exclusive_origin<'origin, T>(
    result: VmResult<OwnedValue>,
    identity: &DirectHostIdentity,
    origin: &'origin mut T,
) -> VmResult<&'origin mut T>
where
    T: ScriptHostObject + Send + Sync,
{
    validate_direct_origin(result, identity)?;
    Ok(origin)
}

fn validate_direct_origin(
    result: VmResult<OwnedValue>,
    identity: &DirectHostIdentity,
) -> VmResult<()> {
    let returned = vela_host::path::HostRef::from_script_arg(&result?)?;
    if identity.host_ref() == Some(returned) {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::TypeMismatch {
        operation: "replaceable borrowed return provenance",
    }))
}
