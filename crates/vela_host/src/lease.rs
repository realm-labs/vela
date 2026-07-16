use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::{ArcRwLockReadGuard, ArcRwLockWriteGuard, RawRwLock, RwLock};

use crate::error::{HostError, HostErrorKind};
use crate::object::ScriptHostObject;
use crate::path::{HostPath, HostRef};
use crate::resolved::{HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use crate::target::HostTargetInstance;
use crate::value::HostValue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostLeaseKind {
    Shared,
    Exclusive,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BorrowLeaseId(u64);

impl BorrowLeaseId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

pub type SharedHostLeaseCount = Arc<AtomicUsize>;
pub type MutableHostLeaseObject<'host> = &'host mut (dyn ScriptHostObject + Send + Sync);
pub type MutableHostLeaseSlot<'host> = Arc<RwLock<MutableHostLeaseObject<'host>>>;
pub type SharedMutableHostLease<'host> =
    ArcRwLockReadGuard<RawRwLock, MutableHostLeaseObject<'host>>;
pub type ExclusiveHostLease<'host> = ArcRwLockWriteGuard<RawRwLock, MutableHostLeaseObject<'host>>;
pub type ScopedHostLeaseObject<'host> = Box<dyn ScriptHostObject + Send + Sync + 'host>;
pub type ScopedHostLeaseSlot<'host> = Arc<RwLock<ScopedHostLeaseObject<'host>>>;
pub type SharedScopedHostLease<'host> = ArcRwLockReadGuard<RawRwLock, ScopedHostLeaseObject<'host>>;
pub type ExclusiveScopedHostLease<'host> =
    ArcRwLockWriteGuard<RawRwLock, ScopedHostLeaseObject<'host>>;

pub enum ErasedHostLease<'host> {
    Vacant,
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
    ScopedShared {
        object: SharedScopedHostLease<'host>,
    },
    ScopedExclusive {
        object: ExclusiveScopedHostLease<'host>,
    },
}

impl ErasedHostLease<'_> {
    #[must_use]
    pub fn object(&self) -> &dyn ScriptHostObject {
        match self {
            Self::Vacant => panic!("vacant host lease has no object"),
            Self::SharedBorrowed { object, .. } => *object,
            Self::SharedMutable { object } => &***object,
            Self::Exclusive { object } => &***object,
            Self::ScopedShared { object } => &***object,
            Self::ScopedExclusive { object } => &***object,
        }
    }

    pub fn object_mut(&mut self) -> Option<&mut dyn ScriptHostObject> {
        match self {
            Self::Vacant
            | Self::SharedBorrowed { .. }
            | Self::SharedMutable { .. }
            | Self::ScopedShared { .. } => None,
            Self::Exclusive { object } => Some(&mut ***object),
            Self::ScopedExclusive { object } => Some(&mut ***object),
        }
    }

    #[must_use]
    pub const fn is_exclusive(&self) -> bool {
        matches!(self, Self::Exclusive { .. } | Self::ScopedExclusive { .. })
    }

    #[must_use]
    pub fn take(&mut self) -> Self {
        std::mem::replace(self, Self::Vacant)
    }
}

pub type ScopedHostDependent<'host> = Box<dyn ScriptHostObject + Send + Sync + 'host>;

self_cell::self_cell!(
    /// A movable, safe owner/dependent cell retaining the parent host lease.
    pub struct ScopedBorrowedHostCell<'host> {
        owner: self_cell::MutBorrow<ErasedHostLease<'host>>,

        #[covariant]
        dependent: ScopedHostDependent,
    }
);

pub type ScopedHostGroupDependents<'host> = Vec<ScopedHostLeaseSlot<'host>>;

self_cell::self_cell!(
    /// One retained parent lease with multiple independently leased children.
    pub struct ScopedBorrowedHostGroupCell<'host> {
        owner: self_cell::MutBorrow<ErasedHostLease<'host>>,

        #[not_covariant]
        dependent: ScopedHostGroupDependents,
    }
);

pub struct SharedScopedHost<'host, T>(&'host T);

impl<'host, T> SharedScopedHost<'host, T> {
    #[must_use]
    pub const fn new(value: &'host T) -> Self {
        Self(value)
    }
}

pub struct ExclusiveScopedHost<'host, T>(&'host mut T);

impl<'host, T> ExclusiveScopedHost<'host, T> {
    #[must_use]
    pub const fn new(value: &'host mut T) -> Self {
        Self(value)
    }
}

pub fn try_scoped_host_cell<'host, Error>(
    parent_lease: ErasedHostLease<'host>,
    build: impl for<'borrow> FnOnce(
        &'borrow mut ErasedHostLease<'host>,
    ) -> Result<ScopedHostDependent<'borrow>, Error>,
) -> Result<ScopedBorrowedHostCell<'host>, Error> {
    ScopedBorrowedHostCell::try_new(self_cell::MutBorrow::new(parent_lease), |owner| {
        build(owner.borrow_mut())
    })
}

pub fn try_scoped_host_group_cell<'host, Error>(
    parent_lease: ErasedHostLease<'host>,
    build: impl for<'borrow> FnOnce(
        &'borrow mut ErasedHostLease<'host>,
    ) -> Result<Vec<ScopedHostDependent<'borrow>>, Error>,
) -> Result<ScopedBorrowedHostGroupCell<'host>, Error> {
    ScopedBorrowedHostGroupCell::try_new(self_cell::MutBorrow::new(parent_lease), |owner| {
        build(owner.borrow_mut()).map(|objects| {
            objects
                .into_iter()
                .map(|object| Arc::new(RwLock::new(object)))
                .collect()
        })
    })
}

impl ScopedBorrowedHostGroupCell<'_> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.with_dependent(|_, objects| objects.len())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.with_dependent(|_, objects| objects.is_empty())
    }

    #[must_use]
    pub fn child_type_id(&self, index: usize) -> Option<vela_common::HostTypeId> {
        self.with_dependent(|_, objects| {
            objects
                .get(index)
                .and_then(|object| object.try_read())
                .map(|object| object.host_type_id())
        })
    }
}

pub fn shared_scoped_host<T>(value: &T) -> ScopedHostDependent<'_>
where
    T: ScriptHostObject + Send + Sync + 'static,
{
    Box::new(SharedScopedHost::new(value))
}

pub fn exclusive_scoped_host<T>(value: &mut T) -> ScopedHostDependent<'_>
where
    T: ScriptHostObject + Send + Sync + 'static,
{
    Box::new(ExclusiveScopedHost::new(value))
}

fn scoped_read_only_error(
    target: HostTargetInstance<'_>,
    action: &'static str,
) -> crate::error::HostError {
    crate::error::HostError::new(crate::error::HostErrorKind::PermissionDenied {
        path: target.to_diagnostic_path().to_host_path(),
        action,
    })
}

macro_rules! impl_scoped_host_common {
    ($wrapper:ident) => {
        fn host_type_id(&self) -> vela_common::HostTypeId {
            self.0.host_type_id()
        }

        fn lease_any(&self) -> Option<&dyn std::any::Any> {
            self.0.lease_any()
        }

        fn resolve_host_target(
            &self,
            spec: HostAccessSpec<'_>,
        ) -> crate::error::HostResult<ResolvedHostAccess> {
            self.0.resolve_host_target(spec)
        }

        fn read_resolved_host(
            &self,
            access: ResolvedHostAccess,
            target: HostTargetInstance<'_>,
        ) -> crate::error::HostResult<HostValue> {
            self.0.read_resolved_host(access, target)
        }
    };
}

impl<T> ScriptHostObject for SharedScopedHost<'_, T>
where
    T: ScriptHostObject + Send + Sync + 'static,
{
    impl_scoped_host_common!(SharedScopedHost);

    fn write_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _value: HostValue,
    ) -> crate::error::HostResult<()> {
        Err(scoped_read_only_error(target, "write"))
    }

    fn mutate_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _op: HostMutationOp,
        _rhs: HostValue,
    ) -> crate::error::HostResult<()> {
        Err(scoped_read_only_error(target, "write"))
    }

    fn remove_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> crate::error::HostResult<()> {
        Err(scoped_read_only_error(target, "write"))
    }

    fn call_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _method: vela_common::HostMethodId,
        _args: &[HostValue],
    ) -> crate::error::HostResult<HostValue> {
        Err(scoped_read_only_error(target, "call"))
    }
}

impl<T> ScriptHostObject for ExclusiveScopedHost<'_, T>
where
    T: ScriptHostObject + Send + Sync + 'static,
{
    impl_scoped_host_common!(ExclusiveScopedHost);

    fn lease_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        self.0.lease_any_mut()
    }

    fn write_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> crate::error::HostResult<()> {
        self.0.write_resolved_host(access, target, value)
    }

    fn mutate_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> crate::error::HostResult<()> {
        self.0.mutate_resolved_host(access, target, op, rhs)
    }

    fn remove_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> crate::error::HostResult<()> {
        self.0.remove_resolved_host(access, target)
    }

    fn call_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: vela_common::HostMethodId,
        args: &[HostValue],
    ) -> crate::error::HostResult<HostValue> {
        self.0.call_resolved_host(access, target, method, args)
    }
}

impl ScriptHostObject for ScopedBorrowedHostCell<'_> {
    fn host_type_id(&self) -> vela_common::HostTypeId {
        self.borrow_dependent().host_type_id()
    }

    fn lease_any(&self) -> Option<&dyn std::any::Any> {
        self.borrow_dependent().lease_any()
    }

    fn lease_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        self.with_dependent_mut(|_, object| object.lease_any_mut())
    }

    fn resolve_host_target(
        &self,
        spec: HostAccessSpec<'_>,
    ) -> crate::error::HostResult<ResolvedHostAccess> {
        self.borrow_dependent().resolve_host_target(spec)
    }

    fn read_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> crate::error::HostResult<HostValue> {
        self.borrow_dependent().read_resolved_host(access, target)
    }

    fn write_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> crate::error::HostResult<()> {
        self.with_dependent_mut(|_, object| object.write_resolved_host(access, target, value))
    }

    fn mutate_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> crate::error::HostResult<()> {
        self.with_dependent_mut(|_, object| object.mutate_resolved_host(access, target, op, rhs))
    }

    fn remove_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> crate::error::HostResult<()> {
        self.with_dependent_mut(|_, object| object.remove_resolved_host(access, target))
    }

    fn call_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: vela_common::HostMethodId,
        args: &[HostValue],
    ) -> crate::error::HostResult<HostValue> {
        self.with_dependent_mut(|_, object| object.call_resolved_host(access, target, method, args))
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
