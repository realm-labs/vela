use vela_common::HostMethodId;
use vela_def::StateId;

use crate::{
    call_value::HostCallValue,
    error::{HostError, HostErrorKind, HostRefLifetimeBoundary, HostResult},
    lease::{
        ErasedHostLease, HostLeaseKind, ScopedBorrowedHostCell, ScopedBorrowedHostGroupCell,
        host_lease_unsupported,
    },
    object::ScriptHostObject,
    path::{HostRef, HostSlotRef},
    protocol::{
        HostCollectionMutation, HostCollectionProjection, HostCollectionQuery,
        HostCollectionSnapshot,
    },
    resolved::{HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess},
    slot::HostRefSlots,
    target::HostTargetInstance,
    value::HostValue,
};

#[derive(Clone, Copy, Debug)]
pub struct ExternStateBinding<'a> {
    pub id: StateId,
    pub name: &'a str,
}

pub type HostLeaseInvoker<'callback> = dyn for<'lease> FnMut(
        &mut [ErasedHostLease<'lease>],
        &mut (dyn ScriptStateAdapter + Send),
    ) -> HostResult<()>
    + 'callback;

pub struct ScopedHostReturn<'lease> {
    pub object: ScopedBorrowedHostCell<'lease>,
    pub access: HostLeaseKind,
}

pub struct ScopedHostReturnGroup<'lease> {
    pub object: ScopedBorrowedHostGroupCell<'lease>,
    pub accesses: Vec<HostLeaseKind>,
}

pub enum ScopedHostReturns<'lease> {
    Single(ScopedHostReturn<'lease>),
    Group(ScopedHostReturnGroup<'lease>),
}

pub type ScopedHostReturnInvoker<'callback> = dyn for<'lease> FnMut(
        &mut [ErasedHostLease<'lease>],
    ) -> HostResult<Option<ScopedHostReturns<'lease>>>
    + 'callback;

pub trait ScriptStateAdapter {
    fn host_schema_epoch(&self) -> HostSchemaEpoch {
        HostSchemaEpoch::new(0)
    }

    /// Reports the strongest receiver access currently available for one root.
    ///
    /// Legacy adapters default to exclusive access; call-scoped adapters
    /// override this with the exact Rust `&T` or `&mut T` capability.
    fn host_receiver_access(&self, _root: HostRef) -> HostLeaseKind {
        HostLeaseKind::Exclusive
    }

    /// Interns one canonical host reference into this root execution's dense
    /// slot table.
    fn intern_host_ref(&mut self, _root: HostRef) -> HostResult<HostSlotRef> {
        Err(HostError::new(HostErrorKind::HostSlotStorageUnsupported))
    }

    /// Resolves a compact script handle back to canonical host metadata.
    fn resolve_host_ref(&self, handle: HostSlotRef) -> HostResult<HostRef> {
        Err(HostError::new(HostErrorKind::InvalidHostSlot { handle }))
    }

    /// Releases a root-scoped slot and invalidates every copied alias.
    fn release_host_ref(&mut self, handle: HostSlotRef) -> HostResult<HostRef> {
        Err(HostError::new(HostErrorKind::InvalidHostSlot { handle }))
    }

    fn extern_state_ref(&self, state: ExternStateBinding<'_>) -> HostResult<HostRef> {
        Err(HostError {
            kind: HostErrorKind::MissingExternState {
                name: state.name.to_owned(),
            },
            source_span: None,
        })
    }

    /// Transfers a newly constructed host object into the adapter's durable
    /// host-owned storage and returns its stable handle.
    ///
    /// Generic adapters fail closed. Runtime adapters that opt in own the Rust
    /// object independently of the script GC.
    fn retain_owned_host(
        &mut self,
        _object: Box<dyn ScriptHostObject + Send + Sync>,
    ) -> HostResult<HostRef> {
        Err(HostError::new(HostErrorKind::OwnedHostStorageUnsupported))
    }

    /// Transfers a newly constructed host object into the current root call.
    ///
    /// The adapter must reclaim the object deterministically when the root
    /// ends. Generic adapters fail closed.
    fn retain_call_scoped_host(
        &mut self,
        _object: Box<dyn ScriptHostObject + Send + Sync>,
    ) -> HostResult<HostRef> {
        Err(HostError::new(
            HostErrorKind::CallScopedHostStorageUnsupported,
        ))
    }

    fn resolve_host_access(&self, _spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        Ok(ResolvedHostAccess::generic_target(self.host_schema_epoch()))
    }

    /// Runs one invocation while holding an atomically acquired set of exact
    /// direct-host leases.
    ///
    /// Generic/opaque adapters deliberately fail closed. Runtime execution
    /// adapters override this only when their call arguments retain the
    /// concrete Rust objects and can prove canonical identity and lifetime.
    fn with_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        _invoke: &mut HostLeaseInvoker<'_>,
    ) -> HostResult<()> {
        match requests.first() {
            Some((root, _)) => Err(host_lease_unsupported(*root)),
            None => Err(HostError::new(HostErrorKind::InvalidArgument {
                expected: "at least one exact host lease request",
            })),
        }
    }

    /// Runs one synchronous invocation and retains an optional owner-frozen
    /// child in this adapter's root execution scope.
    fn with_scoped_host_return(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        _invoke: &mut ScopedHostReturnInvoker<'_>,
    ) -> HostResult<Option<Vec<HostRef>>> {
        match requests.first() {
            Some((root, _)) => Err(host_lease_unsupported(*root)),
            None => Err(HostError::new(HostErrorKind::InvalidArgument {
                expected: "at least one scoped host lease request",
            })),
        }
    }

    fn release_scoped_host(&mut self, root: HostRef) -> HostResult<()> {
        Err(host_lease_unsupported(root))
    }

    /// Releases a scoped host group and reports whether this call performed
    /// the release. Implementations may return `false` only for a group that
    /// is known to have already been released in the current root execution.
    fn try_release_scoped_host(&mut self, root: HostRef) -> HostResult<bool> {
        self.release_scoped_host(root)?;
        Ok(true)
    }

    /// Validates a host handle before it crosses a boundary that can outlive
    /// its synchronous root call tree.
    fn validate_host_ref_lifetime(
        &self,
        _root: HostRef,
        _boundary: HostRefLifetimeBoundary,
    ) -> HostResult<()> {
        Ok(())
    }

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue>;

    fn query_collection_host(
        &self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        Err(HostError::new(HostErrorKind::UnsupportedCollectionQuery {
            query,
        }))
    }

    fn snapshot_collection_host(
        &self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        Err(HostError::new(HostErrorKind::InvalidArgument {
            expected: projection.name(),
        }))
    }

    /// Retains one complex collection element as a call-scoped child HostRef.
    ///
    /// Generic adapters do not own the parent lease machinery and therefore
    /// decline this optional path.
    fn read_scoped_host(
        &mut self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<Option<HostRef>> {
        Ok(None)
    }

    /// Retains one homogeneous complex-element projection as child HostRefs.
    fn snapshot_scoped_collection_host(
        &mut self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        _projection: HostCollectionProjection,
    ) -> HostResult<Option<HostCollectionSnapshot>> {
        Ok(None)
    }

    fn mutate_collection_host(
        &mut self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        Err(HostError::new(
            HostErrorKind::UnsupportedCollectionMutation {
                mutation: mutation.kind(),
            },
        ))
    }

    fn write_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()>;

    fn mutate_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()>;

    fn remove_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()>;

    fn call_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostCallValue],
    ) -> HostResult<HostCallValue>;
}

impl ScriptStateAdapter for HostRefSlots {
    fn intern_host_ref(&mut self, root: HostRef) -> HostResult<HostSlotRef> {
        Ok(self.intern(root))
    }

    fn resolve_host_ref(&self, handle: HostSlotRef) -> HostResult<HostRef> {
        self.resolve(handle)
            .ok_or_else(|| HostError::new(HostErrorKind::InvalidHostSlot { handle }))
    }

    fn release_host_ref(&mut self, handle: HostSlotRef) -> HostResult<HostRef> {
        self.release(handle)
            .ok_or_else(|| HostError::new(HostErrorKind::InvalidHostSlot { handle }))
    }

    fn read_host(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        Err(slot_only_access_error(target, "read"))
    }

    fn write_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _value: HostValue,
    ) -> HostResult<()> {
        Err(slot_only_access_error(target, "write"))
    }

    fn mutate_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _op: HostMutationOp,
        _rhs: HostValue,
    ) -> HostResult<()> {
        Err(slot_only_access_error(target, "mutate"))
    }

    fn remove_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        Err(slot_only_access_error(target, "remove"))
    }

    fn call_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _method: HostMethodId,
        _args: &[HostCallValue],
    ) -> HostResult<HostCallValue> {
        Err(slot_only_access_error(target, "call"))
    }
}

fn slot_only_access_error(target: HostTargetInstance<'_>, action: &'static str) -> HostError {
    HostError::new(HostErrorKind::PermissionDenied {
        path: target.to_diagnostic_path().to_host_path(),
        action,
    })
}
