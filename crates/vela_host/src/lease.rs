use crate::error::{HostError, HostErrorKind};
use crate::object::ScriptHostObject;
use crate::path::{HostPath, HostRef};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostLeaseKind {
    Shared,
    Exclusive,
}

pub type SharedHostLeaseCount = Arc<AtomicUsize>;
pub type ExclusiveHostLeaseSlot<'host> =
    Arc<Mutex<Option<&'host mut (dyn ScriptHostObject + Send)>>>;

pub enum ErasedHostLease<'host> {
    Shared {
        object: &'host (dyn ScriptHostObject + Sync),
        leases: SharedHostLeaseCount,
    },
    Exclusive {
        object: Option<&'host mut (dyn ScriptHostObject + Send)>,
        slot: ExclusiveHostLeaseSlot<'host>,
    },
}

impl ErasedHostLease<'_> {
    #[must_use]
    pub fn object(&self) -> &dyn ScriptHostObject {
        match self {
            Self::Shared { object, .. } => *object,
            Self::Exclusive {
                object: Some(object),
                ..
            } => &**object,
            Self::Exclusive { object: None, .. } => {
                unreachable!("exclusive host lease always owns its object")
            }
        }
    }

    pub fn object_mut(&mut self) -> Option<&mut dyn ScriptHostObject> {
        match self {
            Self::Shared { .. } => None,
            Self::Exclusive {
                object: Some(object),
                ..
            } => Some(&mut **object),
            Self::Exclusive { object: None, .. } => None,
        }
    }

    #[must_use]
    pub const fn is_exclusive(&self) -> bool {
        matches!(self, Self::Exclusive { .. })
    }
}

impl Drop for ErasedHostLease<'_> {
    fn drop(&mut self) {
        match self {
            Self::Shared { leases, .. } => {
                let previous = leases.fetch_sub(1, Ordering::AcqRel);
                debug_assert!(previous > 0, "shared host lease count underflow");
            }
            Self::Exclusive { object, slot } => {
                if let Some(object) = object.take() {
                    let mut stored = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    debug_assert!(stored.is_none(), "exclusive host lease slot was replaced");
                    *stored = Some(object);
                }
            }
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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
