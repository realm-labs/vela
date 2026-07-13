use vela_common::HostMethodId;
use vela_host::adapter::{GlobalBinding, ScriptStateAdapter};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::lease::{ErasedHostLease, HostLeaseKind, host_lease_unsupported, host_object_busy};
use vela_host::path::HostRef;
use vela_host::resolved::{HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess};
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;
use vela_vm::heap::ScriptHeap;
use vela_vm::value::Value;

use super::call_args::HostArgBinding;
use super::{CallArgs, RuntimeGlobalStore};

const EXECUTION_HOST_OBJECT_ID_BASE: u64 = 1 << 63;

pub(super) struct ExecutionHost<'state, 'host> {
    args: CallArgs<'host>,
    globals: &'state mut RuntimeGlobalStore,
    fallback: FallbackAdapter<'host>,
    next_direct_object_id: u64,
}

impl<'state, 'host> ExecutionHost<'state, 'host> {
    pub(super) fn new(mut args: CallArgs<'host>, globals: &'state mut RuntimeGlobalStore) -> Self {
        let fallback = args
            .take_fallback()
            .map_or(FallbackAdapter::Empty, FallbackAdapter::Borrowed);
        let mut execution_host = Self {
            args,
            globals,
            fallback,
            next_direct_object_id: EXECUTION_HOST_OBJECT_ID_BASE,
        };
        execution_host
            .args
            .assign_direct_host_refs(&mut execution_host.next_direct_object_id);
        execution_host
    }

    pub(super) fn resolve_values(
        &self,
        entry: &str,
        params: &[String],
        param_defaults: &[bool],
        runtime_id: u64,
        heap: &mut ScriptHeap,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Vec<Value>> {
        self.args
            .resolve_values(entry, params, param_defaults, runtime_id, heap, budget)
    }

    fn direct_access_error(target: HostTargetInstance<'_>, action: &'static str) -> HostError {
        HostError {
            kind: HostErrorKind::PermissionDenied {
                path: target.to_diagnostic_path().to_host_path(),
                action,
            },
            source_span: None,
        }
    }

    #[cfg(test)]
    pub(super) const fn next_direct_object_id(&self) -> u64 {
        self.next_direct_object_id
    }

    pub(super) fn take_host_lease(
        &mut self,
        root: HostRef,
        kind: HostLeaseKind,
    ) -> HostResult<ErasedHostLease<'host>> {
        if self.globals.binding(root).is_some() {
            return Err(host_lease_unsupported(root));
        }
        self.args
            .take_host_leases(&[(root, kind)])?
            .pop()
            .ok_or_else(|| host_lease_unsupported(root))
    }
}

impl ScriptStateAdapter for ExecutionHost<'_, '_> {
    fn host_schema_epoch(&self) -> HostSchemaEpoch {
        self.fallback.host_schema_epoch()
    }

    fn global_ref(&self, global: GlobalBinding<'_>) -> HostResult<HostRef> {
        self.globals
            .host_ref_for_binding(global)
            .or_else(|_| self.fallback.global_ref(global))
    }

    fn resolve_host_access(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        if let Some(global) = self.globals.binding_by_type(spec.plan.root_type) {
            return global.object.resolve_host_target(spec);
        }
        match self.args.direct_binding_by_type(spec.plan.root_type) {
            Some((_, HostArgBinding::Shared { object, .. })) => object.resolve_host_target(spec),
            Some((root, HostArgBinding::Mutable { object })) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref()
                .ok_or_else(|| host_object_busy(root))?
                .resolve_host_target(spec),
            None => self.fallback.resolve_host_access(spec),
        }
    }

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        if let Some(global) = self.globals.binding(target.root) {
            return global.object.read_resolved_host(access, target);
        }
        match self.args.direct_binding(target.root) {
            Some(HostArgBinding::Shared { object, .. }) => {
                object.read_resolved_host(access, target)
            }
            Some(HostArgBinding::Mutable { object }) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref()
                .ok_or_else(|| host_object_busy(target.root))?
                .read_resolved_host(access, target),
            None => self.fallback.read_host(access, target),
        }
    }

    fn write_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        if let Some(global) = self.globals.binding_mut(target.root) {
            return global.object.write_resolved_host(access, target, value);
        }
        match self.args.direct_binding_mut(target.root) {
            Some(HostArgBinding::Shared { .. }) => Err(Self::direct_access_error(target, "write")),
            Some(HostArgBinding::Mutable { object }) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref_mut()
                .ok_or_else(|| host_object_busy(target.root))?
                .write_resolved_host(access, target, value),
            None => self.fallback.write_host(access, target, value),
        }
    }

    fn mutate_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        if let Some(global) = self.globals.binding_mut(target.root) {
            return global.object.mutate_resolved_host(access, target, op, rhs);
        }
        match self.args.direct_binding_mut(target.root) {
            Some(HostArgBinding::Shared { .. }) => Err(Self::direct_access_error(target, "write")),
            Some(HostArgBinding::Mutable { object }) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref_mut()
                .ok_or_else(|| host_object_busy(target.root))?
                .mutate_resolved_host(access, target, op, rhs),
            None => self.fallback.mutate_host(access, target, op, rhs),
        }
    }

    fn remove_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        if let Some(global) = self.globals.binding_mut(target.root) {
            return global.object.remove_resolved_host(access, target);
        }
        match self.args.direct_binding_mut(target.root) {
            Some(HostArgBinding::Shared { .. }) => Err(Self::direct_access_error(target, "write")),
            Some(HostArgBinding::Mutable { object }) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref_mut()
                .ok_or_else(|| host_object_busy(target.root))?
                .remove_resolved_host(access, target),
            None => self.fallback.remove_host(access, target),
        }
    }

    fn call_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        if let Some(global) = self.globals.binding_mut(target.root) {
            return global
                .object
                .call_resolved_host(access, target, method, args);
        }
        match self.args.direct_binding_mut(target.root) {
            Some(HostArgBinding::Shared { .. }) => Err(Self::direct_access_error(target, "call")),
            Some(HostArgBinding::Mutable { object }) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref_mut()
                .ok_or_else(|| host_object_busy(target.root))?
                .call_resolved_host(access, target, method, args),
            None => self.fallback.call_host(access, target, method, args),
        }
    }
}

enum FallbackAdapter<'call> {
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

    fn global_ref(&self, global: GlobalBinding<'_>) -> HostResult<HostRef> {
        self.adapter().map_or_else(
            || Err(missing_global(global.name)),
            |adapter| adapter.global_ref(global),
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

fn missing_global(name: &str) -> HostError {
    HostError {
        kind: HostErrorKind::MissingGlobal {
            name: name.to_owned(),
        },
        source_span: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{EXECUTION_HOST_OBJECT_ID_BASE, ExecutionHost};
    use crate::runtime::{CallArgs, RuntimeGlobalStore};

    #[test]
    fn direct_host_ids_are_allocated_by_the_execution_owner() {
        let shared = vec![1_i64];
        let mut mutable = vec![2_i64];
        let args = CallArgs::new()
            .with_host_ref("shared", &shared)
            .with_host_mut("mutable", &mut mutable);
        let mut globals = RuntimeGlobalStore::new();

        let host = ExecutionHost::new(args, &mut globals);

        assert_eq!(
            host.next_direct_object_id(),
            EXECUTION_HOST_OBJECT_ID_BASE + 2
        );
    }
}
