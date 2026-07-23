use std::sync::Arc;

use parking_lot::RwLock;
use vela_common::{HostMethodId, HostObjectId, HostTypeId};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::lease::{ErasedHostLease, HostLeaseKind, OwnedHostLeaseSlot, host_object_busy};
use vela_host::object::ScriptHostObject;
use vela_host::path::{HostRef, HostSlotRef};
use vela_host::protocol::{
    HostCollectionMutation, HostCollectionProjection, HostCollectionQuery, HostCollectionSnapshot,
};
use vela_host::resolved::{HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use vela_host::slot::HostSlotTable;
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;

const OWNED_HOST_OBJECT_ID_BASE: u64 = 1 << 61;

/// Runtime-owned Rust objects created by registered host factories.
///
/// The script heap stores only compact `HostRef` handles. Objects live here
/// until their owning Runtime is dropped; the script GC neither owns nor
/// traces Rust state.
pub(super) struct RuntimeHostArena {
    objects: HostSlotTable<RuntimeHostObject>,
}

struct RuntimeHostObject {
    type_id: HostTypeId,
    object: OwnedHostLeaseSlot,
}

impl RuntimeHostArena {
    pub(super) fn new() -> Self {
        Self {
            objects: HostSlotTable::new(),
        }
    }

    pub(super) fn retain(&mut self, object: Box<dyn ScriptHostObject + Send + Sync>) -> HostRef {
        let type_id = object.host_type_id();
        let handle = self.objects.insert(RuntimeHostObject {
            type_id,
            object: Arc::new(RwLock::new(object)),
        });
        Self::root_for(handle, type_id)
    }

    pub(super) fn contains(&self, root: HostRef) -> bool {
        self.entry(root).is_some()
    }

    pub(super) fn take_lease<'host>(
        &self,
        root: HostRef,
        kind: HostLeaseKind,
    ) -> HostResult<ErasedHostLease<'host>> {
        let object = self.object(root)?;
        match kind {
            HostLeaseKind::Shared => object
                .try_read_arc()
                .map(|object| ErasedHostLease::OwnedShared { object })
                .ok_or_else(|| host_object_busy(root)),
            HostLeaseKind::Exclusive => object
                .try_write_arc()
                .map(|object| ErasedHostLease::OwnedExclusive { object })
                .ok_or_else(|| host_object_busy(root)),
        }
    }

    pub(super) fn resolve(
        &self,
        spec: HostAccessSpec<'_>,
    ) -> Option<HostResult<ResolvedHostAccess>> {
        let (root, object) = self
            .objects
            .iter()
            .find(|(_, object)| object.type_id == spec.plan.root_type)
            .map(|(handle, object)| (Self::root_for(handle, object.type_id), &object.object))?;
        Some(
            object
                .try_read()
                .ok_or_else(|| host_object_busy(root))
                .and_then(|object| object.resolve_host_target(spec)),
        )
    }

    pub(super) fn read(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> Option<HostResult<HostValue>> {
        let object = &self.entry(target.root)?.object;
        Some(
            object
                .try_read()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|object| object.read_resolved_host(access, target)),
        )
    }

    pub(super) fn query_collection(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        query: HostCollectionQuery,
    ) -> Option<HostResult<HostValue>> {
        let object = &self.entry(target.root)?.object;
        Some(
            object
                .try_read()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|object| object.query_collection_resolved_host(access, target, query)),
        )
    }

    pub(super) fn snapshot_collection(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> Option<HostResult<HostCollectionSnapshot>> {
        let object = &self.entry(target.root)?.object;
        Some(
            object
                .try_read()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|object| {
                    object.snapshot_collection_resolved_host(access, target, projection)
                }),
        )
    }

    pub(super) fn mutate_collection(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        mutation: HostCollectionMutation<'_>,
    ) -> Option<HostResult<()>> {
        let object = &self.entry(target.root)?.object;
        Some(
            object
                .try_write()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|mut object| {
                    object.mutate_collection_resolved_host(access, target, mutation)
                }),
        )
    }

    pub(super) fn write(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> Option<HostResult<()>> {
        let object = &self.entry(target.root)?.object;
        Some(
            object
                .try_write()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|mut object| object.write_resolved_host(access, target, value)),
        )
    }

    pub(super) fn mutate(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> Option<HostResult<()>> {
        let object = &self.entry(target.root)?.object;
        Some(
            object
                .try_write()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|mut object| object.mutate_resolved_host(access, target, op, rhs)),
        )
    }

    pub(super) fn remove(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> Option<HostResult<()>> {
        let object = &self.entry(target.root)?.object;
        Some(
            object
                .try_write()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|mut object| object.remove_resolved_host(access, target)),
        )
    }

    pub(super) fn call(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> Option<HostResult<HostValue>> {
        let object = &self.entry(target.root)?.object;
        Some(
            object
                .try_write()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|mut object| object.call_resolved_host(access, target, method, args)),
        )
    }

    fn object(&self, root: HostRef) -> HostResult<&OwnedHostLeaseSlot> {
        self.entry(root)
            .map(|entry| &entry.object)
            .ok_or_else(|| HostError {
                kind: HostErrorKind::MissingPath {
                    path: vela_host::path::HostPath::new(root),
                },
                source_span: None,
            })
    }

    fn entry(&self, root: HostRef) -> Option<&RuntimeHostObject> {
        let slot = root
            .object_id
            .get()
            .checked_sub(OWNED_HOST_OBJECT_ID_BASE)
            .and_then(|slot| u32::try_from(slot).ok())?;
        let entry = self.objects.get(HostSlotRef::new(slot, root.generation))?;
        (entry.type_id == root.type_id).then_some(entry)
    }

    fn root_for(handle: HostSlotRef, type_id: HostTypeId) -> HostRef {
        HostRef::new(
            type_id,
            HostObjectId::new(OWNED_HOST_OBJECT_ID_BASE + u64::from(handle.slot())),
            handle.generation(),
        )
    }
}

impl Default for RuntimeHostArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use vela_common::HostTypeId;
    use vela_host::error::HostResult;
    use vela_host::object::ScriptHostObject;
    use vela_host::path::HostRef;
    use vela_host::target::HostTargetInstance;
    use vela_host::value::HostValue;

    use super::RuntimeHostArena;
    use vela_host::lease::HostLeaseKind;

    struct ArenaObject;

    impl ScriptHostObject for ArenaObject {
        fn host_type_id(&self) -> HostTypeId {
            HostTypeId::new(91)
        }

        fn read_resolved_host(
            &self,
            _access: vela_host::resolved::ResolvedHostAccess,
            _target: HostTargetInstance<'_>,
        ) -> HostResult<HostValue> {
            Ok(HostValue::Unit)
        }
    }

    #[test]
    fn owned_host_slots_keep_identity_and_enforce_exact_leases() {
        let mut arena = RuntimeHostArena::new();
        let first = arena.retain(Box::new(ArenaObject));
        let second = arena.retain(Box::new(ArenaObject));
        assert_ne!(first, second);
        assert_eq!(first.object_id.get(), super::OWNED_HOST_OBJECT_ID_BASE);
        assert_eq!(second.object_id.get(), super::OWNED_HOST_OBJECT_ID_BASE + 1);
        assert_eq!(first.generation, 1);
        assert!(!arena.objects.spilled());
        assert!(arena.contains(first));
        assert!(!arena.contains(HostRef::new(
            HostTypeId::new(92),
            first.object_id,
            first.generation,
        )));
        assert!(!arena.contains(HostRef::new(
            first.type_id,
            first.object_id,
            first.generation + 1,
        )));

        let shared = arena
            .take_lease(first, HostLeaseKind::Shared)
            .expect("shared owned-host lease");
        assert!(!shared.is_exclusive());
        assert!(matches!(
            arena.take_lease(first, HostLeaseKind::Exclusive),
            Err(error)
                if matches!(
                    error.kind,
                    vela_host::error::HostErrorKind::HostObjectBusy { .. }
                )
        ));
        drop(shared);

        let exclusive = arena
            .take_lease(first, HostLeaseKind::Exclusive)
            .expect("exclusive owned-host lease after shared release");
        assert!(exclusive.is_exclusive());
    }
}
