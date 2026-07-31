use std::sync::Arc;

use smallvec::SmallVec;
use vela_common::{HostObjectId, HostTypeId};
use vela_host::adapter::{HostLeaseInvoker, ScopedHostReturn, ScopedHostReturnGroup};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::lease::{
    BorrowLeaseId, ErasedHostLease, ErasedHostLeaseSet, HostLeaseKind, host_lease_unsupported,
    host_object_busy,
};
use vela_host::object::ScriptHostObject;
use vela_host::path::{HostRef, HostSlotRef};

use super::{
    ExecutionHost, SCOPED_HOST_OBJECT_ID_BASE, ScopedHostBinding, ScopedHostObjectBinding,
};

impl<'state, 'host> ExecutionHost<'state, 'host> {
    pub(super) fn retain_scoped_host(&mut self, returned: ScopedHostReturn<'host>) -> HostRef {
        self.retain_scoped_host_with_parent_activity(returned, None, None)
    }

    pub(super) fn retain_scoped_host_with_parent_activity(
        &mut self,
        returned: ScopedHostReturn<'host>,
        parent_activity: Option<Arc<()>>,
        parent: Option<HostRef>,
    ) -> HostRef {
        let type_id = returned.object.host_type_id();
        let handle = self.scoped_hosts.insert_with(|handle| ScopedHostBinding {
            borrow_lease_id: Self::borrow_lease_id(handle),
            type_id,
            access: returned.access,
            object: ScopedHostObjectBinding::Single(Arc::new(parking_lot::RwLock::new(Box::new(
                returned.object,
            )))),
            activity: Arc::new(()),
            _parent_activity: parent_activity,
            parent,
        });
        Self::scoped_root(handle, type_id)
    }

    pub(super) fn retain_scoped_host_group(
        &mut self,
        returned: ScopedHostReturnGroup<'host>,
    ) -> HostResult<Vec<HostRef>> {
        self.retain_scoped_host_group_with_parent_activity(returned, None, None)
    }

    pub(super) fn retain_scoped_host_group_with_parent_activity(
        &mut self,
        returned: ScopedHostReturnGroup<'host>,
        parent_activity: Option<Arc<()>>,
        parent: Option<HostRef>,
    ) -> HostResult<Vec<HostRef>> {
        if returned.object.len() != returned.accesses.len() || returned.object.is_empty() {
            return Err(HostError {
                kind: HostErrorKind::InvalidArgument {
                    expected: "matching non-empty scoped host group children and access modes",
                },
                source_span: None,
            });
        }
        let object = Arc::new(returned.object);
        let mut roots = Vec::with_capacity(returned.accesses.len());
        for (index, access) in returned.accesses.into_iter().enumerate() {
            let type_id = object.child_type_id(index).ok_or(HostError {
                kind: HostErrorKind::InvalidArgument {
                    expected: "uncontended scoped host group child",
                },
                source_span: None,
            })?;
            let handle = self.scoped_hosts.insert_with(|handle| ScopedHostBinding {
                borrow_lease_id: Self::borrow_lease_id(handle),
                type_id,
                access,
                object: ScopedHostObjectBinding::Group {
                    object: Arc::clone(&object),
                    index,
                },
                activity: Arc::new(()),
                _parent_activity: parent_activity.clone(),
                parent,
            });
            roots.push(Self::scoped_root(handle, type_id));
        }
        Ok(roots)
    }

    pub(super) fn scoped_activity(&self, root: HostRef) -> Option<Arc<()>> {
        self.scoped_binding(root)
            .map(|binding| Arc::clone(&binding.activity))
    }

    pub(super) fn with_group_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        invoke: &mut HostLeaseInvoker<'_>,
    ) -> Option<HostResult<()>> {
        let mut group = None;
        let mut children = SmallVec::<[(HostRef, usize, HostLeaseKind, Arc<()>); 8]>::with_capacity(
            requests.len(),
        );
        for (root, kind) in requests {
            let binding = self.scoped_binding(*root)?;
            let ScopedHostObjectBinding::Group { object, index } = &binding.object else {
                return None;
            };
            if let Some(group) = &group {
                if !Arc::ptr_eq(group, object) {
                    return None;
                }
            } else {
                group = Some(Arc::clone(object));
            }
            if binding.access == HostLeaseKind::Shared && *kind == HostLeaseKind::Exclusive {
                return Some(Err(host_object_busy(*root)));
            }
            children.push((*root, *index, *kind, Arc::clone(&binding.activity)));
        }
        let group = group?;
        Some(group.with_dependent(move |_, objects| {
            let mut leases = ErasedHostLeaseSet::with_capacity(children.len());
            let mut activities = SmallVec::<[Arc<()>; 8]>::with_capacity(children.len());
            for (root, index, kind, activity) in children {
                let child = objects
                    .get(index)
                    .ok_or_else(|| host_lease_unsupported(root))?;
                let lease = match kind {
                    HostLeaseKind::Shared => child
                        .try_read_arc()
                        .map(|object| ErasedHostLease::ScopedShared { object })
                        .ok_or_else(|| host_object_busy(root))?,
                    HostLeaseKind::Exclusive => child
                        .try_write_arc()
                        .map(|object| ErasedHostLease::ScopedExclusive { object })
                        .ok_or_else(|| host_object_busy(root))?,
                };
                leases.push(lease);
                activities.push(activity);
            }
            let _activities = activities;
            invoke(&mut leases, self)
        }))
    }

    pub(super) fn borrow_lease_id(handle: HostSlotRef) -> BorrowLeaseId {
        BorrowLeaseId::new((u64::from(handle.generation()) << 32) | u64::from(handle.slot()))
    }

    pub(super) fn scoped_handle(&self, root: HostRef) -> Option<HostSlotRef> {
        let slot = root
            .object_id
            .get()
            .checked_sub(SCOPED_HOST_OBJECT_ID_BASE)
            .and_then(|slot| u32::try_from(slot).ok())?;
        let handle = HostSlotRef::new(slot, root.generation);
        let binding = self.scoped_hosts.get(handle)?;
        (binding.type_id == root.type_id).then_some(handle)
    }

    pub(super) fn scoped_binding(&self, root: HostRef) -> Option<&ScopedHostBinding<'host>> {
        self.scoped_hosts.get(self.scoped_handle(root)?)
    }

    #[cfg(test)]
    pub(super) fn scoped_borrow_lease_id(&self, root: HostRef) -> Option<BorrowLeaseId> {
        self.scoped_binding(root)
            .map(|binding| binding.borrow_lease_id)
    }

    pub(super) fn scoped_binding_mut(
        &mut self,
        root: HostRef,
    ) -> Option<&mut ScopedHostBinding<'host>> {
        let handle = self.scoped_handle(root)?;
        self.scoped_hosts.get_mut(handle)
    }

    pub(super) fn scoped_root(handle: HostSlotRef, type_id: HostTypeId) -> HostRef {
        HostRef::new(
            type_id,
            HostObjectId::new(SCOPED_HOST_OBJECT_ID_BASE + u64::from(handle.slot())),
            handle.generation(),
        )
    }

    pub(super) fn inspect_scoped_host<T>(
        &self,
        root: HostRef,
        inspect: impl FnOnce(&dyn ScriptHostObject) -> HostResult<T>,
    ) -> Option<HostResult<T>> {
        let binding = self.scoped_binding(root)?;
        Some(match &binding.object {
            ScopedHostObjectBinding::Single(object) => object
                .try_read()
                .ok_or_else(|| host_object_busy(root))
                .and_then(|object| inspect(&**object)),
            ScopedHostObjectBinding::IteratorLease { lease, .. } => lease
                .try_lock()
                .ok_or_else(|| host_object_busy(root))
                .and_then(|lease| inspect(lease.object())),
            ScopedHostObjectBinding::Group { object, index } => {
                object.with_dependent(|_, objects| {
                    objects
                        .get(*index)
                        .ok_or_else(|| host_lease_unsupported(root))?
                        .try_read()
                        .ok_or_else(|| host_object_busy(root))
                        .and_then(|object| inspect(&**object))
                })
            }
        })
    }

    pub(super) fn mutate_scoped_host<T>(
        &mut self,
        root: HostRef,
        mutate: impl FnOnce(&mut dyn ScriptHostObject) -> HostResult<T>,
    ) -> Option<HostResult<T>> {
        let binding = self.scoped_binding_mut(root)?;
        Some(match &mut binding.object {
            ScopedHostObjectBinding::Single(object) => object
                .try_write()
                .ok_or_else(|| host_object_busy(root))
                .and_then(|mut object| mutate(&mut **object)),
            ScopedHostObjectBinding::IteratorLease { .. } => Err(host_object_busy(root)),
            ScopedHostObjectBinding::Group { object, index } => {
                object.with_dependent(|_, objects| {
                    objects
                        .get(*index)
                        .ok_or_else(|| host_lease_unsupported(root))?
                        .try_write()
                        .ok_or_else(|| host_object_busy(root))
                        .and_then(|mut object| mutate(&mut **object))
                })
            }
        })
    }
}
