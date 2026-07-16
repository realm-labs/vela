use vela_common::HostMethodId;
use vela_def::StateId;

use crate::{
    error::{HostError, HostErrorKind, HostResult},
    lease::{ErasedHostLease, HostLeaseKind, ScopedBorrowedHostCell, host_lease_unsupported},
    path::HostRef,
    resolved::{HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess},
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

pub type ScopedHostReturnInvoker<'callback> = dyn for<'lease> FnMut(
        &mut [ErasedHostLease<'lease>],
    ) -> HostResult<Option<ScopedHostReturn<'lease>>>
    + 'callback;

pub trait ScriptStateAdapter {
    fn host_schema_epoch(&self) -> HostSchemaEpoch {
        HostSchemaEpoch::new(0)
    }

    fn extern_state_ref(&self, state: ExternStateBinding<'_>) -> HostResult<HostRef> {
        Err(HostError {
            kind: HostErrorKind::MissingExternState {
                name: state.name.to_owned(),
            },
            source_span: None,
        })
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
    ) -> HostResult<Option<HostRef>> {
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

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue>;

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
        args: &[HostValue],
    ) -> HostResult<HostValue>;
}
