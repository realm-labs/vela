use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use vela_common::HostMethodId;
use vela_host::adapter::{
    ExternStateBinding, HostLeaseInvoker, ScopedHostReturn, ScopedHostReturnGroup,
    ScopedHostReturnInvoker, ScopedHostReturns, ScriptStateAdapter,
};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::lease::{
    BorrowLeaseId, ErasedHostLease, HostLeaseKind, ScopedBorrowedHostGroupCell,
    ScopedHostLeaseSlot, host_lease_unsupported, host_object_busy,
};
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_host::protocol::{
    HostCollectionMutation, HostCollectionProjection, HostCollectionQuery, HostCollectionSnapshot,
};
use vela_host::resolved::{HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess};
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::value::Value;
use vela_vm::{NativeCallFuture, PreparedAsyncCall};

use super::call_args::CallArgRuntime;
use super::{CallArgs, RuntimeExternStateBindings, RuntimeHostArena};

mod fallback;
mod scoped_access;

use fallback::FallbackAdapter;

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
    extern_states: &'state mut RuntimeExternStateBindings,
    host_arena: &'state mut RuntimeHostArena,
    fallback: FallbackAdapter<'host>,
    next_direct_object_id: u64,
    scoped_hosts: BTreeMap<HostRef, ScopedHostBinding<'host>>,
    expired_scoped_hosts: BTreeSet<HostRef>,
}

struct ScopedHostBinding<'host> {
    _borrow_lease_id: BorrowLeaseId,
    access: HostLeaseKind,
    object: ScopedHostObjectBinding<'host>,
    activity: Arc<()>,
}

enum ScopedHostObjectBinding<'host> {
    Single(ScopedHostLeaseSlot<'host>),
    Group {
        object: Arc<ScopedBorrowedHostGroupCell<'host>>,
        index: usize,
    },
}

pub(super) trait ExecutionHostBoundary: ScriptStateAdapter + Send {
    fn assign_direct_host_refs(&mut self, args: &mut CallArgs<'_>);

    fn with_execution_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        invoke: &mut ExecutionHostLeaseInvoker<'_>,
    ) -> VmResult<()>;

    fn invoke_prepared_with_lease<'call>(
        &'call mut self,
        prepared: &'call PreparedAsyncCall,
    ) -> NativeCallFuture<'call>;

    fn invoke_prepared_with_leases<'call>(
        &'call mut self,
        prepared: &'call PreparedAsyncCall,
    ) -> NativeCallFuture<'call>;

    fn invoke_direct_context<'call>(
        &'call mut self,
        requests: vela_host::lease::HostLeaseRequestSet,
        invoke: Box<dyn DirectContextInvoker + 'call>,
    ) -> NativeCallFuture<'call>;
}

pub(super) type ExecutionHostLeaseInvoker<'invoke> = dyn for<'lease, 'host> FnMut(
        &mut [ErasedHostLease<'lease>],
        &'host mut dyn ExecutionHostBoundary,
    ) -> VmResult<()>
    + 'invoke;

impl<'state, 'host> ExecutionHost<'state, 'host> {
    pub(super) fn new(
        mut args: CallArgs<'host>,
        extern_states: &'state mut RuntimeExternStateBindings,
        host_arena: &'state mut RuntimeHostArena,
    ) -> Self {
        let fallback = args
            .take_fallback()
            .map_or(FallbackAdapter::Empty, FallbackAdapter::Borrowed);
        let mut execution_host = Self {
            args,
            extern_states,
            host_arena,
            fallback,
            next_direct_object_id: EXECUTION_HOST_OBJECT_ID_BASE,
            scoped_hosts: BTreeMap::new(),
            expired_scoped_hosts: BTreeSet::new(),
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
        mut runtime: CallArgRuntime<'_, '_, '_>,
    ) -> VmResult<Vec<Value>> {
        self.args
            .resolve_values(entry, params, param_defaults, &mut runtime)
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
        if self.host_arena.contains(root) {
            return self.host_arena.take_lease(root, kind);
        }
        if self.extern_states.binding(root).is_some() {
            return Err(host_lease_unsupported(root));
        }
        self.args
            .take_host_leases(&[(root, kind)])?
            .pop()
            .ok_or_else(|| host_lease_unsupported(root))
    }

    fn take_execution_host_lease(
        &mut self,
        root: HostRef,
        kind: HostLeaseKind,
    ) -> HostResult<ErasedHostLease<'host>> {
        if self.expired_scoped_hosts.contains(&root) {
            return Err(HostError {
                kind: HostErrorKind::ExpiredBorrowedHostRef {
                    path: vela_host::path::HostPath::new(root),
                },
                source_span: None,
            });
        }
        if let Some(binding) = self.scoped_hosts.get(&root) {
            let ScopedHostObjectBinding::Single(object) = &binding.object else {
                return Err(host_lease_unsupported(root));
            };
            return match (binding.access, kind) {
                (HostLeaseKind::Shared, HostLeaseKind::Exclusive) => Err(host_object_busy(root)),
                (_, HostLeaseKind::Shared) => object
                    .try_read_arc()
                    .map(|object| ErasedHostLease::ScopedShared { object })
                    .ok_or_else(|| host_object_busy(root)),
                (HostLeaseKind::Exclusive, HostLeaseKind::Exclusive) => object
                    .try_write_arc()
                    .map(|object| ErasedHostLease::ScopedExclusive { object })
                    .ok_or_else(|| host_object_busy(root)),
            };
        }
        self.take_host_lease(root, kind)
    }

    fn take_execution_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
    ) -> HostResult<Vec<ErasedHostLease<'host>>> {
        let mut leases = Vec::with_capacity(requests.len());
        for (root, kind) in requests {
            match self.take_execution_host_lease(*root, *kind) {
                Ok(lease) => leases.push(lease),
                Err(error) => {
                    drop(leases);
                    return Err(error);
                }
            }
        }
        Ok(leases)
    }

    fn retain_scoped_host(&mut self, returned: ScopedHostReturn<'host>) -> HostRef {
        let type_id = returned.object.host_type_id();
        let root = HostRef::new(
            type_id,
            vela_common::HostObjectId::new(self.next_direct_object_id),
            1,
        );
        self.next_direct_object_id = self.next_direct_object_id.saturating_add(1);
        self.scoped_hosts.insert(
            root,
            ScopedHostBinding {
                _borrow_lease_id: BorrowLeaseId::new(root.object_id.get()),
                access: returned.access,
                object: ScopedHostObjectBinding::Single(Arc::new(parking_lot::RwLock::new(
                    Box::new(returned.object),
                ))),
                activity: Arc::new(()),
            },
        );
        root
    }

    fn retain_scoped_host_group(
        &mut self,
        returned: ScopedHostReturnGroup<'host>,
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
            let root = HostRef::new(
                type_id,
                vela_common::HostObjectId::new(self.next_direct_object_id),
                1,
            );
            self.next_direct_object_id = self.next_direct_object_id.saturating_add(1);
            self.scoped_hosts.insert(
                root,
                ScopedHostBinding {
                    _borrow_lease_id: BorrowLeaseId::new(root.object_id.get()),
                    access,
                    object: ScopedHostObjectBinding::Group {
                        object: Arc::clone(&object),
                        index,
                    },
                    activity: Arc::new(()),
                },
            );
            roots.push(root);
        }
        Ok(roots)
    }

    fn with_group_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        invoke: &mut HostLeaseInvoker<'_>,
    ) -> Option<HostResult<()>> {
        let mut group = None;
        let mut children = Vec::with_capacity(requests.len());
        for (root, kind) in requests {
            let binding = self.scoped_hosts.get(root)?;
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
            let mut leases = Vec::with_capacity(children.len());
            let mut activities = Vec::with_capacity(children.len());
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
}

impl ExecutionHostBoundary for ExecutionHost<'_, '_> {
    fn assign_direct_host_refs(&mut self, args: &mut CallArgs<'_>) {
        args.assign_direct_host_refs(&mut self.next_direct_object_id);
    }

    fn with_execution_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        invoke: &mut ExecutionHostLeaseInvoker<'_>,
    ) -> VmResult<()> {
        for (root, _) in requests {
            if self.extern_states.binding(*root).is_some() {
                return Err(host_lease_unsupported(*root).into());
            }
        }
        let mut leases = self.take_execution_host_leases(requests)?;
        invoke(&mut leases, self)
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
        match self.take_execution_host_lease(root, kind) {
            Ok(lease) => prepared.invoke_with_host_lease(lease),
            Err(error) => Box::pin(async move { Err(error.into()) }),
        }
    }

    fn invoke_prepared_with_leases<'call>(
        &'call mut self,
        prepared: &'call PreparedAsyncCall,
    ) -> NativeCallFuture<'call> {
        Box::pin(async move {
            let requests = prepared.host_lease_requests().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "async direct host function leases",
                })
            })?;
            for (root, _) in &requests {
                if self.extern_states.binding(*root).is_some() {
                    return Err(host_lease_unsupported(*root).into());
                }
            }
            let mut leases = self.take_execution_host_leases(&requests)?;
            prepared.invoke_with_host_leases(&mut leases).await
        })
    }

    fn invoke_direct_context<'call>(
        &'call mut self,
        requests: vela_host::lease::HostLeaseRequestSet,
        invoke: Box<dyn DirectContextInvoker + 'call>,
    ) -> NativeCallFuture<'call> {
        Box::pin(async move {
            for (root, _) in &requests {
                if self.extern_states.binding(*root).is_some() {
                    return Err(host_lease_unsupported(*root).into());
                }
            }
            let mut leases = self.take_execution_host_leases(&requests)?;
            invoke.invoke(&mut leases, self).await
        })
    }
}

pub(super) struct ReentryExecutionHost<'args, 'parent> {
    args: CallArgs<'args>,
    parent: &'parent mut dyn ExecutionHostBoundary,
}

impl<'args, 'parent> ReentryExecutionHost<'args, 'parent> {
    pub(super) fn new(
        mut args: CallArgs<'args>,
        parent: &'parent mut dyn ExecutionHostBoundary,
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
        mut runtime: CallArgRuntime<'_, '_, '_>,
    ) -> VmResult<Vec<Value>> {
        self.args
            .resolve_values(entry, params, param_defaults, &mut runtime)
    }
}

impl ExecutionHostBoundary for ReentryExecutionHost<'_, '_> {
    fn assign_direct_host_refs(&mut self, args: &mut CallArgs<'_>) {
        self.parent.assign_direct_host_refs(args);
    }

    fn with_execution_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        invoke: &mut ExecutionHostLeaseInvoker<'_>,
    ) -> VmResult<()> {
        if requests
            .iter()
            .all(|(root, _)| self.args.direct_binding(*root).is_some())
        {
            let mut leases = self.args.take_host_leases(requests)?;
            return invoke(&mut leases, self);
        }
        self.parent.with_execution_host_leases(requests, invoke)
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
            return self.parent.invoke_prepared_with_lease(prepared);
        }
        match self.args.take_host_leases(&[(root, kind)]) {
            Ok(mut leases) => match leases.pop() {
                Some(lease) => prepared.invoke_with_host_lease(lease),
                None => Box::pin(async move { Err(host_lease_unsupported(root).into()) }),
            },
            Err(error) => Box::pin(async move { Err(error.into()) }),
        }
    }

    fn invoke_prepared_with_leases<'call>(
        &'call mut self,
        prepared: &'call PreparedAsyncCall,
    ) -> NativeCallFuture<'call> {
        Box::pin(async move {
            let requests = prepared.host_lease_requests().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "nested async direct host function leases",
                })
            })?;
            if requests
                .iter()
                .any(|(root, _)| self.args.direct_binding(*root).is_none())
            {
                return self.parent.invoke_prepared_with_leases(prepared).await;
            }
            let mut leases = self.args.take_host_leases(&requests)?;
            prepared.invoke_with_host_leases(&mut leases).await
        })
    }

    fn invoke_direct_context<'call>(
        &'call mut self,
        requests: vela_host::lease::HostLeaseRequestSet,
        invoke: Box<dyn DirectContextInvoker + 'call>,
    ) -> NativeCallFuture<'call> {
        Box::pin(async move {
            if requests
                .iter()
                .any(|(root, _)| self.args.direct_binding(*root).is_none())
            {
                return self.parent.invoke_direct_context(requests, invoke).await;
            }
            let mut leases = self.args.take_host_leases(&requests)?;
            invoke.invoke(&mut leases, self).await
        })
    }
}

impl ScriptStateAdapter for ReentryExecutionHost<'_, '_> {
    fn host_schema_epoch(&self) -> HostSchemaEpoch {
        self.parent.host_schema_epoch()
    }

    fn host_receiver_access(&self, root: HostRef) -> HostLeaseKind {
        match self.args.direct_binding(root) {
            Some(binding) => binding.receiver_access(),
            None => self.parent.host_receiver_access(root),
        }
    }

    fn extern_state_ref(&self, state: ExternStateBinding<'_>) -> HostResult<HostRef> {
        self.parent.extern_state_ref(state)
    }

    fn retain_owned_host(
        &mut self,
        object: Box<dyn ScriptHostObject + Send + Sync>,
    ) -> HostResult<HostRef> {
        self.parent.retain_owned_host(object)
    }

    fn with_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        invoke: &mut HostLeaseInvoker<'_>,
    ) -> HostResult<()> {
        for (root, _) in requests {
            if self.args.direct_binding(*root).is_none() {
                return Err(host_lease_unsupported(*root));
            }
        }
        let mut leases = self.args.take_host_leases(requests)?;
        invoke(&mut leases, self)
    }

    fn with_scoped_host_return(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        invoke: &mut ScopedHostReturnInvoker<'_>,
    ) -> HostResult<Option<Vec<HostRef>>> {
        if requests
            .iter()
            .all(|(root, _)| self.args.direct_binding(*root).is_some())
        {
            let mut leases = self.args.take_host_leases(requests)?;
            let returned = invoke(&mut leases)?;
            return match returned {
                Some(_) => Err(host_lease_unsupported(requests[0].0)),
                None => Ok(None),
            };
        }
        self.parent.with_scoped_host_return(requests, invoke)
    }

    fn release_scoped_host(&mut self, root: HostRef) -> HostResult<()> {
        self.parent.release_scoped_host(root)
    }

    fn resolve_host_access(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        match self.args.direct_binding_by_type(spec.plan.root_type) {
            Some((root, binding)) => {
                binding.inspect(root, |object| object.resolve_host_target(spec))
            }
            None => self.parent.resolve_host_access(spec),
        }
    }

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        match self.args.direct_binding(target.root) {
            Some(binding) => binding.inspect(target.root, |object| {
                object.read_resolved_host(access, target)
            }),
            None => self.parent.read_host(access, target),
        }
    }

    fn query_collection_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        match self.args.direct_binding(target.root) {
            Some(binding) => binding.inspect(target.root, |object| {
                object.query_collection_resolved_host(access, target, query)
            }),
            None => self.parent.query_collection_host(access, target, query),
        }
    }

    fn snapshot_collection_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        match self.args.direct_binding(target.root) {
            Some(binding) => binding.inspect(target.root, |object| {
                object.snapshot_collection_resolved_host(access, target, projection)
            }),
            None => self
                .parent
                .snapshot_collection_host(access, target, projection),
        }
    }

    fn mutate_collection_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        match self.args.direct_binding_mut(target.root) {
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(ExecutionHost::direct_access_error(target, "write"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.mutate_collection_resolved_host(access, target, mutation)
            }),
            None => self.parent.mutate_collection_host(access, target, mutation),
        }
    }

    fn write_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        match self.args.direct_binding_mut(target.root) {
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(ExecutionHost::direct_access_error(target, "write"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.write_resolved_host(access, target, value)
            }),
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
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(ExecutionHost::direct_access_error(target, "write"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.mutate_resolved_host(access, target, op, rhs)
            }),
            None => self.parent.mutate_host(access, target, op, rhs),
        }
    }

    fn remove_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        match self.args.direct_binding_mut(target.root) {
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(ExecutionHost::direct_access_error(target, "write"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.remove_resolved_host(access, target)
            }),
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
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(ExecutionHost::direct_access_error(target, "call"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.call_resolved_host(access, target, method, args)
            }),
            None => self.parent.call_host(access, target, method, args),
        }
    }
}

impl ScriptStateAdapter for ExecutionHost<'_, '_> {
    fn host_schema_epoch(&self) -> HostSchemaEpoch {
        self.fallback.host_schema_epoch()
    }

    fn host_receiver_access(&self, root: HostRef) -> HostLeaseKind {
        if self.host_arena.contains(root) {
            return HostLeaseKind::Exclusive;
        }
        if self.extern_states.binding(root).is_some() {
            return HostLeaseKind::Exclusive;
        }
        if let Some(binding) = self.scoped_hosts.get(&root) {
            return binding.access;
        }
        match self.args.direct_binding(root) {
            Some(binding) => binding.receiver_access(),
            None => self.fallback.host_receiver_access(root),
        }
    }

    fn extern_state_ref(&self, state: ExternStateBinding<'_>) -> HostResult<HostRef> {
        self.extern_states
            .host_ref_for_binding(state)
            .or_else(|_| self.fallback.extern_state_ref(state))
    }

    fn retain_owned_host(
        &mut self,
        object: Box<dyn ScriptHostObject + Send + Sync>,
    ) -> HostResult<HostRef> {
        Ok(self.host_arena.retain(object))
    }

    fn with_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        invoke: &mut HostLeaseInvoker<'_>,
    ) -> HostResult<()> {
        for (root, _) in requests {
            if self.extern_states.binding(*root).is_some() {
                return Err(host_lease_unsupported(*root));
            }
        }
        if let Some(result) = self.with_group_host_leases(requests, invoke) {
            return result;
        }
        let mut leases = self.take_execution_host_leases(requests)?;
        invoke(&mut leases, self)
    }

    fn with_scoped_host_return(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
        invoke: &mut ScopedHostReturnInvoker<'_>,
    ) -> HostResult<Option<Vec<HostRef>>> {
        for (root, _) in requests {
            if self.extern_states.binding(*root).is_some() {
                return Err(host_lease_unsupported(*root));
            }
        }
        let mut leases = self.take_execution_host_leases(requests)?;
        invoke(&mut leases)?
            .map(|returned| match returned {
                ScopedHostReturns::Single(returned) => Ok(vec![self.retain_scoped_host(returned)]),
                ScopedHostReturns::Group(returned) => self.retain_scoped_host_group(returned),
            })
            .transpose()
    }

    fn release_scoped_host(&mut self, root: HostRef) -> HostResult<()> {
        let Some(binding) = self.scoped_hosts.get(&root) else {
            let kind = if self.expired_scoped_hosts.contains(&root) {
                HostErrorKind::ExpiredBorrowedHostRef {
                    path: vela_host::path::HostPath::new(root),
                }
            } else {
                HostErrorKind::NotScopedBorrow {
                    path: vela_host::path::HostPath::new(root),
                }
            };
            return Err(HostError {
                kind,
                source_span: None,
            });
        };
        let in_use = match &binding.object {
            ScopedHostObjectBinding::Single(object) => Arc::strong_count(object) != 1,
            ScopedHostObjectBinding::Group { .. } => Arc::strong_count(&binding.activity) != 1,
        };
        if in_use {
            return Err(HostError {
                kind: HostErrorKind::BorrowStillInUse {
                    path: vela_host::path::HostPath::new(root),
                },
                source_span: None,
            });
        }
        self.scoped_hosts.remove(&root);
        self.expired_scoped_hosts.insert(root);
        Ok(())
    }

    fn resolve_host_access(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        if let Some(binding) = self.extern_states.binding_by_type(spec.plan.root_type) {
            return binding.object.resolve_host_target(spec);
        }
        if let Some(result) = self.host_arena.resolve(spec) {
            return result;
        }
        if let Some((root, _)) = self
            .scoped_hosts
            .iter()
            .find(|(root, _)| root.type_id == spec.plan.root_type)
            && let Some(result) =
                self.inspect_scoped_host(*root, |object| object.resolve_host_target(spec))
        {
            return result;
        }
        match self.args.direct_binding_by_type(spec.plan.root_type) {
            Some((root, binding)) => {
                binding.inspect(root, |object| object.resolve_host_target(spec))
            }
            None => self.fallback.resolve_host_access(spec),
        }
    }

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        if let Some(binding) = self.extern_states.binding(target.root) {
            return binding.object.read_resolved_host(access, target);
        }
        if let Some(result) = self.host_arena.read(access, target) {
            return result;
        }
        if let Some(result) = self.inspect_scoped_host(target.root, |object| {
            object.read_resolved_host(access, target)
        }) {
            return result;
        }
        match self.args.direct_binding(target.root) {
            Some(binding) => binding.inspect(target.root, |object| {
                object.read_resolved_host(access, target)
            }),
            None => self.fallback.read_host(access, target),
        }
    }

    fn query_collection_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue> {
        if let Some(binding) = self.extern_states.binding(target.root) {
            return binding
                .object
                .query_collection_resolved_host(access, target, query);
        }
        if let Some(result) = self.host_arena.query_collection(access, target, query) {
            return result;
        }
        if let Some(result) = self.inspect_scoped_host(target.root, |object| {
            object.query_collection_resolved_host(access, target, query)
        }) {
            return result;
        }
        match self.args.direct_binding(target.root) {
            Some(binding) => binding.inspect(target.root, |object| {
                object.query_collection_resolved_host(access, target, query)
            }),
            None => self.fallback.query_collection_host(access, target, query),
        }
    }

    fn snapshot_collection_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot> {
        if let Some(binding) = self.extern_states.binding(target.root) {
            return binding
                .object
                .snapshot_collection_resolved_host(access, target, projection);
        }
        if let Some(result) = self
            .host_arena
            .snapshot_collection(access, target, projection)
        {
            return result;
        }
        if let Some(result) = self.inspect_scoped_host(target.root, |object| {
            object.snapshot_collection_resolved_host(access, target, projection)
        }) {
            return result;
        }
        match self.args.direct_binding(target.root) {
            Some(binding) => binding.inspect(target.root, |object| {
                object.snapshot_collection_resolved_host(access, target, projection)
            }),
            None => self
                .fallback
                .snapshot_collection_host(access, target, projection),
        }
    }

    fn mutate_collection_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()> {
        if let Some(binding) = self.extern_states.binding_mut(target.root) {
            return binding
                .object
                .mutate_collection_resolved_host(access, target, mutation);
        }
        if self.host_arena.contains(target.root) {
            return self
                .host_arena
                .mutate_collection(access, target, mutation)
                .expect("owned host root remains present");
        }
        if self.scoped_hosts.contains_key(&target.root) {
            return self
                .mutate_scoped_host(target.root, |object| {
                    object.mutate_collection_resolved_host(access, target, mutation)
                })
                .expect("scoped host root remains present");
        }
        match self.args.direct_binding_mut(target.root) {
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(Self::direct_access_error(target, "write"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.mutate_collection_resolved_host(access, target, mutation)
            }),
            None => self
                .fallback
                .mutate_collection_host(access, target, mutation),
        }
    }

    fn write_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        if let Some(binding) = self.extern_states.binding_mut(target.root) {
            return binding.object.write_resolved_host(access, target, value);
        }
        if self.host_arena.contains(target.root) {
            return self
                .host_arena
                .write(access, target, value)
                .expect("owned host root remains present");
        }
        if self.scoped_hosts.contains_key(&target.root) {
            return self
                .mutate_scoped_host(target.root, |object| {
                    object.write_resolved_host(access, target, value)
                })
                .expect("scoped host root remains present");
        }
        match self.args.direct_binding_mut(target.root) {
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(Self::direct_access_error(target, "write"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.write_resolved_host(access, target, value)
            }),
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
        if let Some(binding) = self.extern_states.binding_mut(target.root) {
            return binding.object.mutate_resolved_host(access, target, op, rhs);
        }
        if self.host_arena.contains(target.root) {
            return self
                .host_arena
                .mutate(access, target, op, rhs)
                .expect("owned host root remains present");
        }
        if self.scoped_hosts.contains_key(&target.root) {
            return self
                .mutate_scoped_host(target.root, |object| {
                    object.mutate_resolved_host(access, target, op, rhs)
                })
                .expect("scoped host root remains present");
        }
        match self.args.direct_binding_mut(target.root) {
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(Self::direct_access_error(target, "write"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.mutate_resolved_host(access, target, op, rhs)
            }),
            None => self.fallback.mutate_host(access, target, op, rhs),
        }
    }

    fn remove_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        if let Some(binding) = self.extern_states.binding_mut(target.root) {
            return binding.object.remove_resolved_host(access, target);
        }
        if let Some(result) = self.host_arena.remove(access, target) {
            return result;
        }
        if let Some(result) = self.mutate_scoped_host(target.root, |object| {
            object.remove_resolved_host(access, target)
        }) {
            return result;
        }
        match self.args.direct_binding_mut(target.root) {
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(Self::direct_access_error(target, "write"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.remove_resolved_host(access, target)
            }),
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
        if let Some(binding) = self.extern_states.binding_mut(target.root) {
            return binding
                .object
                .call_resolved_host(access, target, method, args);
        }
        if let Some(result) = self.host_arena.call(access, target, method, args) {
            return result;
        }
        if let Some(result) = self.mutate_scoped_host(target.root, |object| {
            object.call_resolved_host(access, target, method, args)
        }) {
            return result;
        }
        match self.args.direct_binding_mut(target.root) {
            Some(binding) if binding.receiver_access() == HostLeaseKind::Shared => {
                Err(Self::direct_access_error(target, "call"))
            }
            Some(binding) => binding.mutate(target.root, |object| {
                object.call_resolved_host(access, target, method, args)
            }),
            None => self.fallback.call_host(access, target, method, args),
        }
    }
}

#[cfg(test)]
mod tests {
    use vela_vm::budget::ExecutionBudget;
    use vela_vm::heap::ScriptHeap;
    use vela_vm::value::Value;

    use super::{
        CallArgRuntime, EXECUTION_HOST_OBJECT_ID_BASE, ExecutionHost, ReentryExecutionHost,
    };
    use crate::runtime::{CallArgs, RuntimeExternStateBindings, RuntimeHostArena};

    #[test]
    fn direct_host_ids_are_allocated_by_the_execution_owner() {
        let shared = vec![1_i64];
        let mut mutable = vec![2_i64];
        let args = CallArgs::new()
            .with_host_ref("shared", &shared)
            .with_host_mut("mutable", &mut mutable);
        let mut extern_states = RuntimeExternStateBindings::new();
        let mut host_arena = RuntimeHostArena::new();

        let host = ExecutionHost::new(args, &mut extern_states, &mut host_arena);

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
        let mut extern_states = RuntimeExternStateBindings::new();
        let mut host_arena = RuntimeHostArena::new();
        let mut host = ExecutionHost::new(args, &mut extern_states, &mut host_arena);
        let mut heap = ScriptHeap::default();
        let mut budget = ExecutionBudget::unbounded();
        let program = vela_bytecode::LinkedProgram::new();

        let child_ref = {
            let child_args = CallArgs::new().with_host_ref("child", &child_value);
            let child = ReentryExecutionHost::new(child_args, &mut host)
                .expect("nested scope should allocate its direct binding");
            let values = child
                .resolve_values(
                    "child",
                    &["child".to_owned()],
                    &[false],
                    CallArgRuntime::new(1, &program, &mut heap, &mut budget),
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
