use vela_common::HostMethodId;
use vela_host::adapter::{ExternStateBinding, ScriptStateAdapter};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::lease::HostLeaseKind;
use vela_host::path::HostRef;
use vela_host::protocol::{HostCollectionProjection, HostCollectionQuery, HostCollectionSnapshot};
use vela_host::resolved::{HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess};
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;

pub(super) enum FallbackAdapter<'call> {
    Borrowed(&'call mut (dyn ScriptStateAdapter + Send)),
    Empty,
}

impl FallbackAdapter<'_> {
    fn adapter(&self) -> Option<&(dyn ScriptStateAdapter + Send)> {
        match self {
            Self::Borrowed(adapter) => Some(&**adapter),
            Self::Empty => None,
        }
    }

    fn adapter_mut(&mut self) -> Option<&mut (dyn ScriptStateAdapter + Send)> {
        match self {
            Self::Borrowed(adapter) => Some(&mut **adapter),
            Self::Empty => None,
        }
    }

    fn missing_path(target: HostTargetInstance<'_>) -> HostError {
        HostError {
            kind: HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        }
    }
}

impl ScriptStateAdapter for FallbackAdapter<'_> {
    fn host_schema_epoch(&self) -> HostSchemaEpoch {
        self.adapter().map_or(
            HostSchemaEpoch::new(0),
            ScriptStateAdapter::host_schema_epoch,
        )
    }

    fn host_receiver_access(&self, root: HostRef) -> HostLeaseKind {
        self.adapter().map_or(HostLeaseKind::Exclusive, |adapter| {
            adapter.host_receiver_access(root)
        })
    }

    fn extern_state_ref(&self, state: ExternStateBinding<'_>) -> HostResult<HostRef> {
        self.adapter().map_or_else(
            || Err(missing_extern_state(state.name)),
            |adapter| adapter.extern_state_ref(state),
        )
    }

    fn resolve_host_access(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        self.adapter().map_or_else(
            || Ok(ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0))),
            |adapter| adapter.resolve_host_access(spec),
        )
    }

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        self.adapter().map_or_else(
            || Err(Self::missing_path(target)),
            |adapter| adapter.read_host(access, target),
        )
    }

    fn query_collection_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        self.adapter().map_or_else(
            || {
                Err(HostError {
                    kind: HostErrorKind::UnsupportedCollectionQuery { query },
                    source_span: None,
                })
            },
            |adapter| adapter.query_collection_host(access, target, query),
        )
    }

    fn snapshot_collection_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        self.adapter().map_or_else(
            || {
                Err(HostError {
                    kind: HostErrorKind::InvalidArgument {
                        expected: projection.name(),
                    },
                    source_span: None,
                })
            },
            |adapter| adapter.snapshot_collection_host(access, target, projection),
        )
    }

    fn write_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        self.adapter_mut().map_or_else(
            || Err(Self::missing_path(target)),
            |adapter| adapter.write_host(access, target, value),
        )
    }

    fn mutate_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        self.adapter_mut().map_or_else(
            || Err(Self::missing_path(target)),
            |adapter| adapter.mutate_host(access, target, op, rhs),
        )
    }

    fn remove_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        self.adapter_mut().map_or_else(
            || Err(Self::missing_path(target)),
            |adapter| adapter.remove_host(access, target),
        )
    }

    fn call_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        self.adapter_mut().map_or_else(
            || {
                Err(HostError {
                    kind: HostErrorKind::UnsupportedMethod { method },
                    source_span: None,
                })
            },
            |adapter| adapter.call_host(access, target, method, args),
        )
    }
}

fn missing_extern_state(name: &str) -> HostError {
    HostError {
        kind: HostErrorKind::MissingExternState {
            name: name.to_owned(),
        },
        source_span: None,
    }
}
