use vela_common::HostMethodId;
use vela_host::adapter::{GlobalBinding, ScriptStateAdapter};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::lease::{ErasedHostLease, HostLeaseKind, host_lease_unsupported, host_object_busy};
use vela_host::path::HostRef;
use vela_host::resolved::{HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess};
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap::ScriptHeap;
use vela_vm::value::Value;
use vela_vm::{NativeCallFuture, PreparedAsyncCall};

use super::call_args::HostArgBinding;
use super::{CallArgs, RuntimeGlobalStore};

const EXECUTION_HOST_OBJECT_ID_BASE: u64 = 1 << 63;

pub(super) trait DirectContextInvoker: Send {
    fn invoke<'invoke, 'lease>(
        self: Box<Self>,
        leases: &'invoke mut [ErasedHostLease<'lease>],
        host: &'invoke mut dyn ExecutionHostBoundary,
    ) -> NativeCallFuture<'invoke>
    where
        Self: 'invoke;
}

pub(super) struct ExecutionHost<'state, 'host> {
    args: CallArgs<'host>,
    globals: &'state mut RuntimeGlobalStore,
    fallback: FallbackAdapter<'host>,
    next_direct_object_id: u64,
}

pub(super) trait ExecutionHostBoundary: ScriptStateAdapter + Send {
    fn assign_direct_host_refs(&mut self, args: &mut CallArgs<'_>);

    fn invoke_prepared_with_lease<'call>(
        &'call mut self,
        prepared: &'call PreparedAsyncCall,
    ) -> NativeCallFuture<'call>;

    fn invoke_direct_context<'call>(
        &'call mut self,
        requests: Vec<(HostRef, HostLeaseKind)>,
        invoke: Box<dyn DirectContextInvoker + 'call>,
    ) -> NativeCallFuture<'call>;
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

impl ExecutionHostBoundary for ExecutionHost<'_, '_> {
    fn assign_direct_host_refs(&mut self, args: &mut CallArgs<'_>) {
        args.assign_direct_host_refs(&mut self.next_direct_object_id);
    }

    fn invoke_prepared_with_lease<'call>(
        &'call mut self,
        prepared: &'call PreparedAsyncCall,
    ) -> NativeCallFuture<'call> {
        let Some((root, kind)) = prepared.host_lease_request() else {
            return Box::pin(async {
                Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "nested typed host lease",
                }))
            });
        };
        match self.take_host_lease(root, kind) {
            Ok(lease) => prepared.invoke_with_host_lease(lease),
            Err(error) => Box::pin(async move { Err(error.into()) }),
        }
    }

    fn invoke_direct_context<'call>(
        &'call mut self,
        requests: Vec<(HostRef, HostLeaseKind)>,
        invoke: Box<dyn DirectContextInvoker + 'call>,
    ) -> NativeCallFuture<'call> {
        Box::pin(async move {
            for (root, _) in &requests {
                if self.globals.binding(*root).is_some() {
                    return Err(host_lease_unsupported(*root).into());
                }
            }
            let mut leases = self.args.take_host_leases(&requests)?;
            invoke.invoke(&mut leases, self).await
        })
    }
}

pub(super) struct ReentryExecutionHost<'scope> {
    args: CallArgs<'scope>,
    parent: &'scope mut dyn ExecutionHostBoundary,
}

impl<'scope> ReentryExecutionHost<'scope> {
    pub(super) fn new(
        mut args: CallArgs<'scope>,
        parent: &'scope mut dyn ExecutionHostBoundary,
    ) -> VmResult<Self> {
        if args.take_fallback().is_some() {
            return Err(vela_vm::error::VmError::new(
                vela_vm::error::VmErrorKind::TypeMismatch {
                    operation: "nested reentry fallback adapter",
                },
            ));
        }
        parent.assign_direct_host_refs(&mut args);
        Ok(Self { args, parent })
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
}

impl ExecutionHostBoundary for ReentryExecutionHost<'_> {
    fn assign_direct_host_refs(&mut self, args: &mut CallArgs<'_>) {
        self.parent.assign_direct_host_refs(args);
    }

    fn invoke_prepared_with_lease<'call>(
        &'call mut self,
        prepared: &'call PreparedAsyncCall,
    ) -> NativeCallFuture<'call> {
        let Some((root, kind)) = prepared.host_lease_request() else {
            return Box::pin(async {
                Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "nested typed host lease",
                }))
            });
        };
        if self.args.direct_binding(root).is_none() {
            return Box::pin(async move { Err(host_lease_unsupported(root).into()) });
        }
        match self.args.take_host_leases(&[(root, kind)]) {
            Ok(mut leases) => match leases.pop() {
                Some(lease) => prepared.invoke_with_host_lease(lease),
                None => Box::pin(async move { Err(host_lease_unsupported(root).into()) }),
            },
            Err(error) => Box::pin(async move { Err(error.into()) }),
        }
    }

    fn invoke_direct_context<'call>(
        &'call mut self,
        requests: Vec<(HostRef, HostLeaseKind)>,
        invoke: Box<dyn DirectContextInvoker + 'call>,
    ) -> NativeCallFuture<'call> {
        Box::pin(async move {
            for (root, _) in &requests {
                if self.args.direct_binding(*root).is_none() {
                    return Err(host_lease_unsupported(*root).into());
                }
            }
            let mut leases = self.args.take_host_leases(&requests)?;
            invoke.invoke(&mut leases, self).await
        })
    }
}

impl ScriptStateAdapter for ReentryExecutionHost<'_> {
    fn host_schema_epoch(&self) -> HostSchemaEpoch {
        self.parent.host_schema_epoch()
    }

    fn global_ref(&self, global: GlobalBinding<'_>) -> HostResult<HostRef> {
        self.parent.global_ref(global)
    }

    fn resolve_host_access(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        match self.args.direct_binding_by_type(spec.plan.root_type) {
            Some((_, HostArgBinding::Shared { object, .. })) => object.resolve_host_target(spec),
            Some((root, HostArgBinding::Mutable { object })) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref()
                .ok_or_else(|| host_object_busy(root))?
                .resolve_host_target(spec),
            None => self.parent.resolve_host_access(spec),
        }
    }

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
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
            None => self.parent.read_host(access, target),
        }
    }

    fn write_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        match self.args.direct_binding_mut(target.root) {
            Some(HostArgBinding::Shared { .. }) => {
                Err(ExecutionHost::direct_access_error(target, "write"))
            }
            Some(HostArgBinding::Mutable { object }) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref_mut()
                .ok_or_else(|| host_object_busy(target.root))?
                .write_resolved_host(access, target, value),
            None => self.parent.write_host(access, target, value),
        }
    }

    fn mutate_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        match self.args.direct_binding_mut(target.root) {
            Some(HostArgBinding::Shared { .. }) => {
                Err(ExecutionHost::direct_access_error(target, "write"))
            }
            Some(HostArgBinding::Mutable { object }) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref_mut()
                .ok_or_else(|| host_object_busy(target.root))?
                .mutate_resolved_host(access, target, op, rhs),
            None => self.parent.mutate_host(access, target, op, rhs),
        }
    }

    fn remove_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        match self.args.direct_binding_mut(target.root) {
            Some(HostArgBinding::Shared { .. }) => {
                Err(ExecutionHost::direct_access_error(target, "write"))
            }
            Some(HostArgBinding::Mutable { object }) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref_mut()
                .ok_or_else(|| host_object_busy(target.root))?
                .remove_resolved_host(access, target),
            None => self.parent.remove_host(access, target),
        }
    }

    fn call_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        match self.args.direct_binding_mut(target.root) {
            Some(HostArgBinding::Shared { .. }) => {
                Err(ExecutionHost::direct_access_error(target, "call"))
            }
            Some(HostArgBinding::Mutable { object }) => object
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_deref_mut()
                .ok_or_else(|| host_object_busy(target.root))?
                .call_resolved_host(access, target, method, args),
            None => self.parent.call_host(access, target, method, args),
        }
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
    use vela_vm::budget::ExecutionBudget;
    use vela_vm::heap::ScriptHeap;
    use vela_vm::value::Value;

    use super::{EXECUTION_HOST_OBJECT_ID_BASE, ExecutionHost, ReentryExecutionHost};
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

    #[test]
    fn nested_scope_uses_shared_allocator_and_invalidates_child_ref_on_drop() {
        let root_value = vec![1_i64];
        let child_value = vec![2_i64];
        let args = CallArgs::new().with_host_ref("root", &root_value);
        let mut globals = RuntimeGlobalStore::new();
        let mut host = ExecutionHost::new(args, &mut globals);
        let mut heap = ScriptHeap::default();
        let mut budget = ExecutionBudget::unbounded();

        let child_ref = {
            let child_args = CallArgs::new().with_host_ref("child", &child_value);
            let child = ReentryExecutionHost::new(child_args, &mut host)
                .expect("nested scope should allocate its direct binding");
            let values = child
                .resolve_values(
                    "child",
                    &["child".to_owned()],
                    &[false],
                    1,
                    &mut heap,
                    &mut budget,
                )
                .expect("child binding should resolve");
            let [Value::HostRef(child_ref)] = values.as_slice() else {
                panic!("child binding should resolve to HostRef");
            };
            *child_ref
        };

        assert_eq!(child_ref.object_id.get(), EXECUTION_HOST_OBJECT_ID_BASE + 1);
        assert_eq!(
            host.next_direct_object_id(),
            EXECUTION_HOST_OBJECT_ID_BASE + 2
        );
        assert!(host.args.direct_binding(child_ref).is_none());
    }
}
