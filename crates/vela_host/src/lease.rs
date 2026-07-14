use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::{ArcRwLockReadGuard, ArcRwLockWriteGuard, RawRwLock, RwLock};

use crate::error::{HostError, HostErrorKind};
use crate::object::ScriptHostObject;
use crate::path::{HostPath, HostRef};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostLeaseKind {
    Shared,
    Exclusive,
}

pub type SharedHostLeaseCount = Arc<AtomicUsize>;
pub type MutableHostLeaseObject<'host> = &'host mut (dyn ScriptHostObject + Send + Sync);
pub type MutableHostLeaseSlot<'host> = Arc<RwLock<MutableHostLeaseObject<'host>>>;
pub type SharedMutableHostLease<'host> =
    ArcRwLockReadGuard<RawRwLock, MutableHostLeaseObject<'host>>;
pub type ExclusiveHostLease<'host> = ArcRwLockWriteGuard<RawRwLock, MutableHostLeaseObject<'host>>;

pub enum ErasedHostLease<'host> {
    SharedBorrowed {
        object: &'host (dyn ScriptHostObject + Sync),
        leases: SharedHostLeaseCount,
    },
    SharedMutable {
        object: SharedMutableHostLease<'host>,
    },
    Exclusive {
        object: ExclusiveHostLease<'host>,
    },
}

impl ErasedHostLease<'_> {
    #[must_use]
    pub fn object(&self) -> &dyn ScriptHostObject {
        match self {
            Self::SharedBorrowed { object, .. } => *object,
            Self::SharedMutable { object } => &***object,
            Self::Exclusive { object } => &***object,
        }
    }

    pub fn object_mut(&mut self) -> Option<&mut dyn ScriptHostObject> {
        match self {
            Self::SharedBorrowed { .. } | Self::SharedMutable { .. } => None,
            Self::Exclusive { object } => Some(&mut ***object),
        }
    }

    #[must_use]
    pub const fn is_exclusive(&self) -> bool {
        matches!(self, Self::Exclusive { .. })
    }
}

impl Drop for ErasedHostLease<'_> {
    fn drop(&mut self) {
        if let Self::SharedBorrowed { leases, .. } = self {
            let previous = leases.fetch_sub(1, Ordering::AcqRel);
            debug_assert!(previous > 0, "shared host lease count underflow");
        }
    }
}

#[must_use]
pub fn host_object_busy(root: HostRef) -> HostError {
    HostError::new(HostErrorKind::HostObjectBusy {
        path: HostPath::new(root),
    })
}

#[must_use]
pub fn host_lease_unsupported(root: HostRef) -> HostError {
    HostError::new(HostErrorKind::HostLeaseUnsupported {
        path: HostPath::new(root),
    })
}
