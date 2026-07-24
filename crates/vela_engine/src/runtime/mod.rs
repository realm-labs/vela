use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use vela_bytecode::ProgramImage;
use vela_common::SourceId;
use vela_host::access::HostAccess;
#[cfg(test)]
use vela_host::adapter::ScriptStateAdapter;
use vela_hot_reload::error::HotReloadResult;
use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::runtime::HotReloadRuntime;
pub use vela_hot_reload::runtime::HotReloadStagingHandle;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_hot_reload::version::{HotUpdate, ProgramVersion};
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap::{HeapValue, ScriptHeap};
use vela_vm::heap_execution::HeapExecution;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;
use vela_vm::{LinkedDriveOutcome, LinkedExecutionStart, persistent_value_to_owned_with_slots};
#[cfg(test)]
use vela_vm::{
    LinkedProgramHostBudgetCall, LinkedProgramHostCall, LinkedRuntimeCodeCall,
    PersistentHeapExecution,
};

use crate::engine::Engine;
use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::reload::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};

mod bytecode_profile;
#[cfg(test)]
mod bytecode_profile_tests;
mod call_args;
mod call_future;
pub(crate) mod execution_data;
mod execution_host;
mod extern_state_bindings;
pub(crate) mod handles;
mod host_arena;
mod image;
mod initialization;
mod inline_cache;
mod options;
#[cfg(test)]
mod ownership_proof;
mod profile_api;
mod provider;
mod reentry;
mod state;
mod state_api;
#[cfg(test)]
mod tests;
mod vm_states;

pub use options::CallOptions;

pub use bytecode_profile::{BytecodeProfileSnapshot, FunctionBytecodeProfile};
pub use call_args::{CallArgs, DirectHostIdentity};
pub use call_future::RuntimeCallFuture;
pub use extern_state_bindings::RuntimeExternStateBindings;
pub use handles::{
    RuntimeCallTarget, RuntimeMethodSelector, VelaFunction, VelaMethod, VelaMethodTarget,
};
pub use image::{OwnedImage, RuntimeImage, RuntimeImageStorage, SharedImage};
pub use initialization::{RuntimeBuildError, RuntimeBuilder, RuntimeInitializationLimits};
pub use provider::{ProviderHandle, ProviderMethodTarget};
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
use vm_states::RuntimeValueRoots;

pub type Runtime = RuntimeImpl<OwnedImage>;
pub type SharedRuntime = RuntimeImpl<SharedImage>;

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

    #[must_use]
    pub fn hot_reload_version(&self) -> Option<std::sync::Arc<ProgramVersion>> {
        self.hot_reload.as_ref().map(HotReloadRuntime::current)
    }

    /// Returns a staging-only handle that may queue an update while an async
    /// outer call has the Runtime borrowed.
    ///
    /// The handle cannot activate the update. Call `check_reload` on the
    /// Runtime after the outer call completes or is cancelled.
    #[must_use]
    pub fn hot_reload_staging_handle(&self) -> Option<HotReloadStagingHandle> {
        self.hot_reload
            .as_ref()
            .map(HotReloadRuntime::staging_handle)
    }

    pub fn apply_hot_update(&mut self, update: HotUpdate) -> EngineResult<HotReloadReport> {
        self.apply_hot_update_result_report(Ok(update))
    }

    pub fn stage_hot_update(&mut self, update: HotUpdate) -> EngineResult<()> {
        self.stage_hot_update_result(Ok(update))
    }

    pub fn stage_hot_update_result(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> EngineResult<()> {
        let Some(hot_reload) = self.hot_reload.as_mut() else {
            return Err(EngineError::new(
                EngineErrorKind::RuntimeNotHotReloadEnabled,
            ));
        };
        let _replaced = hot_reload.stage_hot_update_result(update);
        Ok(())
    }

    pub fn stage_hot_reload_update(
        &mut self,
        text: &str,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let update = self.compile_hot_reload_update(text)?;
        self.stage_hot_reload_source_update_result(update)
    }

    pub fn has_pending_hot_update(&self) -> EngineResult<bool> {
        let Some(hot_reload) = self.hot_reload.as_ref() else {
            return Err(EngineError::new(
                EngineErrorKind::RuntimeNotHotReloadEnabled,
            ));
        };
        Ok(hot_reload.has_pending_update())
    }

    pub fn check_reload(&mut self) -> EngineResult<Option<HotReloadReport>> {
        let Some(hot_reload) = self.hot_reload.as_mut() else {
            return Err(EngineError::new(
                EngineErrorKind::RuntimeNotHotReloadEnabled,
            ));
        };
        let update = hot_reload.take_pending_update();
        let report = update
            .map(|update| self.apply_hot_update_result_report(update))
            .transpose();
        self.state.reclaim_dead_generations();
        report
    }

    pub fn check_reload_at_tick_boundary(&mut self) -> EngineResult<Option<HotReloadReport>> {
        self.check_reload()
    }

    pub fn apply_hot_update_result_report(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> EngineResult<HotReloadReport> {
        let Some(current) = self.hot_reload.as_ref().map(HotReloadRuntime::current) else {
            return Err(EngineError::new(
                EngineErrorKind::RuntimeNotHotReloadEnabled,
            ));
        };
        let update = match update {
            Ok(update) => update,
            Err(error) => {
                return Ok(HotReloadReport::rejected(current.id, error));
            }
        };
        let staging =
            match self.prepare_hot_update_state(&update, RuntimeInitializationLimits::default()) {
                Ok(staging) => staging,
                Err(error) => return Ok(HotReloadReport::rejected(current.id, error)),
            };
        let next_states = update.linked_artifact().image().states().to_vec();
        let report = self
            .hot_reload
            .as_mut()
            .expect("hot reload runtime was checked above")
            .apply_hot_update_report(update);
        self.commit_hot_update_state(&next_states, staging);
        self.rebind_image_from_reload_report(Some(&report));
        Ok(report)
    }

    pub fn compile_hot_reload_update(
        &self,
        text: &str,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        self.compile_hot_reload_update_with_id(SourceId::new(1), text)
    }

    pub(crate) fn compile_hot_reload_update_with_id(
        &self,
        source: SourceId,
        text: &str,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self
            .image
            .engine()
            .compile_hot_reload_update_with_id(&previous, source, text))
    }

    pub fn compile_hot_reload_update_file(
        &self,
        path: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self
            .image
            .engine()
            .compile_hot_reload_update_file(&previous, path))
    }

    pub fn compile_hot_reload_update_dir(
        &self,
        root: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self
            .image
            .engine()
            .compile_hot_reload_update_dir(&previous, root))
    }

    pub fn compile_hot_reload_update_changed_file(
        &self,
        root: impl AsRef<Path>,
        changed_file: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self.image.engine().compile_hot_reload_update_changed_file(
            &previous,
            root,
            changed_file,
        ))
    }

    pub fn stage_hot_reload_update_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let previous = self.current_hot_reload_version()?;
        let update = self
            .image
            .engine()
            .compile_hot_reload_update_file(&previous, path);
        self.stage_hot_reload_source_update_result(update)
    }

    pub fn stage_hot_reload_update_dir(
        &mut self,
        root: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let previous = self.current_hot_reload_version()?;
        let update = self
            .image
            .engine()
            .compile_hot_reload_update_dir(&previous, root);
        self.stage_hot_reload_source_update_result(update)
    }

    pub fn stage_hot_reload_update_changed_file(
        &mut self,
        root: impl AsRef<Path>,
        changed_file: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let previous = self.current_hot_reload_version()?;
        let update = self.image.engine().compile_hot_reload_update_changed_file(
            &previous,
            root,
            changed_file,
        );
        self.stage_hot_reload_source_update_result(update)
    }

    fn stage_hot_reload_source_update_result(
        &mut self,
        update: EngineHotReloadSourceResult<HotUpdate>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        match update {
            Ok(update) => {
                self.stage_hot_update(update)?;
                Ok(Ok(()))
            }
            Err(error) => match error.kind {
                EngineHotReloadSourceErrorKind::Source(error) => {
                    Ok(Err(EngineHotReloadSourceError {
                        kind: EngineHotReloadSourceErrorKind::Source(error),
                    }))
                }
                EngineHotReloadSourceErrorKind::Link(error) => {
                    Ok(Err(EngineHotReloadSourceError {
                        kind: EngineHotReloadSourceErrorKind::Link(error),
                    }))
                }
                EngineHotReloadSourceErrorKind::HotReload(error) => {
                    self.stage_hot_update_result(Err(error))?;
                    Ok(Ok(()))
                }
            },
        }
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

    pub(crate) fn call_service_stable_function<'host>(
        &mut self,
        function: vela_def::FunctionId,
        diagnostic_name: impl Into<String>,
        args: CallArgs<'host>,
        options: CallOptions,
        dispatcher: std::sync::Arc<dyn crate::service::ServiceCallDispatcher>,
    ) -> VmResult<VelaValue> {
        self.call_impl_with_service_dispatcher(
            handles::StableVelaFunction {
                function,
                diagnostic_name: diagnostic_name.into(),
            },
            args,
            options,
            false,
            Some(dispatcher),
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
            service_dispatcher,
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
        RuntimeCallFuture::new(async move { self.call_impl_async(entry, args, options).await })
    }

    async fn call_impl_async<'call, 'args, T>(
        &'call mut self,
        entry: T,
        args: CallArgs<'args>,
        options: CallOptions,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget + Send + 'call,
        'args: 'call,
    {
        let mut budget = options.budget();
        let target = handles::call_target_sealed::Sealed::into_call_target(entry);
        let target = self.resolve_call_target(target, &mut budget)?;
        let state = &mut self.state;
        Self::call_runtime_args_async(RuntimeCallExecution {
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
            service_dispatcher: None,
        })
        .await
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
                .into_vm_for_program_image_with_abi(self.image.program_image(), current.abi())
        } else {
            self.image
                .engine()
                .into_vm_for_program_image(self.image.program_image())
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
                .into_vm_for_program_image_with_abi(self.image.program_image(), current.abi())
        } else {
            self.image
                .engine()
                .into_vm_for_program_image(self.image.program_image())
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
        let vm = runtime_vm(call.engine, call.registry_image, call.hot_reload);
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
        let vm = runtime_vm(call.engine, call.registry_image, call.hot_reload);
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

    fn check_vela_value_runtime(&self, value: &VelaValue) -> VmResult<()> {
        if value.runtime_id == self.state.id {
            return Ok(());
        }
        Err(call_args_type_error("VelaValue belongs to another Runtime"))
    }

    fn current_program_version_id(&self) -> Option<ProgramVersionId> {
        self.image.current_program_version_id()
    }

    pub(crate) fn active_binding_schema(&self) -> &vela_bytecode::RustBindingSchema {
        self.image.linked_artifact().binding_schema()
    }

    fn value_type_id(&self, value: &VelaValue) -> Option<vela_def::TypeId> {
        value_type_id(
            &value.value,
            &self.state.vm_states.heap,
            self.image.engine().registry().as_ref(),
            |handle| self.state.host_slots.resolve(handle),
        )
    }

    fn current_hot_reload_version(&self) -> EngineResult<std::sync::Arc<ProgramVersion>> {
        self.hot_reload_version()
            .ok_or_else(|| EngineError::new(EngineErrorKind::RuntimeNotHotReloadEnabled))
    }

    fn rebind_image_from_reload_report(&mut self, report: Option<&HotReloadReport>) {
        let Some(version) = report.and_then(HotReloadReport::version) else {
            return;
        };
        self.image = I::from_runtime_image(RuntimeImage::from_program_version(
            self.image.engine().clone(),
            &version,
        ));
        self.state.rebind_to_image(&self.image);
    }
}

fn runtime_vm(
    engine: &Engine,
    image: &ProgramImage,
    hot_reload: Option<&HotReloadRuntime>,
) -> vela_vm::Vm {
    if let Some(hot_reload) = hot_reload {
        let current = hot_reload.current();
        engine.into_vm_for_program_image_with_abi(image, current.abi())
    } else {
        engine.into_vm_for_program_image(image)
    }
}

fn value_type_id(
    value: &Value,
    heap: &ScriptHeap,
    registry: &vela_reflect::registry::TypeRegistry,
    resolve_host: impl FnOnce(vela_host::path::HostSlotRef) -> Option<vela_host::path::HostRef>,
) -> Option<vela_def::TypeId> {
    match value {
        Value::HeapRef(reference) => match heap.get(*reference)? {
            HeapValue::Record {
                identity: Some(identity),
                ..
            } => Some(identity.type_id),
            HeapValue::Enum {
                identity: Some(identity),
                ..
            } => Some(identity.type_id),
            _ => None,
        },
        Value::HostRef(reference) => resolve_host(*reference)
            .and_then(|reference| registry.type_of_host(reference))
            .map(|desc| desc.key.id),
        _ => None,
    }
}

fn unknown_function(name: String) -> VmError {
    VmError::new(VmErrorKind::UnknownFunction { name })
}

fn unknown_method(method: String) -> VmError {
    VmError::new(VmErrorKind::UnknownMethod { method })
}
