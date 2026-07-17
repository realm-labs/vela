use vela_host::path::HostRef;
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

/// Validates the HostRef payload of a borrowed replaceable return.
///
/// Generated adapters use this only for direct-origin returns. It proves that
/// every returned handle is the tracked entry argument before reconstructing
/// the authored reference container from that still-live Rust argument.
pub trait DispatchOriginPayload: FromScriptArg {
    fn validate_dispatch_origin(&self, identity: &DirectHostIdentity) -> VmResult<()>;
}

impl DispatchOriginPayload for HostRef {
    fn validate_dispatch_origin(&self, identity: &DirectHostIdentity) -> VmResult<()> {
        validate_returned_origin(*self, identity)
    }
}

macro_rules! dispatch_origin_tuple {
    ($($name:ident),+) => {
        impl<$($name),+> DispatchOriginPayload for ($($name,)+)
        where
            $($name: DispatchOriginPayload,)+
        {
            fn validate_dispatch_origin(
                &self,
                identity: &DirectHostIdentity,
            ) -> VmResult<()> {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                $($name.validate_dispatch_origin(identity)?;)+
                Ok(())
            }
        }
    };
}

dispatch_origin_tuple!(A, B);
dispatch_origin_tuple!(A, B, C);
dispatch_origin_tuple!(A, B, C, D);

pub fn validate_dispatch_origin_payload<P>(
    result: VmResult<OwnedValue>,
    identity: &DirectHostIdentity,
) -> VmResult<()>
where
    P: DispatchOriginPayload,
{
    P::from_script_arg(&result?)?.validate_dispatch_origin(identity)
}

pub fn validate_optional_dispatch_origin_payload<P>(
    result: VmResult<OwnedValue>,
    identity: &DirectHostIdentity,
) -> VmResult<bool>
where
    P: DispatchOriginPayload,
{
    match Option::<P>::from_script_arg(&result?)? {
        Some(payload) => {
            payload.validate_dispatch_origin(identity)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

pub fn validate_business_dispatch_origin_payload<P, E>(
    result: VmResult<OwnedValue>,
    identity: &DirectHostIdentity,
) -> Result<(), E>
where
    P: DispatchOriginPayload,
    E: FromScriptArg + From<VmError>,
{
    let value = result.map_err(E::from)?;
    match Result::<P, E>::from_script_arg(&value).map_err(E::from)? {
        Ok(payload) => payload.validate_dispatch_origin(identity).map_err(E::from),
        Err(error) => Err(error),
    }
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

fn validate_returned_origin(returned: HostRef, identity: &DirectHostIdentity) -> VmResult<()> {
    if identity.host_ref() == Some(returned) {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::TypeMismatch {
        operation: "replaceable borrowed return provenance",
    }))
}
