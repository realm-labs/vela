//! Quarantined typed reborrow boundary for generated Service default thunks.
//!
//! A Service call reaches this module only after the VM has resolved an
//! invocation-local root HostRef and acquired the requested lease. Generated
//! code supplies the exact authored Rust parameter type and its sealed
//! HostTypeId. We validate that identity against both the handle and the
//! leased object before reconstructing a reference. Only direct borrowed roots
//! are admitted; scoped children and Runtime-owned objects stay on their
//! ordinary static extraction paths.

use crate::error::HostResult;
use crate::lease::{ErasedHostLease, host_lease_unsupported};
use crate::object::ScriptHostObject;
use crate::path::HostRef;

/// Reborrows one direct root as the exact generated shared Service parameter.
///
/// # Safety
///
/// T must be the concrete object type originally inserted for root. The
/// generated Service thunk establishes this by construction: the outer typed
/// adapter inserts that parameter, the sealed schema supplies its stable
/// HostTypeId, and lease acquisition preserves the root/object pair until
/// this borrow ends.
pub unsafe fn shared<'lease, T>(
    lease: &'lease ErasedHostLease<'_>,
    root: HostRef,
    expected_type: vela_common::HostTypeId,
) -> HostResult<&'lease T>
where
    T: ScriptHostObject + Sized,
{
    let object: &dyn ScriptHostObject = match lease {
        ErasedHostLease::SharedBorrowed { object, .. } => *object,
        ErasedHostLease::Exclusive { object } => &***object,
        ErasedHostLease::Vacant
        | ErasedHostLease::ScopedShared { .. }
        | ErasedHostLease::ScopedExclusive { .. }
        | ErasedHostLease::OwnedShared { .. }
        | ErasedHostLease::OwnedExclusive { .. } => {
            return Err(host_lease_unsupported(root));
        }
    };
    validate_identity(object, root, expected_type)?;
    let data = std::ptr::from_ref(object).cast::<()>();
    // SAFETY: the caller contract plus validate_identity prove that the
    // direct-root trait object was created from this exact T. The returned
    // reference is tied to the lease borrow, so it cannot outlive the acquired
    // guard or the invocation-local root.
    Ok(unsafe { &*data.cast::<T>() })
}

/// Reborrows one direct exclusive root as the exact generated mutable Service
/// parameter.
///
/// # Safety
///
/// The requirements of shared apply, and lease must be the unique exclusive
/// lease for root.
pub unsafe fn exclusive<'lease, T>(
    lease: &'lease mut ErasedHostLease<'_>,
    root: HostRef,
    expected_type: vela_common::HostTypeId,
) -> HostResult<&'lease mut T>
where
    T: ScriptHostObject + Sized,
{
    let ErasedHostLease::Exclusive { object } = lease else {
        return Err(host_lease_unsupported(root));
    };
    let object: &mut dyn ScriptHostObject = &mut ***object;
    validate_identity(object, root, expected_type)?;
    let data = std::ptr::from_mut(object).cast::<()>();
    // SAFETY: the caller contract, exact identity checks, and the exclusive
    // lease prove both the concrete pointee type and unique mutable access for
    // the duration of the returned borrow.
    Ok(unsafe { &mut *data.cast::<T>() })
}

fn validate_identity(
    object: &dyn ScriptHostObject,
    root: HostRef,
    expected_type: vela_common::HostTypeId,
) -> HostResult<()> {
    if root.type_id != expected_type || object.host_type_id() != expected_type {
        return Err(host_lease_unsupported(root));
    }
    Ok(())
}
