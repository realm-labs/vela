use std::sync::atomic::{AtomicU64, Ordering};

use vela_host::access::HostAccess;
#[cfg(test)]
use vela_host::adapter::ScriptStateAdapter;
use vela_hot_reload::runtime::HotReloadRuntime;
pub use vela_hot_reload::runtime::HotReloadStagingHandle;
use vela_hot_reload::version::ProgramVersion;
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap_execution::HeapExecution;
use vela_vm::owned_value::OwnedValue;
use vela_vm::{LinkedDriveOutcome, LinkedExecutionStart, persistent_value_to_owned_with_slots};
#[cfg(test)]
use vela_vm::{
    LinkedProgramHostBudgetCall, LinkedProgramHostCall, LinkedRuntimeCodeCall,
    PersistentHeapExecution,
};

use crate::engine::Engine;

mod bytecode_profile;
#[cfg(test)]
mod bytecode_profile_tests;
mod call_args;
mod call_future;
mod control;
mod detached_task;
pub(crate) mod execution_data;
mod execution_host;
mod extern_state_bindings;
pub(crate) mod handles;
mod host_arena;
mod image;
mod initialization;
mod inline_cache;
mod lifetime;
mod metadata;
mod options;
#[cfg(test)]
mod ownership_proof;
mod profile_api;
mod provider;
mod reentry;
mod reload_api;
mod service_egress;
mod state;
mod state_api;
mod task_continuation;
#[cfg(test)]
mod task_tests;
#[cfg(test)]
mod tests;
mod value_support;
mod vm_states;

pub use options::CallOptions;

pub use bytecode_profile::{
    BytecodeProfileSnapshot, FunctionBytecodeProfile, ScalarLoopBytecodeProfile,
    ScalarUnitBytecodeProfile,
};
pub use call_args::{
    CallArgs, DirectHostIdentity, ServiceScopedReturn, ServiceScopedReturnEnvelope,
};
pub use call_future::RuntimeCallFuture;
pub use control::{CallControl, CallSnapshot, CallStatus};
pub use extern_state_bindings::RuntimeExternStateBindings;
pub use handles::{
    RuntimeCallTarget, RuntimeMethodSelector, VelaFunction, VelaMethod, VelaMethodTarget,
};
pub use image::{OwnedImage, RuntimeImage, RuntimeImageStorage, SharedImage};
pub use initialization::{RuntimeBuildError, RuntimeBuilder, RuntimeInitializationLimits};
pub use provider::{ProviderHandle, ProviderMethodTarget};
pub use reload_api::{ReloadSource, RuntimeReloadError};
pub use service_egress::ServiceScopedReturnEgress;
pub use vm_states::{IntoStateValue, RuntimeVmStateStore, VelaValue};

use call_args::call_args_type_error;
use execution_host::ExecutionHost;
use handles::{
    RuntimeCallExecution, RuntimeCallTargetKind, RuntimeMethodResolveContext,
    RuntimeMethodSelectorKind,
};
use host_arena::RuntimeHostArena;
use reentry::{ActiveNativeReentry, invoke_prepared_async, invoke_prepared_context};
use state::RuntimeState;
pub(crate) use task_continuation::resume_task_continuation;
use value_support::{runtime_vm, unknown_function, unknown_method, value_type_id};
use vm_states::RuntimeValueRoots;

pub type Runtime = RuntimeImpl<OwnedImage>;
pub type SharedRuntime = RuntimeImpl<SharedImage>;

#[derive(Default)]
struct RuntimeServiceCall {
    dispatcher: Option<std::sync::Arc<dyn crate::service::ServiceCallDispatcher>>,
    pinned: Option<crate::service::PinnedServiceExecution>,
    scoped_return: Option<ServiceScopedReturnEgress>,
}

pub struct RuntimeImpl<I = OwnedImage>
where
    I: RuntimeImageStorage,
{
    image: I,
    hot_reload: Option<HotReloadRuntime>,
    state: RuntimeState,
}

static NEXT_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

fn next_runtime_id() -> u64 {
    NEXT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed)
}

impl RuntimeImpl<OwnedImage> {
    #[must_use]
    pub fn builder_from_linked_artifact(
        engine: Engine,
        artifact: std::sync::Arc<vela_bytecode::LinkedArtifact>,
    ) -> RuntimeBuilder<OwnedImage> {
        let image = OwnedImage::from_image(RuntimeImage::from_linked_artifact(engine, artifact));
        let state = RuntimeState::for_image(&image);
        RuntimeBuilder::new(Self {
            image,
            state,
            hot_reload: None,
        })
    }

    pub fn from_linked_artifact(
        engine: Engine,
        artifact: std::sync::Arc<vela_bytecode::LinkedArtifact>,
    ) -> Result<Self, RuntimeBuildError> {
        Self::builder_from_linked_artifact(engine, artifact).build()
    }

    /// Drops every execution-local owner without running script code. A pooled
    /// detached Runtime must call `initialize_detached_pool_state` before reuse.
    pub(crate) fn clear_detached_pool_state(&mut self) {
        debug_assert!(self.hot_reload.is_none());
        self.state = RuntimeState::for_image(&self.image);
    }

    pub(crate) fn initialize_detached_pool_state(&mut self) -> Result<(), RuntimeBuildError> {
        self.initialize_vm_states(RuntimeInitializationLimits::default())
    }

    pub fn builder(
        engine: Engine,
        program: vela_bytecode::compiler::CompiledProgram,
    ) -> Result<RuntimeBuilder<OwnedImage>, RuntimeBuildError> {
        let image = OwnedImage::from_image(RuntimeImage::try_new_compiled(engine, program)?);
        let state = RuntimeState::for_image(&image);
        Ok(RuntimeBuilder::new(Self {
            image,
            state,
            hot_reload: None,
        }))
    }

    pub fn new_compiled(
        engine: Engine,
        program: vela_bytecode::compiler::CompiledProgram,
    ) -> Result<Self, RuntimeBuildError> {
        Self::builder(engine, program)?.build()
    }

    /// Creates a runtime from a cohesive compiled generation.
    ///
    /// Unlinked bytecode is deliberately not a runtime input:
    ///
    /// ```compile_fail
    /// use vela_bytecode::UnlinkedProgram;
    /// use vela_engine::{engine::Engine, runtime::Runtime};
    /// let engine = Engine::builder().build().unwrap();
    /// let _ = Runtime::new(engine, UnlinkedProgram::new()).expect("runtime should initialize");
    /// ```
    pub fn new(
        engine: Engine,
        program: vela_bytecode::compiler::CompiledProgram,
    ) -> Result<Self, RuntimeBuildError> {
        Self::new_compiled(engine, program)
    }

    #[must_use]
    pub fn builder_from_hot_reload_version(
        engine: Engine,
        version: ProgramVersion,
    ) -> RuntimeBuilder<OwnedImage> {
        let image = OwnedImage::from_image(RuntimeImage::from_program_version(engine, &version));
        let state = RuntimeState::for_image(&image);
        RuntimeBuilder::new(Self {
            image,
            hot_reload: Some(HotReloadRuntime::new(version)),
            state,
        })
    }

    pub fn from_hot_reload_version(
        engine: Engine,
        version: ProgramVersion,
    ) -> Result<Self, RuntimeBuildError> {
        Self::builder_from_hot_reload_version(engine, version).build()
    }
}

impl RuntimeImpl<SharedImage> {
    #[must_use]
    pub fn builder_from_shared_image(image: SharedImage) -> RuntimeBuilder<SharedImage> {
        let state = RuntimeState::for_image(&image);
        RuntimeBuilder::new(Self {
            image,
            hot_reload: None,
            state,
        })
    }

    pub fn from_shared_image(image: SharedImage) -> Result<Self, RuntimeBuildError> {
        Self::builder_from_shared_image(image).build()
    }
}

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    #[must_use]
    pub fn engine(&self) -> &Engine {
        self.image.engine()
    }

    pub fn entry(&self, name: impl Into<String>) -> VmResult<VelaFunction> {
        let name = name.into();
        let linked_program = self.image.linked_program();
        let function = linked_program
            .entry_point_by_name(&name)
            .ok_or_else(|| unknown_function(name.clone()))?;
        let code = linked_program
            .function(function)
            .ok_or_else(|| unknown_function(name.clone()))?;
        Ok(VelaFunction {
            runtime_id: self.state.id,
            name,
            version_id: self.current_program_version_id(),
            params: code
                .params
                .iter()
                .map(|param| linked_program.debug_name(*param).to_owned())
                .collect(),
            param_defaults: code.param_defaults.clone(),
        })
    }

    pub fn method(&self, receiver: &VelaValue, method: impl Into<String>) -> VmResult<VelaMethod> {
        self.check_vela_value_runtime(receiver)?;
        let method = method.into();
        let receiver_type = self
            .value_type_id(receiver)
            .ok_or_else(|| unknown_method(method.clone()))?;
        let method_id = self
            .image
            .program_image()
            .script_methods()
            .get(receiver_type, &method)
            .map(|method| method.id)
            .ok_or_else(|| unknown_method(method.clone()))?;
        let code = self
            .image
            .program_image()
            .script_methods()
            .get_by_id(receiver_type, method_id)
            .and_then(|method| {
                let linked_program = self.image.linked_program();
                let function = linked_program.entry_point_by_id(method.function_id)?;
                linked_program.function(function)
            })
            .ok_or_else(|| unknown_method(method.clone()))?;
        let linked_program = self.image.linked_program();
        Ok(VelaMethod {
            runtime_id: self.state.id,
            receiver_type,
            name: method,
            method_id,
            version_id: self.current_program_version_id(),
            params: code
                .params
                .iter()
                .skip(1)
                .map(|param| linked_program.debug_name(*param).to_owned())
                .collect(),
            param_defaults: code.param_defaults.iter().skip(1).copied().collect(),
        })
    }

    pub fn call<'host, T>(
        &mut self,
        entry: T,
        args: CallArgs<'host>,
        options: CallOptions,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        self.call_impl(entry, args, options, false)
    }

    pub(crate) fn call_stable_function<'host>(
        &mut self,
        function: vela_def::FunctionId,
        diagnostic_name: impl Into<String>,
        args: CallArgs<'host>,
        options: CallOptions,
    ) -> VmResult<VelaValue> {
        self.call_impl(
            handles::StableVelaFunction {
                function,
                diagnostic_name: diagnostic_name.into(),
            },
            args,
            options,
            false,
        )
    }

    fn call_impl<'host, T>(
        &mut self,
        entry: T,
        args: CallArgs<'host>,
        options: CallOptions,
        allow_async_entry: bool,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        self.call_impl_with_service_dispatcher(entry, args, options, allow_async_entry, None)
    }

    fn call_impl_with_service_dispatcher<'host, T>(
        &mut self,
        entry: T,
        args: CallArgs<'host>,
        options: CallOptions,
        allow_async_entry: bool,
        service_dispatcher: Option<std::sync::Arc<dyn crate::service::ServiceCallDispatcher>>,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        self.call_impl_with_service_egress(
            entry,
            args,
            options,
            allow_async_entry,
            RuntimeServiceCall {
                dispatcher: service_dispatcher,
                ..RuntimeServiceCall::default()
            },
        )
    }

    fn call_impl_with_service_egress<'host, T>(
        &mut self,
        entry: T,
        args: CallArgs<'host>,
        options: CallOptions,
        allow_async_entry: bool,
        service: RuntimeServiceCall,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        let task_scope = options.task_scope().cloned();
        let mut budget = options.budget();
        let target = handles::call_target_sealed::Sealed::into_call_target(entry);
        let target = self.resolve_call_target(target, &mut budget)?;
        if target.asyncness.is_async() && !allow_async_entry {
            return Err(VmError::new(VmErrorKind::AsyncEntryRequiresCallAsync {
                name: target.name,
            }));
        }
        let state = &mut self.state;
        Self::call_runtime_args(RuntimeCallExecution {
            runtime_id: state.id,
            engine: self.image.engine(),
            registry_image: self.image.program_image(),
            artifact: self.image.linked_artifact(),
            hot_reload: self.hot_reload.as_ref(),
            extern_states: &mut state.extern_states,
            host_arena: &mut state.host_arena,
            host_slots: &mut state.host_slots,
            vm_states: &mut state.vm_states,
            generations: &mut state.generations,
            target,
            args,
            budget: &mut budget,
            service_dispatcher: service.dispatcher,
            pinned_service: service.pinned,
            service_scoped_return: service.scoped_return,
            task_scope,
        })
    }

    pub fn call_async<'call, 'args, T>(
        &'call mut self,
        entry: T,
        args: CallArgs<'args>,
        options: CallOptions,
    ) -> RuntimeCallFuture<'call>
    where
        T: RuntimeCallTarget + Send + 'call,
        'args: 'call,
    {
        let policy = options.call_policy();
        RuntimeCallFuture::new_controlled(self.call_impl_async(entry, args, options, None), policy)
    }

    async fn call_impl_async<'call, 'args, T>(
        &'call mut self,
        entry: T,
        args: CallArgs<'args>,
        options: CallOptions,
        pinned_service: Option<crate::service::PinnedServiceExecution>,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget + Send + 'call,
        'args: 'call,
    {
        self.call_impl_async_with_budget(entry, args, options, pinned_service)
            .await
            .map(|(value, _)| value)
    }

    async fn call_impl_async_with_budget<'call, 'args, T>(
        &'call mut self,
        entry: T,
        args: CallArgs<'args>,
        options: CallOptions,
        pinned_service: Option<crate::service::PinnedServiceExecution>,
    ) -> VmResult<(VelaValue, ExecutionBudget)>
    where
        T: RuntimeCallTarget + Send + 'call,
        'args: 'call,
    {
        let task_scope = options.task_scope().cloned();
        let service_dispatcher = pinned_service
            .as_ref()
            .map(|execution| std::sync::Arc::clone(execution.dispatcher()));
        let mut budget = options.budget();
        let target = handles::call_target_sealed::Sealed::into_call_target(entry);
        let target = self.resolve_call_target(target, &mut budget)?;
        let state = &mut self.state;
        let value = Self::call_runtime_args_async(RuntimeCallExecution {
            runtime_id: state.id,
            engine: self.image.engine(),
            registry_image: self.image.program_image(),
            artifact: self.image.linked_artifact(),
            hot_reload: self.hot_reload.as_ref(),
            extern_states: &mut state.extern_states,
            host_arena: &mut state.host_arena,
            host_slots: &mut state.host_slots,
            vm_states: &mut state.vm_states,
            generations: &mut state.generations,
            target,
            args,
            budget: &mut budget,
            service_dispatcher,
            pinned_service,
            service_scoped_return: None,
            task_scope,
        })
        .await?;
        Ok((value, budget))
    }

    pub fn bind_method<T>(&self, receiver: &VelaValue, method: T) -> VmResult<VelaMethodTarget>
    where
        T: RuntimeMethodSelector,
    {
        self.check_vela_value_runtime(receiver)?;
        let method = match handles::method_selector_sealed::Sealed::into_method_selector(method) {
            RuntimeMethodSelectorKind::Name(name) => self.method(receiver, name)?,
            RuntimeMethodSelectorKind::Method(method) => {
                if method.runtime_id != self.state.id {
                    return Err(call_args_type_error(
                        "VelaMethod belongs to another Runtime",
                    ));
                }
                let receiver_type = self
                    .value_type_id(receiver)
                    .ok_or_else(|| unknown_method(method.name.clone()))?;
                if receiver_type != method.receiver_type {
                    return Err(call_args_type_error(
                        "VelaMethod receiver type does not match value",
                    ));
                }
                method
            }
        };
        Ok(VelaMethodTarget {
            runtime_id: self.state.id,
            receiver: receiver.clone(),
            method,
        })
    }

    pub fn value_to_owned(&mut self, value: &VelaValue) -> VmResult<OwnedValue> {
        self.check_vela_value_runtime(value)?;
        persistent_value_to_owned_with_slots(
            &value.value,
            &mut self.state.vm_states.heap,
            &self.state.host_slots,
        )
    }

    #[cfg(feature = "serde")]
    pub fn from_value<T>(&self, value: &VelaValue) -> VmResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.check_vela_value_runtime(value)?;
        vela_vm::serde::from_runtime_value(&value.value, &self.state.vm_states.heap)
    }

    #[cfg(test)]
    pub(crate) fn call_raw(
        &mut self,
        entry: &str,
        args: &[OwnedValue],
        options: CallOptions,
        adapter: &mut (dyn ScriptStateAdapter + Send),
        access: &mut HostAccess,
    ) -> VmResult<OwnedValue> {
        let mut budget = options.budget();
        let execution_args =
            CallArgs::from_positional(args.iter().cloned()).with_fallback_adapter(adapter);
        let mut execution_host = ExecutionHost::new(
            execution_args,
            &mut self.state.extern_states,
            &mut self.state.host_arena,
            &mut self.state.host_slots,
        );
        let roots = self.state.vm_states.roots();
        let use_persistent_heap = options.managed_heap || !self.state.vm_states.is_empty();
        let mut host = HostExecution {
            adapter: &mut execution_host,
            access,
            state_values: Some(&mut self.state.vm_states.values),
        };
        let vm = if let Some(hot_reload) = self.hot_reload.as_ref() {
            let current = hot_reload.current();
            self.image
                .engine()
                .vm_for_artifact_with_abi(current.linked_artifact(), current.abi())
        } else {
            self.image
                .engine()
                .vm_for_artifact(self.image.linked_artifact())
        };
        if use_persistent_heap {
            vm.run_linked_program_host_call(LinkedProgramHostCall {
                artifact: self.image.linked_artifact(),
                entry,
                args,
                host: &mut host,
                persistent: PersistentHeapExecution {
                    heap: &mut self.state.vm_states.heap,
                    roots: &roots,
                },
                budget: &mut budget,
                inline_caches: Some(&self.state.generations),
                bytecode_profiler: self.state.generations.bytecode_profiler(),
            })
        } else {
            vm.run_linked_program_host_budget_call(LinkedProgramHostBudgetCall {
                artifact: self.image.linked_artifact(),
                entry,
                args,
                host: &mut host,
                budget: &mut budget,
                inline_caches: Some(&self.state.generations),
                bytecode_profiler: self.state.generations.bytecode_profiler(),
            })
        }
    }

    #[cfg(test)]
    pub(crate) fn retained_generation_count(&self) -> usize {
        self.state.retained_generation_count()
    }

    #[cfg(test)]
    pub(crate) fn retains_vm_state_id(&self, state: vela_def::StateId) -> bool {
        self.state.vm_states.values.get(state).is_some()
    }

    #[cfg(test)]
    pub(crate) fn retains_extern_state_id(&self, state: vela_def::StateId) -> bool {
        self.state.extern_states.contains_state_id(state)
    }

    #[cfg(test)]
    pub(crate) fn call_args_raw<'host>(
        &mut self,
        entry: &str,
        mut args: CallArgs<'host>,
        options: CallOptions,
        adapter: &'host mut (dyn ScriptStateAdapter + Send),
        access: &mut HostAccess,
    ) -> VmResult<OwnedValue> {
        let mut budget = options.budget();
        let linked_program = self.image.linked_program();
        let function = linked_program.entry_point_by_name(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        let code = linked_program.function(function).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        let params = code
            .params
            .iter()
            .map(|param| linked_program.debug_name(*param).to_owned())
            .collect::<Vec<_>>();
        let roots = self.state.vm_states.roots();
        args.set_fallback_adapter(adapter);
        let mut execution_host = ExecutionHost::new(
            args,
            &mut self.state.extern_states,
            &mut self.state.host_arena,
            &mut self.state.host_slots,
        );
        let resolved = execution_host.resolve_values(
            entry,
            &params,
            &code.param_defaults,
            call_args::CallArgRuntime::new(
                self.state.id,
                linked_program,
                &mut self.state.vm_states.heap,
                &mut budget,
            ),
        )?;
        let vm = if let Some(hot_reload) = self.hot_reload.as_ref() {
            let current = hot_reload.current();
            self.image
                .engine()
                .vm_for_artifact_with_abi(current.linked_artifact(), current.abi())
        } else {
            self.image
                .engine()
                .vm_for_artifact(self.image.linked_artifact())
        };
        let value = {
            let mut host = HostExecution {
                adapter: &mut execution_host,
                access,
                state_values: Some(&mut self.state.vm_states.values),
            };
            vm.run_linked_runtime_code_call(LinkedRuntimeCodeCall {
                artifact: self.image.linked_artifact(),
                function,
                args: &resolved,
                host: &mut host,
                persistent: PersistentHeapExecution {
                    heap: &mut self.state.vm_states.heap,
                    roots: &roots,
                },
                budget: &mut budget,
                inline_caches: Some(&self.state.generations),
                bytecode_profiler: self.state.generations.bytecode_profiler(),
            })
        }?;
        vela_vm::persistent_value_to_owned_with_host(
            &value,
            &mut self.state.vm_states.heap,
            &execution_host,
        )
    }

    fn call_runtime_args(call: RuntimeCallExecution<'_, '_, '_, '_>) -> VmResult<VelaValue> {
        let budget = call.budget;
        let mut execution_host = ExecutionHost::new(
            call.args,
            call.extern_states,
            call.host_arena,
            call.host_slots,
        );
        let resolved = execution_host.resolve_values(
            &call.target.name,
            &call.target.params,
            &call.target.param_defaults,
            call_args::CallArgRuntime::new(
                call.runtime_id,
                call.artifact.program(),
                &mut call.vm_states.heap,
                budget,
            ),
        )?;
        let mut access = HostAccess::new();
        let vm = runtime_vm(call.engine, call.artifact, call.hot_reload);
        let roots = call.vm_states.roots();
        let retained_values = std::sync::Arc::clone(&call.vm_states.retained_values);
        let vm_state_values = &mut call.vm_states.values;
        let mut entry_args = Vec::with_capacity(
            resolved
                .len()
                .saturating_add(usize::from(call.target.receiver.is_some())),
        );
        if let Some(receiver) = &call.target.receiver {
            entry_args.push(receiver.value);
        }
        entry_args.extend_from_slice(&resolved);
        let mut heap = HeapExecution::new(&mut call.vm_states.heap);
        let initial_host = HostExecution {
            adapter: &mut execution_host,
            access: &mut access,
            state_values: Some(&mut *vm_state_values),
        };
        let mut session = vm.start_linked_execution(
            LinkedExecutionStart {
                artifact: call.artifact,
                function: call.target.function,
                args: &entry_args,
                roots: &roots,
                inline_caches: Some(&*call.generations),
                bytecode_profiler: call.generations.bytecode_profiler(),
            },
            Some(&initial_host),
            &mut heap,
            budget,
        )?;
        session.enable_context_native_boundaries();

        loop {
            let outcome = {
                let mut host = HostExecution {
                    adapter: &mut execution_host,
                    access: &mut access,
                    state_values: Some(&mut *vm_state_values),
                };
                vm.drive_linked_execution(
                    &mut session,
                    Some(&mut host),
                    &mut heap,
                    budget,
                    Some(&*call.generations),
                    call.generations.bytecode_profiler(),
                )?
            };
            match outcome {
                LinkedDriveOutcome::Complete(value) => {
                    let value = vm.finish_linked_execution(value, &mut heap, &roots, budget);
                    if let Some(egress) = &call.service_scoped_return {
                        let owned = vela_vm::persistent_value_to_owned_with_host(
                            &value,
                            heap.heap,
                            &execution_host,
                        )?;
                        let returned = lifetime::validate_service_scoped_return(
                            owned,
                            &egress.identity,
                            egress.envelope,
                        )?;
                        egress.identity.complete_scoped_return(returned);
                    } else {
                        lifetime::validate_root_return(&value, heap.heap, &execution_host)?;
                    }
                    drop(heap);
                    return Ok(RuntimeValueRoots::retain(
                        &retained_values,
                        call.runtime_id,
                        value,
                    ));
                }
                LinkedDriveOutcome::ReentryComplete(_) => {
                    return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                        opcode: "unexpected root reentry completion",
                    }));
                }
                LinkedDriveOutcome::AsyncBoundary(prepared) => {
                    return Err(VmError::new(VmErrorKind::AsyncCallRequiresAwait {
                        name: prepared.name().to_owned(),
                    }));
                }
                LinkedDriveOutcome::TaskBoundary(prepared) => {
                    detached_task::admit(
                        call.engine,
                        call.task_scope.as_ref(),
                        call.pinned_service.as_ref(),
                        prepared,
                        heap.heap,
                        budget,
                    )?;
                }
                LinkedDriveOutcome::ContextBoundary(prepared) => {
                    let result = {
                        let mut active = ActiveNativeReentry {
                            runtime_id: call.runtime_id,
                            engine: call.engine,
                            registry_image: call.registry_image,
                            artifact: call.artifact,
                            vm: &vm,
                            session: &mut session,
                            host: &mut execution_host,
                            access: &mut access,
                            heap: &mut heap,
                            budget,
                            vm_state_values,
                            retained_values: std::sync::Arc::clone(&retained_values),
                            generations: &mut *call.generations,
                            service_dispatcher: call.service_dispatcher.as_deref(),
                        };
                        invoke_prepared_context(&prepared, &mut active)
                    };
                    let mut host = HostExecution {
                        adapter: &mut execution_host,
                        access: &mut access,
                        state_values: Some(&mut *vm_state_values),
                    };
                    vm.resume_linked_context_call(
                        &mut session,
                        result,
                        Some(&mut host),
                        Some(&mut heap),
                        Some(budget),
                    )?;
                }
            }
        }
    }

    async fn call_runtime_args_async(
        call: RuntimeCallExecution<'_, '_, '_, '_>,
    ) -> VmResult<VelaValue> {
        let budget = call.budget;
        let mut execution_host = ExecutionHost::new(
            call.args,
            call.extern_states,
            call.host_arena,
            call.host_slots,
        );
        let resolved = execution_host.resolve_values(
            &call.target.name,
            &call.target.params,
            &call.target.param_defaults,
            call_args::CallArgRuntime::new(
                call.runtime_id,
                call.artifact.program(),
                &mut call.vm_states.heap,
                budget,
            ),
        )?;
        let mut access = HostAccess::new();
        let vm = runtime_vm(call.engine, call.artifact, call.hot_reload);
        let roots = call.vm_states.roots();
        let retained_values = std::sync::Arc::clone(&call.vm_states.retained_values);
        let vm_state_values = &mut call.vm_states.values;
        let mut entry_args = Vec::with_capacity(
            resolved
                .len()
                .saturating_add(usize::from(call.target.receiver.is_some())),
        );
        if let Some(receiver) = &call.target.receiver {
            entry_args.push(receiver.value);
        }
        entry_args.extend_from_slice(&resolved);
        let mut heap = HeapExecution::new(&mut call.vm_states.heap);
        let initial_host = HostExecution {
            adapter: &mut execution_host,
            access: &mut access,
            state_values: Some(&mut *vm_state_values),
        };
        let mut session = vm.start_linked_execution(
            LinkedExecutionStart {
                artifact: call.artifact,
                function: call.target.function,
                args: &entry_args,
                roots: &roots,
                inline_caches: Some(&*call.generations),
                bytecode_profiler: call.generations.bytecode_profiler(),
            },
            Some(&initial_host),
            &mut heap,
            budget,
        )?;
        session.enable_context_native_boundaries();

        loop {
            let outcome = {
                let mut host = HostExecution {
                    adapter: &mut execution_host,
                    access: &mut access,
                    state_values: Some(&mut *vm_state_values),
                };
                vm.drive_linked_execution(
                    &mut session,
                    Some(&mut host),
                    &mut heap,
                    budget,
                    Some(&*call.generations),
                    call.generations.bytecode_profiler(),
                )?
            };
            match outcome {
                LinkedDriveOutcome::Complete(value) => {
                    let value = vm.finish_linked_execution(value, &mut heap, &roots, budget);
                    lifetime::validate_root_return(&value, heap.heap, &execution_host)?;
                    drop(heap);
                    return Ok(RuntimeValueRoots::retain(
                        &retained_values,
                        call.runtime_id,
                        value,
                    ));
                }
                LinkedDriveOutcome::ReentryComplete(_) => {
                    return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                        opcode: "unexpected root reentry completion",
                    }));
                }
                LinkedDriveOutcome::AsyncBoundary(prepared) => {
                    lifetime::validate_async_suspend(&session, heap.heap, &execution_host)?;
                    let result = {
                        let mut active = ActiveNativeReentry {
                            runtime_id: call.runtime_id,
                            engine: call.engine,
                            registry_image: call.registry_image,
                            artifact: call.artifact,
                            vm: &vm,
                            session: &mut session,
                            host: &mut execution_host,
                            access: &mut access,
                            heap: &mut heap,
                            budget,
                            vm_state_values,
                            retained_values: std::sync::Arc::clone(&retained_values),
                            generations: &mut *call.generations,
                            service_dispatcher: call.service_dispatcher.as_deref(),
                        };
                        invoke_prepared_async(&prepared, &mut active).await
                    };
                    let mut host = HostExecution {
                        adapter: &mut execution_host,
                        access: &mut access,
                        state_values: Some(&mut *vm_state_values),
                    };
                    vm.resume_linked_async_call(
                        &mut session,
                        result,
                        Some(&mut host),
                        Some(&mut heap),
                        Some(budget),
                    )?;
                }
                LinkedDriveOutcome::TaskBoundary(prepared) => {
                    detached_task::admit(
                        call.engine,
                        call.task_scope.as_ref(),
                        call.pinned_service.as_ref(),
                        prepared,
                        heap.heap,
                        budget,
                    )?;
                }
                LinkedDriveOutcome::ContextBoundary(prepared) => {
                    let result = {
                        let mut active = ActiveNativeReentry {
                            runtime_id: call.runtime_id,
                            engine: call.engine,
                            registry_image: call.registry_image,
                            artifact: call.artifact,
                            vm: &vm,
                            session: &mut session,
                            host: &mut execution_host,
                            access: &mut access,
                            heap: &mut heap,
                            budget,
                            vm_state_values,
                            retained_values: std::sync::Arc::clone(&retained_values),
                            generations: &mut *call.generations,
                            service_dispatcher: call.service_dispatcher.as_deref(),
                        };
                        invoke_prepared_context(&prepared, &mut active)
                    };
                    let mut host = HostExecution {
                        adapter: &mut execution_host,
                        access: &mut access,
                        state_values: Some(&mut *vm_state_values),
                    };
                    vm.resume_linked_context_call(
                        &mut session,
                        result,
                        Some(&mut host),
                        Some(&mut heap),
                        Some(budget),
                    )?;
                }
            }
        }
    }

    fn resolve_call_target(
        &mut self,
        target: RuntimeCallTargetKind,
        budget: &mut ExecutionBudget,
    ) -> VmResult<handles::EntryRequest> {
        match target {
            target @ (RuntimeCallTargetKind::FunctionName(_)
            | RuntimeCallTargetKind::Function(_)
            | RuntimeCallTargetKind::StableFunction(_)) => handles::resolve_function_target(
                target,
                self.state.id,
                self.image.linked_program(),
                self.current_program_version_id(),
            ),
            RuntimeCallTargetKind::BoundMethod(target) => handles::resolve_bound_method(
                target,
                RuntimeMethodResolveContext {
                    runtime_id: self.state.id,
                    program_image: self.image.program_image(),
                    linked_program: self.image.linked_program(),
                    version_id: self.current_program_version_id(),
                    script_heap: &self.state.vm_states.heap,
                    engine: self.image.engine(),
                    host: handles::RuntimeHostResolver::Slots(&self.state.host_slots),
                },
            ),
            RuntimeCallTargetKind::ProviderMethod(target) => {
                self.resolve_provider_target(target, budget)
            }
        }
    }
}
