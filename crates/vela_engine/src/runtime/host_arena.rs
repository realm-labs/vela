use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::RwLock;
use vela_common::{HostMethodId, HostObjectId};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::lease::{ErasedHostLease, HostLeaseKind, OwnedHostLeaseSlot, host_object_busy};
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_host::resolved::{HostAccessSpec, HostMutationOp, ResolvedHostAccess};
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;

const OWNED_HOST_OBJECT_ID_BASE: u64 = 1 << 61;

/// Runtime-owned Rust objects created by registered host factories.
///
/// The script heap stores only compact `HostRef` handles. Objects live here
/// until their owning Runtime is dropped; the script GC neither owns nor
/// traces Rust state.
pub(super) struct RuntimeHostArena {
    objects: BTreeMap<HostRef, OwnedHostLeaseSlot>,
    next_object_id: u64,
}

impl RuntimeHostArena {
    pub(super) fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
            next_object_id: OWNED_HOST_OBJECT_ID_BASE,
        }
    }

    pub(super) fn retain(&mut self, object: Box<dyn ScriptHostObject + Send + Sync>) -> HostRef {
        let root = HostRef::new(
            object.host_type_id(),
            HostObjectId::new(self.next_object_id),
            1,
        );
        self.next_object_id = self.next_object_id.saturating_add(1);
        self.objects.insert(root, Arc::new(RwLock::new(object)));
        root
    }

    pub(super) fn contains(&self, root: HostRef) -> bool {
        self.objects.contains_key(&root)
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
            .find(|(root, _)| root.type_id == spec.plan.root_type)?;
        Some(
            object
                .try_read()
                .ok_or_else(|| host_object_busy(*root))
                .and_then(|object| object.resolve_host_target(spec)),
        )
    }

    pub(super) fn read(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> Option<HostResult<HostValue>> {
        let object = self.objects.get(&target.root)?;
        Some(
            object
                .try_read()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|object| object.read_resolved_host(access, target)),
        )
    }

    pub(super) fn write(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> Option<HostResult<()>> {
        let object = self.objects.get(&target.root)?;
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
        let object = self.objects.get(&target.root)?;
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
        let object = self.objects.get(&target.root)?;
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
        let object = self.objects.get(&target.root)?;
        Some(
            object
                .try_write()
                .ok_or_else(|| host_object_busy(target.root))
                .and_then(|mut object| object.call_resolved_host(access, target, method, args)),
        )
    }

    fn object(&self, root: HostRef) -> HostResult<&OwnedHostLeaseSlot> {
        self.objects.get(&root).ok_or_else(|| HostError {
            kind: HostErrorKind::MissingPath {
                path: vela_host::path::HostPath::new(root),
            },
            source_span: None,
        })
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
