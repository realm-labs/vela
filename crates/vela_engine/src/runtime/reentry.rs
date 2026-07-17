use vela_bytecode::ProgramImage;
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::path::HostRef;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap_execution::HeapExecution;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;
use vela_vm::{
    HostExecution, LinkedDriveOutcome, LinkedExecutionReentry, LinkedExecutionSession,
    PreparedAsyncCall, PreparedContextCall, VmStateValues,
};

use crate::context::{NativeCallContext, NativeContextLeaseInvoker, NativeReentry};
use crate::engine::Engine;
use crate::method::AsyncNativeMethodImplementation;

use super::call_args::{CallArgs, call_args_type_error};
use super::call_future::RuntimeCallFuture;
use super::execution_host::{DirectContextInvoker, ExecutionHostBoundary, ReentryExecutionHost};
use super::handles::{
    self, RuntimeCallTargetKind, RuntimeMethodResolveContext, RuntimeMethodSelectorKind,
};
use super::provider;
use super::state;
use super::vm_states::{RuntimeValueRoots, VelaValue};
use super::{VelaMethod, VelaMethodTarget, unknown_method, value_type_id};

pub(super) struct ActiveNativeReentry<'execution, 'heap> {
    pub(super) runtime_id: u64,
    pub(super) engine: &'execution Engine,
    pub(super) registry_image: &'execution ProgramImage,
    pub(super) artifact: &'execution std::sync::Arc<vela_bytecode::LinkedArtifact>,
    pub(super) dispatch_generation: Option<std::sync::Arc<crate::dispatch::DispatchGeneration>>,
    pub(super) vm: &'execution vela_vm::Vm,
    pub(super) session: &'execution mut LinkedExecutionSession,
    pub(super) host: &'execution mut dyn ExecutionHostBoundary,
    pub(super) access: &'execution mut HostAccess,
    pub(super) heap: &'execution mut HeapExecution<'heap>,
    pub(super) budget: &'execution mut ExecutionBudget,
    pub(super) vm_state_values: &'execution mut VmStateValues,
    pub(super) retained_values: std::sync::Arc<std::sync::Mutex<RuntimeValueRoots>>,
    pub(super) generations: &'execution mut state::RuntimeGenerations,
}

impl ActiveNativeReentry<'_, '_> {
    fn resolve_target(&mut self, target: RuntimeCallTargetKind) -> VmResult<handles::EntryRequest> {
        match target {
            target @ (RuntimeCallTargetKind::FunctionName(_)
            | RuntimeCallTargetKind::Function(_)
            | RuntimeCallTargetKind::StableFunction(_)) => handles::resolve_function_target(
                target,
                self.runtime_id,
                self.artifact.program(),
                None,
            ),
            RuntimeCallTargetKind::BoundMethod(target) => handles::resolve_bound_method(
                target,
                RuntimeMethodResolveContext {
                    runtime_id: self.runtime_id,
                    program_image: self.registry_image,
                    linked_program: self.artifact.program(),
                    version_id: None,
                    script_heap: self.heap.heap,
                    engine: self.engine,
                },
            ),
            RuntimeCallTargetKind::ProviderMethod(target) => {
                provider::resolve_provider_reentry_target(
                    target,
                    self.runtime_id,
                    self.artifact,
                    self.heap,
                    self.budget,
                    &self.retained_values,
                )
            }
        }
    }

    fn method_handle(&self, receiver: &VelaValue, name: String) -> VmResult<VelaMethod> {
        if receiver.runtime_id != self.runtime_id {
            return Err(call_args_type_error("VelaValue belongs to another Runtime"));
        }
        let receiver_type = value_type_id(
            &receiver.value,
            self.heap.heap,
            self.engine.registry().as_ref(),
        )
        .ok_or_else(|| unknown_method(name.clone()))?;
        let method_id = self
            .registry_image
            .script_methods()
            .get(receiver_type, &name)
            .map(|method| method.id)
            .ok_or_else(|| unknown_method(name.clone()))?;
        let code = self
            .registry_image
            .script_methods()
            .get_by_id(receiver_type, method_id)
            .and_then(|method| {
                let function = self
                    .artifact
                    .program()
                    .entry_point_by_id(method.function_id)?;
                self.artifact.program().function(function)
            })
            .ok_or_else(|| unknown_method(name.clone()))?;
        Ok(VelaMethod {
            runtime_id: self.runtime_id,
            receiver_type,
            name,
            method_id,
            version_id: None,
            params: code
                .params
                .iter()
                .skip(1)
                .map(|param| self.artifact.program().debug_name(*param).to_owned())
                .collect(),
            param_defaults: code.param_defaults.iter().skip(1).copied().collect(),
        })
    }

    fn drive_sync<'args>(
        &mut self,
        target: handles::EntryRequest,
        args: CallArgs<'args>,
    ) -> VmResult<VelaValue> {
        if target.asyncness.is_async() {
            return Err(VmError::new(VmErrorKind::AsyncEntryRequiresCallAsync {
                name: target.name,
            }));
        }
        let runtime_id = self.runtime_id;
        let retained_values = std::sync::Arc::clone(&self.retained_values);
        let mut child_host = ReentryExecutionHost::new(args, self.host)?;
        let resolved = child_host.resolve_values(
            &target.name,
            &target.params,
            &target.param_defaults,
            self.runtime_id,
            self.heap.heap,
            self.budget,
        )?;
        let entry_args = reentry_entry_args(&target, &resolved);
        self.vm.push_linked_reentry(
            self.session,
            LinkedExecutionReentry {
                artifact: self.artifact,
                function: target.function,
                args: &entry_args,
                inline_caches: Some(&*self.generations),
                bytecode_profiler: self.generations.bytecode_profiler(),
            },
            self.heap,
            self.budget,
        )?;
        loop {
            let outcome = {
                let mut host = HostExecution {
                    adapter: &mut child_host,
                    access: self.access,
                    state_values: Some(&mut *self.vm_state_values),
                };
                match self.vm.drive_linked_execution(
                    self.session,
                    Some(&mut host),
                    self.heap,
                    self.budget,
                    Some(&*self.generations),
                    self.generations.bytecode_profiler(),
                ) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        self.vm
                            .abort_linked_reentry(self.session, self.heap, self.budget)?;
                        return Err(error);
                    }
                }
            };
            match outcome {
                LinkedDriveOutcome::ReentryComplete(value) => {
                    let (value, active_root) = value.into_parts();
                    return Ok(RuntimeValueRoots::retain_active(
                        &retained_values,
                        runtime_id,
                        value,
                        active_root,
                    ));
                }
                LinkedDriveOutcome::AsyncBoundary(call) => {
                    let name = call.name().to_owned();
                    self.vm
                        .abort_linked_reentry(self.session, self.heap, self.budget)?;
                    return Err(VmError::new(VmErrorKind::AsyncCallRequiresAwait { name }));
                }
                LinkedDriveOutcome::ContextBoundary(prepared) => {
                    let result = {
                        let mut nested = ActiveNativeReentry {
                            runtime_id: self.runtime_id,
                            engine: self.engine,
                            registry_image: self.registry_image,
                            artifact: self.artifact,
                            dispatch_generation: self.dispatch_generation.clone(),
                            vm: self.vm,
                            session: self.session,
                            host: &mut child_host,
                            access: self.access,
                            heap: self.heap,
                            budget: self.budget,
                            vm_state_values: &mut *self.vm_state_values,
                            retained_values: std::sync::Arc::clone(&self.retained_values),
                            generations: self.generations,
                        };
                        invoke_prepared_context(&prepared, &mut nested)
                    };
                    if let Err(error) = self.vm.resume_linked_context_call(
                        self.session,
                        result,
                        Some(self.heap),
                        Some(self.budget),
                    ) {
                        self.vm
                            .abort_linked_reentry(self.session, self.heap, self.budget)?;
                        return Err(error);
                    }
                }
                LinkedDriveOutcome::Complete(_) => {
                    return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                        opcode: "root completed while driving native reentry",
                    }));
                }
            }
        }
    }

    async fn drive_async<'call, 'args>(
        &'call mut self,
        target: handles::EntryRequest,
        args: CallArgs<'args>,
    ) -> VmResult<VelaValue>
    where
        'args: 'call,
    {
        let runtime_id = self.runtime_id;
        let retained_values = std::sync::Arc::clone(&self.retained_values);
        let mut child_host = ReentryExecutionHost::new(args, self.host)?;
        let resolved = child_host.resolve_values(
            &target.name,
            &target.params,
            &target.param_defaults,
            self.runtime_id,
            self.heap.heap,
            self.budget,
        )?;
        let entry_args = reentry_entry_args(&target, &resolved);
        self.vm.push_linked_reentry(
            self.session,
            LinkedExecutionReentry {
                artifact: self.artifact,
                function: target.function,
                args: &entry_args,
                inline_caches: Some(&*self.generations),
                bytecode_profiler: self.generations.bytecode_profiler(),
            },
            self.heap,
            self.budget,
        )?;

        loop {
            let outcome = {
                let mut host = HostExecution {
                    adapter: &mut child_host,
                    access: self.access,
                    state_values: Some(&mut *self.vm_state_values),
                };
                match self.vm.drive_linked_execution(
                    self.session,
                    Some(&mut host),
                    self.heap,
                    self.budget,
                    Some(&*self.generations),
                    self.generations.bytecode_profiler(),
                ) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        self.vm
                            .abort_linked_reentry(self.session, self.heap, self.budget)?;
                        return Err(error);
                    }
                }
            };
            match outcome {
                LinkedDriveOutcome::ReentryComplete(value) => {
                    let (value, active_root) = value.into_parts();
                    return Ok(RuntimeValueRoots::retain_active(
                        &retained_values,
                        runtime_id,
                        value,
                        active_root,
                    ));
                }
                LinkedDriveOutcome::Complete(_) => {
                    return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                        opcode: "root completed while driving native reentry",
                    }));
                }
                LinkedDriveOutcome::AsyncBoundary(prepared) => {
                    let result = {
                        let mut nested = ActiveNativeReentry {
                            runtime_id: self.runtime_id,
                            engine: self.engine,
                            registry_image: self.registry_image,
                            artifact: self.artifact,
                            dispatch_generation: self.dispatch_generation.clone(),
                            vm: self.vm,
                            session: self.session,
                            host: &mut child_host,
                            access: self.access,
                            heap: self.heap,
                            budget: self.budget,
                            vm_state_values: &mut *self.vm_state_values,
                            retained_values: std::sync::Arc::clone(&self.retained_values),
                            generations: self.generations,
                        };
                        invoke_prepared_async(&prepared, &mut nested).await
                    };
                    if let Err(error) = self.vm.resume_linked_async_call(
                        self.session,
                        result,
                        Some(self.heap),
                        Some(self.budget),
                    ) {
                        self.vm
                            .abort_linked_reentry(self.session, self.heap, self.budget)?;
                        return Err(error);
                    }
                }
                LinkedDriveOutcome::ContextBoundary(prepared) => {
                    let result = {
                        let mut nested = ActiveNativeReentry {
                            runtime_id: self.runtime_id,
                            engine: self.engine,
                            registry_image: self.registry_image,
                            artifact: self.artifact,
                            dispatch_generation: self.dispatch_generation.clone(),
                            vm: self.vm,
                            session: self.session,
                            host: &mut child_host,
                            access: self.access,
                            heap: self.heap,
                            budget: self.budget,
                            vm_state_values: &mut *self.vm_state_values,
                            retained_values: std::sync::Arc::clone(&self.retained_values),
                            generations: self.generations,
                        };
                        invoke_prepared_context(&prepared, &mut nested)
                    };
                    if let Err(error) = self.vm.resume_linked_context_call(
                        self.session,
                        result,
                        Some(self.heap),
                        Some(self.budget),
                    ) {
                        self.vm
                            .abort_linked_reentry(self.session, self.heap, self.budget)?;
                        return Err(error);
                    }
                }
            }
        }
    }
}

impl NativeReentry for ActiveNativeReentry<'_, '_> {
    fn binding_schema(&self) -> &vela_bytecode::RustBindingSchema {
        self.artifact.binding_schema()
    }

    fn dispatch_generation(&self) -> Option<&std::sync::Arc<crate::dispatch::DispatchGeneration>> {
        self.dispatch_generation.as_ref()
    }

    fn value_to_owned(&mut self, value: &VelaValue) -> VmResult<OwnedValue> {
        if value.runtime_id() != self.runtime_id {
            return Err(call_args_type_error("VelaValue belongs to another Runtime"));
        }
        vela_vm::persistent_value_to_owned(&value.value(), self.heap.heap)
    }

    fn adapter(&mut self) -> &mut dyn ScriptStateAdapter {
        self.host
    }

    fn access(&mut self) -> &mut HostAccess {
        self.access
    }

    fn host_execution(&mut self) -> HostExecution<'_> {
        HostExecution {
            adapter: self.host,
            access: self.access,
            state_values: Some(&mut *self.vm_state_values),
        }
    }

    fn budget(&self) -> Option<&ExecutionBudget> {
        Some(self.budget)
    }

    fn budget_mut(&mut self) -> Option<&mut ExecutionBudget> {
        Some(self.budget)
    }

    fn with_host_leases(
        &mut self,
        requests: &[(HostRef, vela_host::lease::HostLeaseKind)],
        effect_ceiling: vela_common::CapabilitySet,
        invoke: &mut NativeContextLeaseInvoker<'_>,
    ) -> VmResult<()> {
        let runtime_id = self.runtime_id;
        let engine = self.engine;
        let registry_image = self.registry_image;
        let artifact = self.artifact;
        let vm = self.vm;
        let retained_values = std::sync::Arc::clone(&self.retained_values);
        let session = &mut *self.session;
        let access = &mut *self.access;
        let heap = &mut *self.heap;
        let budget = &mut *self.budget;
        let vm_state_values = &mut *self.vm_state_values;
        let generations = &mut *self.generations;
        self.host
            .with_execution_host_leases(requests, &mut |leases, leased_host| {
                let mut nested_reentry = ActiveNativeReentry {
                    runtime_id,
                    engine,
                    registry_image,
                    artifact,
                    dispatch_generation: self.dispatch_generation.clone(),
                    vm,
                    session: &mut *session,
                    host: leased_host,
                    access: &mut *access,
                    heap: &mut *heap,
                    budget: &mut *budget,
                    vm_state_values: &mut *vm_state_values,
                    retained_values: std::sync::Arc::clone(&retained_values),
                    generations: &mut *generations,
                };
                let mut context =
                    NativeCallContext::new_reentry(engine, &mut nested_reentry, effect_ceiling);
                context.set_host_provenance(requests, leases);
                invoke(leases, &mut context)
            })
    }

    fn call<'args>(
        &mut self,
        target: RuntimeCallTargetKind,
        args: CallArgs<'args>,
    ) -> VmResult<VelaValue> {
        let target = self.resolve_target(target)?;
        self.drive_sync(target, args)
    }

    fn call_async<'call, 'args>(
        &'call mut self,
        target: RuntimeCallTargetKind,
        args: CallArgs<'args>,
    ) -> RuntimeCallFuture<'call>
    where
        'args: 'call,
    {
        RuntimeCallFuture::new(async move {
            let target = self.resolve_target(target)?;
            self.drive_async(target, args).await
        })
    }

    fn bind_method(
        &mut self,
        receiver: &VelaValue,
        method: RuntimeMethodSelectorKind,
    ) -> VmResult<VelaMethodTarget> {
        let method = match method {
            RuntimeMethodSelectorKind::Name(name) => self.method_handle(receiver, name)?,
            RuntimeMethodSelectorKind::Method(method) => method,
        };
        let target = VelaMethodTarget {
            runtime_id: self.runtime_id,
            receiver: receiver.clone(),
            method,
        };
        handles::resolve_bound_method(
            target.clone(),
            RuntimeMethodResolveContext {
                runtime_id: self.runtime_id,
                program_image: self.registry_image,
                linked_program: self.artifact.program(),
                version_id: None,
                script_heap: self.heap.heap,
                engine: self.engine,
            },
        )?;
        Ok(target)
    }
}

fn reentry_entry_args(target: &handles::EntryRequest, resolved: &[Value]) -> Vec<Value> {
    let mut entry_args = Vec::with_capacity(
        resolved
            .len()
            .saturating_add(usize::from(target.receiver.is_some())),
    );
    if let Some(receiver) = &target.receiver {
        entry_args.push(receiver.value);
    }
    entry_args.extend_from_slice(resolved);
    entry_args
}

struct RuntimeDirectContextInvoker<'execution, 'heap> {
    pub(super) runtime_id: u64,
    pub(super) engine: &'execution Engine,
    pub(super) registry_image: &'execution ProgramImage,
    pub(super) artifact: &'execution std::sync::Arc<vela_bytecode::LinkedArtifact>,
    pub(super) dispatch_generation: Option<std::sync::Arc<crate::dispatch::DispatchGeneration>>,
    pub(super) vm: &'execution vela_vm::Vm,
    pub(super) session: &'execution mut LinkedExecutionSession,
    pub(super) access: &'execution mut HostAccess,
    pub(super) heap: &'execution mut HeapExecution<'heap>,
    pub(super) budget: &'execution mut ExecutionBudget,
    pub(super) vm_state_values: &'execution mut VmStateValues,
    pub(super) retained_values: std::sync::Arc<std::sync::Mutex<RuntimeValueRoots>>,
    pub(super) generations: &'execution mut state::RuntimeGenerations,
    root: HostRef,
    args: Vec<OwnedValue>,
    function: crate::method::AsyncContextDirectNativeMethodFunction,
    effect_ceiling: vela_common::CapabilitySet,
}

impl DirectContextInvoker for RuntimeDirectContextInvoker<'_, '_> {
    fn invoke<'invoke, 'lease>(
        self: Box<Self>,
        leases: &'invoke mut [vela_host::lease::ErasedHostLease<'lease>],
        host: &'invoke mut dyn ExecutionHostBoundary,
    ) -> vela_vm::NativeCallFuture<'invoke>
    where
        Self: 'invoke,
    {
        Box::pin(async move {
            let mut nested = ActiveNativeReentry {
                runtime_id: self.runtime_id,
                engine: self.engine,
                registry_image: self.registry_image,
                artifact: self.artifact,
                dispatch_generation: self.dispatch_generation.clone(),
                vm: self.vm,
                session: self.session,
                host,
                access: self.access,
                heap: self.heap,
                budget: self.budget,
                vm_state_values: &mut *self.vm_state_values,
                retained_values: self.retained_values,
                generations: self.generations,
            };
            let engine = self.engine.clone();
            let mut context =
                NativeCallContext::new_reentry(&engine, &mut nested, self.effect_ceiling);
            (self.function)(self.root, leases, self.args, &mut context).await
        })
    }
}

pub(super) async fn invoke_prepared_async(
    prepared: &PreparedAsyncCall,
    active: &mut ActiveNativeReentry<'_, '_>,
) -> VmResult<OwnedValue> {
    if let Some(entry) = prepared
        .native_id()
        .and_then(|id| active.engine.async_context_host_native_function(id))
    {
        crate::engine::check_capabilities(
            &entry.desc.name,
            &entry.desc.effects,
            active.engine.capabilities(),
        )?;
        let engine = active.engine.clone();
        let function = std::sync::Arc::clone(&entry.function);
        let args = prepared.args().to_vec();
        let mut context = NativeCallContext::new_reentry(
            &engine,
            active,
            entry.desc.effects.required_capability_set(),
        );
        return function(&args, &mut context).await;
    }
    if let Some(entry) = prepared
        .method_id()
        .and_then(|id| active.engine.async_native_method(id))
        && let AsyncNativeMethodImplementation::DirectContext {
            lease_kind,
            param_leases,
            function,
        } = &entry.function
    {
        crate::engine::check_capabilities(
            &entry.desc.name,
            &entry.desc.effects,
            active.engine.capabilities(),
        )?;
        let (root, prepared_kind) = prepared.host_lease_request().ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "context direct method lease",
            })
        })?;
        if prepared_kind != *lease_kind {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "context direct method lease kind",
            }));
        }
        let mut requests = Vec::with_capacity(param_leases.len().saturating_add(1));
        requests.push((root, *lease_kind));
        for (index, kind) in param_leases {
            let Some(OwnedValue::HostRef(root)) = prepared.args().get(*index) else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "typed direct method host lease parameter",
                }));
            };
            requests.push((*root, *kind));
        }
        let invoke = RuntimeDirectContextInvoker {
            runtime_id: active.runtime_id,
            engine: active.engine,
            registry_image: active.registry_image,
            artifact: active.artifact,
            dispatch_generation: active.dispatch_generation.clone(),
            vm: active.vm,
            session: &mut *active.session,
            access: &mut *active.access,
            heap: &mut *active.heap,
            budget: &mut *active.budget,
            vm_state_values: &mut *active.vm_state_values,
            retained_values: std::sync::Arc::clone(&active.retained_values),
            generations: &mut *active.generations,
            root,
            args: prepared.args().to_vec(),
            function: std::sync::Arc::clone(function),
            effect_ceiling: entry.desc.effects.required_capability_set(),
        };
        return active
            .host
            .invoke_direct_context(requests, Box::new(invoke))
            .await;
    }
    if prepared.requires_host_lease_set() {
        return active.host.invoke_prepared_with_leases(prepared).await;
    }
    if prepared.requires_host_lease() {
        return active.host.invoke_prepared_with_lease(prepared).await;
    }
    if prepared.requires_host() {
        let mut host = HostExecution {
            adapter: active.host,
            access: active.access,
            state_values: Some(&mut *active.vm_state_values),
        };
        return prepared
            .invoke_with_host(&mut host, Some(active.budget))
            .await;
    }
    prepared.invoke().await
}

pub(super) fn invoke_prepared_context(
    prepared: &PreparedContextCall,
    active: &mut ActiveNativeReentry<'_, '_>,
) -> VmResult<OwnedValue> {
    let entry = active
        .engine
        .context_host_native_function(prepared.native_id())
        .ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownNative {
                name: prepared.name().to_owned(),
            })
        })?;
    crate::engine::check_capabilities(
        &entry.desc.name,
        &entry.desc.effects,
        active.engine.capabilities(),
    )?;
    let engine = active.engine.clone();
    let function = std::sync::Arc::clone(&entry.function);
    let args = prepared.args().to_vec();
    let mut context = NativeCallContext::new_reentry(
        &engine,
        active,
        entry.desc.effects.required_capability_set(),
    );
    function(&args, &mut context)
}
