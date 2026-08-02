use std::sync::Arc;

use parking_lot::{
    ArcMutexGuard, ArcRwLockReadGuard, ArcRwLockWriteGuard, Mutex, RawMutex, RawRwLock, RwLock,
};

use crate::call_value::HostCallValue;
use crate::error::{HostError, HostErrorKind, HostResult};
use crate::object::ScriptHostObject;
use crate::path::{HostPath, HostRef};
use crate::resolved::{HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use crate::target::HostTargetInstance;
use crate::value::HostValue;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HostLeaseKind {
    Shared,
    Exclusive,
}

/// Lease requests stay inline for ordinary service arities and spill only for
/// unusually wide generated boundaries.
pub type HostLeaseRequestSet = smallvec::SmallVec<[(HostRef, HostLeaseKind); 8]>;

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

pub type MutableHostLeaseObject<'host> = &'host mut (dyn ScriptHostObject + Send);
pub type MutableHostLeaseSlot<'host> = Arc<Mutex<MutableHostLeaseObject<'host>>>;
pub type ExclusiveHostLease<'host> = ArcMutexGuard<RawMutex, MutableHostLeaseObject<'host>>;
pub type ScopedHostLeaseObject<'host> = Box<dyn ScriptHostObject + Send + Sync + 'host>;
pub type ScopedHostLeaseSlot<'host> = Arc<RwLock<ScopedHostLeaseObject<'host>>>;
pub type SharedScopedHostLease<'host> = ArcRwLockReadGuard<RawRwLock, ScopedHostLeaseObject<'host>>;
pub type ExclusiveScopedHostLease<'host> =
    ArcRwLockWriteGuard<RawRwLock, ScopedHostLeaseObject<'host>>;
pub type OwnedHostLeaseObject = Box<dyn ScriptHostObject + Send + Sync + 'static>;
pub type OwnedHostLeaseSlot = Arc<RwLock<OwnedHostLeaseObject>>;
pub type SharedOwnedHostLease = ArcRwLockReadGuard<RawRwLock, OwnedHostLeaseObject>;
pub type ExclusiveOwnedHostLease = ArcRwLockWriteGuard<RawRwLock, OwnedHostLeaseObject>;

pub enum ErasedHostLease<'host> {
    Vacant,
    SharedBorrowed {
        object: &'host (dyn ScriptHostObject + Sync),
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
    OwnedShared {
        object: SharedOwnedHostLease,
    },
    OwnedExclusive {
        object: ExclusiveOwnedHostLease,
    },
}

/// Acquired lease guards stay inline for ordinary service arities and spill
/// only for unusually wide generated boundaries.
pub type ErasedHostLeaseSet<'host> = smallvec::SmallVec<[ErasedHostLease<'host>; 8]>;

impl ErasedHostLease<'_> {
    #[must_use]
    pub fn object(&self) -> &dyn ScriptHostObject {
        match self {
            Self::Vacant => panic!("vacant host lease has no object"),
            Self::SharedBorrowed { object } => *object,
            Self::Exclusive { object } => &***object,
            Self::ScopedShared { object } => &***object,
            Self::ScopedExclusive { object } => &***object,
            Self::OwnedShared { object } => &***object,
            Self::OwnedExclusive { object } => &***object,
        }
    }

    pub fn object_mut(&mut self) -> Option<&mut dyn ScriptHostObject> {
        match self {
            Self::Vacant
            | Self::SharedBorrowed { .. }
            | Self::ScopedShared { .. }
            | Self::OwnedShared { .. } => None,
            Self::Exclusive { object } => Some(&mut ***object),
            Self::ScopedExclusive { object } => Some(&mut ***object),
            Self::OwnedExclusive { object } => Some(&mut ***object),
        }
    }

    /// Returns a shared object whose erased capability proves `Sync`.
    #[must_use]
    pub fn object_sync(&self) -> Option<&(dyn ScriptHostObject + Sync)> {
        match self {
            Self::SharedBorrowed { object } => Some(*object),
            Self::ScopedShared { object } => Some(&***object),
            Self::ScopedExclusive { object } => Some(&***object),
            Self::OwnedShared { object } => Some(&***object),
            Self::OwnedExclusive { object } => Some(&***object),
            Self::Vacant | Self::Exclusive { .. } => None,
        }
    }

    /// Returns an exclusive object whose erased capability proves `Send`.
    pub fn object_send_mut(&mut self) -> Option<&mut (dyn ScriptHostObject + Send)> {
        match self {
            Self::Exclusive { object } => Some(&mut ***object),
            Self::ScopedExclusive { object } => Some(&mut ***object),
            Self::OwnedExclusive { object } => Some(&mut ***object),
            Self::Vacant
            | Self::SharedBorrowed { .. }
            | Self::ScopedShared { .. }
            | Self::OwnedShared { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_exclusive(&self) -> bool {
        matches!(
            self,
            Self::Exclusive { .. } | Self::ScopedExclusive { .. } | Self::OwnedExclusive { .. }
        )
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

pub struct SharedScopedHost<'host, T: ?Sized>(&'host T, Option<vela_common::HostTypeId>);

impl<'host, T: ?Sized> SharedScopedHost<'host, T> {
    #[must_use]
    pub const fn new(value: &'host T) -> Self {
        Self(value, None)
    }

    #[must_use]
    pub const fn with_type_id(value: &'host T, type_id: vela_common::HostTypeId) -> Self {
        Self(value, Some(type_id))
    }
}

pub struct ExclusiveScopedHost<'host, T: ?Sized>(&'host mut T, Option<vela_common::HostTypeId>);

impl<'host, T: ?Sized> ExclusiveScopedHost<'host, T> {
    #[must_use]
    pub const fn new(value: &'host mut T) -> Self {
        Self(value, None)
    }

    #[must_use]
    pub const fn with_type_id(value: &'host mut T, type_id: vela_common::HostTypeId) -> Self {
        Self(value, Some(type_id))
    }
}

/// A call-scoped shared borrow of a Rust type whose Vela surface is supplied
/// entirely by an explicit type and method registration.
///
/// The wrapped Rust type does not implement [`ScriptHostObject`]. Vela keeps
/// only this erased object behind a [`HostRef`], while registered native
/// method thunks recover `&T` through [`ScriptHostObject::lease_any`].
pub struct RegisteredSharedHost<'host, T> {
    value: &'host T,
    type_id: vela_common::HostTypeId,
}

impl<'host, T> RegisteredSharedHost<'host, T> {
    #[must_use]
    pub const fn new(value: &'host T, type_id: vela_common::HostTypeId) -> Self {
        Self { value, type_id }
    }
}

/// Exclusive counterpart to [`RegisteredSharedHost`].
pub struct RegisteredExclusiveHost<'host, T> {
    value: &'host mut T,
    type_id: vela_common::HostTypeId,
}

impl<'host, T> RegisteredExclusiveHost<'host, T> {
    #[must_use]
    pub const fn new(value: &'host mut T, type_id: vela_common::HostTypeId) -> Self {
        Self { value, type_id }
    }
}

fn registered_host_read_error(target: HostTargetInstance<'_>) -> HostError {
    HostError {
        kind: HostErrorKind::MissingPath {
            path: target.to_diagnostic_path().to_host_path(),
        },
        source_span: None,
    }
}

impl<T> ScriptHostObject for RegisteredSharedHost<'_, T>
where
    T: Send + Sync + 'static,
{
    fn host_type_id(&self) -> vela_common::HostTypeId {
        self.type_id
    }

    fn lease_any(&self) -> Option<&dyn std::any::Any> {
        Some(self.value)
    }

    fn read_resolved_host(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        Err(registered_host_read_error(target))
    }
}

impl<T> ScriptHostObject for RegisteredExclusiveHost<'_, T>
where
    T: Send + Sync + 'static,
{
    fn host_type_id(&self) -> vela_common::HostTypeId {
        self.type_id
    }

    fn lease_any(&self) -> Option<&dyn std::any::Any> {
        Some(self.value)
    }

    fn lease_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self.value)
    }

    fn read_resolved_host(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        Err(registered_host_read_error(target))
    }
}

/// Erases a registered shared Rust borrow into one call-scoped Host object.
#[must_use]
pub fn registered_shared_host<T>(
    value: &T,
    type_id: vela_common::HostTypeId,
) -> ScopedHostDependent<'_>
where
    T: Send + Sync + 'static,
{
    Box::new(RegisteredSharedHost::new(value, type_id))
}

/// Erases a registered exclusive Rust borrow into one call-scoped Host object.
#[must_use]
pub fn registered_exclusive_host<T>(
    value: &mut T,
    type_id: vela_common::HostTypeId,
) -> ScopedHostDependent<'_>
where
    T: Send + Sync + 'static,
{
    Box::new(RegisteredExclusiveHost::new(value, type_id))
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
    T: ScriptHostObject + Send + Sync + ?Sized + 'static,
{
    Box::new(SharedScopedHost::new(value))
}

pub fn exclusive_scoped_host<T>(value: &mut T) -> ScopedHostDependent<'_>
where
    T: ScriptHostObject + Send + Sync + ?Sized + 'static,
{
    Box::new(ExclusiveScopedHost::new(value))
}

pub fn shared_scoped_host_with_type_id<T>(
    value: &T,
    type_id: vela_common::HostTypeId,
) -> ScopedHostDependent<'_>
where
    T: ScriptHostObject + Send + Sync + ?Sized + 'static,
{
    Box::new(SharedScopedHost::with_type_id(value, type_id))
}

pub fn exclusive_scoped_host_with_type_id<T>(
    value: &mut T,
    type_id: vela_common::HostTypeId,
) -> ScopedHostDependent<'_>
where
    T: ScriptHostObject + Send + Sync + ?Sized + 'static,
{
    Box::new(ExclusiveScopedHost::with_type_id(value, type_id))
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
            self.1.unwrap_or_else(|| self.0.host_type_id())
        }

        fn lease_any(&self) -> Option<&dyn std::any::Any> {
            self.0.lease_any()
        }

        fn erased_slice_ref(&self) -> Option<crate::erased_slice::ErasedSliceRef<'_>> {
            self.0.erased_slice_ref()
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

        fn borrow_resolved_host_shared(
            &self,
            access: ResolvedHostAccess,
            target: HostTargetInstance<'_>,
        ) -> crate::error::HostResult<Option<ScopedHostDependent<'_>>> {
            self.0.borrow_resolved_host_shared(access, target)
        }

        fn borrow_collection_resolved_host_shared(
            &self,
            access: ResolvedHostAccess,
            target: HostTargetInstance<'_>,
            projection: crate::protocol::HostCollectionProjection,
        ) -> crate::error::HostResult<Option<crate::object::ScopedHostCollectionDependents<'_>>> {
            self.0
                .borrow_collection_resolved_host_shared(access, target, projection)
        }

        fn query_collection_resolved_host(
            &self,
            access: ResolvedHostAccess,
            target: HostTargetInstance<'_>,
            query: crate::protocol::HostCollectionQuery,
        ) -> crate::error::HostResult<HostValue> {
            self.0.query_collection_resolved_host(access, target, query)
        }

        fn snapshot_collection_resolved_host(
            &self,
            access: ResolvedHostAccess,
            target: HostTargetInstance<'_>,
            projection: crate::protocol::HostCollectionProjection,
        ) -> crate::error::HostResult<crate::protocol::HostCollectionSnapshot> {
            self.0
                .snapshot_collection_resolved_host(access, target, projection)
        }
    };
}

impl<T: ?Sized> ScriptHostObject for SharedScopedHost<'_, T>
where
    T: ScriptHostObject + Send + Sync + 'static,
{
    impl_scoped_host_common!(SharedScopedHost);

    fn mutate_collection_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _mutation: crate::protocol::HostCollectionMutation<'_>,
    ) -> crate::error::HostResult<()> {
        Err(scoped_read_only_error(target, "write"))
    }

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
        _args: &[HostCallValue],
    ) -> crate::error::HostResult<HostCallValue> {
        Err(scoped_read_only_error(target, "call"))
    }
}

impl<T: ?Sized> ScriptHostObject for ExclusiveScopedHost<'_, T>
where
    T: ScriptHostObject + Send + Sync + 'static,
{
    impl_scoped_host_common!(ExclusiveScopedHost);

    fn lease_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        self.0.lease_any_mut()
    }

    fn erased_slice_mut(&mut self) -> Option<crate::erased_slice::ErasedSliceMut<'_>> {
        self.0.erased_slice_mut()
    }

    fn borrow_resolved_host_exclusive(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> crate::error::HostResult<Option<ScopedHostDependent<'_>>> {
        self.0.borrow_resolved_host_exclusive(access, target)
    }

    fn borrow_collection_resolved_host_exclusive(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: crate::protocol::HostCollectionProjection,
    ) -> crate::error::HostResult<Option<crate::object::ScopedHostCollectionDependents<'_>>> {
        self.0
            .borrow_collection_resolved_host_exclusive(access, target, projection)
    }

    fn mutate_collection_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        mutation: crate::protocol::HostCollectionMutation<'_>,
    ) -> crate::error::HostResult<()> {
        self.0
            .mutate_collection_resolved_host(access, target, mutation)
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
        args: &[HostCallValue],
    ) -> crate::error::HostResult<HostCallValue> {
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

    fn erased_slice_ref(&self) -> Option<crate::erased_slice::ErasedSliceRef<'_>> {
        self.borrow_dependent().erased_slice_ref()
    }

    fn erased_slice_mut(&mut self) -> Option<crate::erased_slice::ErasedSliceMut<'_>> {
        self.with_dependent_mut(|_, object| object.erased_slice_mut())
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

    fn borrow_resolved_host_shared(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> crate::error::HostResult<Option<ScopedHostDependent<'_>>> {
        self.borrow_dependent()
            .borrow_resolved_host_shared(access, target)
    }

    fn borrow_resolved_host_exclusive(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> crate::error::HostResult<Option<ScopedHostDependent<'_>>> {
        self.with_dependent_mut(|_, object| object.borrow_resolved_host_exclusive(access, target))
    }

    fn borrow_collection_resolved_host_shared(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: crate::protocol::HostCollectionProjection,
    ) -> crate::error::HostResult<Option<crate::object::ScopedHostCollectionDependents<'_>>> {
        self.borrow_dependent()
            .borrow_collection_resolved_host_shared(access, target, projection)
    }

    fn borrow_collection_resolved_host_exclusive(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: crate::protocol::HostCollectionProjection,
    ) -> crate::error::HostResult<Option<crate::object::ScopedHostCollectionDependents<'_>>> {
        self.with_dependent_mut(|_, object| {
            object.borrow_collection_resolved_host_exclusive(access, target, projection)
        })
    }

    fn query_collection_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        query: crate::protocol::HostCollectionQuery,
    ) -> crate::error::HostResult<HostValue> {
        self.borrow_dependent()
            .query_collection_resolved_host(access, target, query)
    }

    fn snapshot_collection_resolved_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: crate::protocol::HostCollectionProjection,
    ) -> crate::error::HostResult<crate::protocol::HostCollectionSnapshot> {
        self.borrow_dependent()
            .snapshot_collection_resolved_host(access, target, projection)
    }

    fn mutate_collection_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        mutation: crate::protocol::HostCollectionMutation<'_>,
    ) -> crate::error::HostResult<()> {
        self.with_dependent_mut(|_, object| {
            object.mutate_collection_resolved_host(access, target, mutation)
        })
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
        args: &[HostCallValue],
    ) -> crate::error::HostResult<HostCallValue> {
        self.with_dependent_mut(|_, object| object.call_resolved_host(access, target, method, args))
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
