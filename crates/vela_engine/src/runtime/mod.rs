use std::sync::atomic::{AtomicU64, Ordering};

use vela_hot_reload::runtime::HotReloadRuntime;
pub use vela_hot_reload::runtime::HotReloadStagingHandle;
use vela_hot_reload::version::ProgramVersion;
use vela_vm::HostExecution;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap_execution::HeapExecution;
use vela_vm::owned_value::OwnedValue;
use vela_vm::{LinkedDriveOutcome, LinkedExecutionStart, persistent_value_to_owned};
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
mod control;
#[cfg(test)]
mod control_tests;
pub(crate) mod execution_data;
pub(crate) mod handles;
mod image;
mod initialization;
mod inline_cache;
mod metadata;
mod options;
mod profile_api;
mod reentry;
#[cfg(test)]
mod reentry_tests;
mod reload_api;
mod state;
mod state_api;
#[cfg(test)]
mod tests;
mod value_support;
mod vm_states;

pub use options::CallOptions;

pub use bytecode_profile::{
    BytecodeProfileSnapshot, BytecodeProfileSummary, FunctionBytecodeProfile,
    ScalarLoopBytecodeProfile, ScalarUnitBytecodeProfile, SuperinstructionBytecodeProfile,
};
pub use call_args::CallArgs;
pub use control::{CallControl, CallSnapshot, CallStatus};
pub use handles::{
    RuntimeCallTarget, RuntimeMethodSelector, VelaFunction, VelaMethod, VelaMethodTarget,
};
pub use image::{OwnedImage, RuntimeImage, RuntimeImageStorage, SharedImage};
pub use initialization::{RuntimeBuildError, RuntimeBuilder, RuntimeInitializationLimits};
pub use reload_api::{ReloadSource, RuntimeReloadError};
pub use vm_states::{IntoStateValue, RuntimeVmStateStore, VelaValue};

use call_args::call_args_type_error;
use handles::{
    RuntimeCallExecution, RuntimeCallTargetKind, RuntimeMethodResolveContext,
    RuntimeMethodSelectorKind,
};
use reentry::{ActiveNativeReentry, invoke_prepared_context};
use state::RuntimeState;
use value_support::{runtime_vm, unknown_function, unknown_method, value_type_id};
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

    pub fn call<T>(&mut self, entry: T, args: CallArgs, options: CallOptions) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        let policy = options.call_policy();
        if let Some(policy) = &policy {
            policy.attach();
            policy.check()?;
        }
        let result = (|| {
            let mut budget = options.budget();
            let target = handles::call_target_sealed::Sealed::into_call_target(entry);
            let target = self.resolve_call_target(target)?;
            let state = &mut self.state;
            Self::call_runtime_args(RuntimeCallExecution {
                runtime_id: state.id,
                engine: self.image.engine(),
                registry_image: self.image.program_image(),
                artifact: self.image.linked_artifact(),
                hot_reload: self.hot_reload.as_ref(),
                vm_states: &mut state.vm_states,
                generations: &mut state.generations,
                target,
                args,
                budget: &mut budget,
            })
        })();
        if let Some(policy) = &policy {
            policy.check()?;
            policy.finish(&result);
        }
        result
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

    /// Materializes a bounded owned tree. Cycles and exports exceeding 64 nested
    /// heap objects, 65,536 values, or 16 MiB of copied storage return a VM error.
    pub fn value_to_owned(&mut self, value: &VelaValue) -> VmResult<OwnedValue> {
        self.check_vela_value_runtime(value)?;
        persistent_value_to_owned(&value.value, &mut self.state.vm_states.heap)
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
    ) -> VmResult<OwnedValue> {
        let mut budget = options.budget();
        let roots = self.state.vm_states.roots();
        let use_persistent_heap = options.managed_heap || !self.state.vm_states.is_empty();
        let mut host = HostExecution {
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
    pub(crate) fn call_args_raw(
        &mut self,
        entry: &str,
        args: CallArgs,
        options: CallOptions,
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
        let resolved = args.resolve_values(
            entry,
            &params,
            &code.param_defaults,
            &mut call_args::CallArgRuntime::new(
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
        vela_vm::persistent_value_to_owned(&value, &mut self.state.vm_states.heap)
    }

    fn call_runtime_args(call: RuntimeCallExecution<'_, '_, '_>) -> VmResult<VelaValue> {
        let budget = call.budget;
        let resolved = call.args.resolve_values(
            &call.target.name,
            &call.target.params,
            &call.target.param_defaults,
            &mut call_args::CallArgRuntime::new(
                call.runtime_id,
                call.artifact.program(),
                &mut call.vm_states.heap,
                budget,
            ),
        )?;

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
                LinkedDriveOutcome::ContextBoundary(prepared) => {
                    let result = {
                        let mut active = ActiveNativeReentry {
                            runtime_id: call.runtime_id,
                            engine: call.engine,
                            registry_image: call.registry_image,
                            artifact: call.artifact,
                            vm: &vm,
                            session: &mut session,

                            heap: &mut heap,
                            budget,
                            vm_state_values,
                            retained_values: std::sync::Arc::clone(&retained_values),
                            generations: &mut *call.generations,
                        };
                        invoke_prepared_context(&prepared, &mut active)
                    };
                    let mut host = HostExecution {
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
                },
            ),
        }
    }
}
