use vela_common::{HostObjectId, HostTypeId};
use vela_host::error::HostResult;
use vela_host::lease::{BorrowLeaseId, host_lease_unsupported, host_object_busy};
use vela_host::object::ScriptHostObject;
use vela_host::path::{HostRef, HostSlotRef};

use super::{
    ExecutionHost, SCOPED_HOST_OBJECT_ID_BASE, ScopedHostBinding, ScopedHostObjectBinding,
};

impl<'state, 'host> ExecutionHost<'state, 'host> {
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
