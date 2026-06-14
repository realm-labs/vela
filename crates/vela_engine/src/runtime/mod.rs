use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use vela_bytecode::{ProgramImage, UnlinkedProgram};
use vela_common::SourceId;
use vela_hir::module_graph::DeclarationKind;
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::HostErrorKind;
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_hot_reload::error::HotReloadResult;
use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_hot_reload::version::{HotUpdate, ProgramVersion};
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap::{HeapValue, ScriptHeap};
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;
use vela_vm::{
    LinkedProgramHostBudgetCall, LinkedProgramHostCall, LinkedRuntimeCodeCall,
    PersistentHeapExecution, persistent_value_to_owned,
};

use crate::engine::Engine;
use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::reload::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};

mod bytecode_profile;
#[cfg(test)]
#[path = "bytecode_profile_tests.rs"]
mod bytecode_profile_tests;
mod call_args;
mod global_store;
mod handles;
mod image;
mod inline_cache;
mod script_globals;
mod state;

pub use call_args::CallArgs;
pub use global_store::RuntimeGlobalStore;
pub use handles::{RuntimeCallTarget, RuntimeMethodTarget, VelaFunction, VelaMethod};
pub use image::{OwnedImage, RuntimeImage, RuntimeImageStorage, SharedImage};
pub use script_globals::{IntoGlobalValue, RuntimeScriptGlobalStore, VelaValue};

use call_args::{CallArgsAdapter, EmptyStateAdapter, call_args_type_error};
use global_store::GlobalStoreAdapter;
use handles::{RuntimeCallExecution, RuntimeMethodResolveContext};
use state::RuntimeState;

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
    pub fn new(engine: Engine, program: UnlinkedProgram) -> Self {
        let image = OwnedImage::from_image(RuntimeImage::new(engine, program));
        let state = RuntimeState::for_image(&image);
        Self {
            image,
            hot_reload: None,
            state,
        }
    }

    pub fn try_new(
        engine: Engine,
        program: UnlinkedProgram,
    ) -> Result<Self, vela_bytecode::linker::LinkError> {
        let image = OwnedImage::from_image(RuntimeImage::try_new(engine, program)?);
        let state = RuntimeState::for_image(&image);
        Ok(Self {
            image,
            hot_reload: None,
            state,
        })
    }

    #[must_use]
    pub fn from_hot_reload_version(engine: Engine, version: ProgramVersion) -> Self {
        let image = OwnedImage::from_image(RuntimeImage::from_program_version(engine, &version));
        let state = RuntimeState::for_image(&image);
        Self {
            image,
            hot_reload: Some(HotReloadRuntime::new(version)),
            state,
        }
    }
}

impl RuntimeImpl<SharedImage> {
    #[must_use]
    pub fn from_shared_image(image: SharedImage) -> Self {
        let state = RuntimeState::for_image(&image);
        Self {
            image,
            hot_reload: None,
            state,
        }
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

    pub fn insert_host_global<T>(&mut self, name: impl Into<String>, value: T) -> HostRef
    where
        T: ScriptHostObject + Send + 'static,
    {
        self.state.globals.insert_host(name, value)
    }

    #[must_use]
    pub fn host_global_ref(&self, name: &str) -> Option<HostRef> {
        self.state.globals.host_ref(name)
    }

    pub fn insert_global(
        &mut self,
        name: impl Into<String>,
        value: impl IntoGlobalValue,
    ) -> VmResult<()> {
        value.insert_global(self, name.into())
    }

    pub fn set_global(
        &mut self,
        name: impl Into<String>,
        value: impl IntoGlobalValue,
    ) -> VmResult<()> {
        self.insert_global(name, value)
    }

    pub fn global(&mut self, name: &str) -> VmResult<Option<OwnedValue>> {
        self.state.script_globals.value(name)
    }

    pub fn update_global(
        &mut self,
        name: &str,
        update: impl FnOnce(&mut OwnedValue),
    ) -> VmResult<()> {
        let mut value = self.state.script_globals.value(name)?.ok_or_else(|| {
            VmError::new(VmErrorKind::Host(HostErrorKind::MissingGlobal {
                name: name.to_owned(),
            }))
        })?;
        update(&mut value);
        self.insert_owned_global(name.to_owned(), value)
    }

    #[must_use]
    pub fn hot_reload_version(&self) -> Option<std::sync::Arc<ProgramVersion>> {
        self.hot_reload.as_ref().map(HotReloadRuntime::current)
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

    pub fn stage_hot_reload_update(&mut self, text: &str) -> EngineResult<()> {
        let update = self.compile_hot_reload_update(text)?;
        self.stage_hot_update_result(update)
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
        let report = hot_reload.check_reload();
        self.rebind_image_from_reload_report(report.as_ref());
        Ok(report)
    }

    pub fn check_reload_at_tick_boundary(&mut self) -> EngineResult<Option<HotReloadReport>> {
        self.check_reload()
    }

    pub fn apply_hot_update_result_report(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> EngineResult<HotReloadReport> {
        let Some(hot_reload) = self.hot_reload.as_mut() else {
            return Err(EngineError::new(
                EngineErrorKind::RuntimeNotHotReloadEnabled,
            ));
        };
        let report = hot_reload.apply_hot_update_result_report(update);
        self.rebind_image_from_reload_report(Some(&report));
        Ok(report)
    }

    pub fn compile_hot_reload_update(
        &self,
        text: &str,
    ) -> EngineResult<HotReloadResult<HotUpdate>> {
        self.compile_hot_reload_update_with_id(SourceId::new(1), text)
    }

    pub(crate) fn compile_hot_reload_update_with_id(
        &self,
        source: SourceId,
        text: &str,
    ) -> EngineResult<HotReloadResult<HotUpdate>> {
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
            .value_type_name(receiver)
            .ok_or_else(|| unknown_method(method.clone()))?;
        let method_id = self
            .image
            .program_image()
            .script_methods()
            .get(&receiver_type, &method)
            .map(|method| method.id)
            .ok_or_else(|| unknown_method(method.clone()))?;
        let code = self
            .image
            .program_image()
            .script_methods()
            .get_by_id(&receiver_type, method_id)
            .and_then(|method| {
                let linked_program = self.image.linked_program();
                let function = linked_program.entry_point_by_name(&method.function)?;
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

    pub fn call<T>(
        &mut self,
        entry: T,
        args: CallArgs<'_>,
        options: CallOptions,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        let mut adapter = EmptyStateAdapter;
        self.call_with_adapter(entry, args, options, &mut adapter)
    }

    pub fn call_with_adapter<T>(
        &mut self,
        entry: T,
        mut args: CallArgs<'_>,
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        let version_id = self.current_program_version_id();
        let state = &mut self.state;
        let linked_program = self.image.linked_program();
        let target = entry.resolve(state.id, linked_program, version_id)?;
        let mut access = HostAccess::new();
        Self::call_runtime_args(RuntimeCallExecution {
            runtime_id: state.id,
            engine: self.image.engine(),
            registry_image: self.image.program_image(),
            program: linked_program,
            hot_reload: self.hot_reload.as_ref(),
            globals: &mut state.globals,
            script_globals: &mut state.script_globals,
            inline_caches: &state.inline_caches,
            bytecode_profile: &state.bytecode_profile,
            target,
            args: &mut args,
            options,
            adapter,
            access: &mut access,
        })
    }

    pub fn call_method<T>(
        &mut self,
        receiver: &VelaValue,
        method: T,
        mut args: CallArgs<'_>,
        options: CallOptions,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeMethodTarget,
    {
        self.check_vela_value_runtime(receiver)?;
        let version_id = self.current_program_version_id();
        let state = &mut self.state;
        let linked_program = self.image.linked_program();
        let target = method.resolve(RuntimeMethodResolveContext {
            runtime_id: state.id,
            program_image: self.image.program_image(),
            linked_program,
            receiver,
            version_id,
            script_globals: &state.script_globals,
            engine: self.image.engine(),
        })?;
        let mut budget = options.budget();
        let resolved = args.resolve_values(
            &target.name,
            &target.params,
            &target.param_defaults,
            state.id,
            &mut state.script_globals.heap,
            &mut budget,
        )?;
        let mut adapter = EmptyStateAdapter;
        let mut access = HostAccess::new();
        let mut adapter = CallArgsAdapter::new(&mut args, &mut adapter);
        let mut adapter = GlobalStoreAdapter::new(&mut state.globals, &mut adapter);
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut access,
            script_globals: Some(&state.script_globals.values),
        };
        let vm = runtime_vm(
            self.image.engine(),
            self.image.program_image(),
            self.hot_reload.as_ref(),
        );
        let roots = state.script_globals.roots();
        let mut method_args = Vec::with_capacity(resolved.len().saturating_add(1));
        method_args.push(receiver.value);
        method_args.extend_from_slice(&resolved);
        let result = vm.run_linked_runtime_code_call(LinkedRuntimeCodeCall {
            program: linked_program,
            code: target.code,
            args: &method_args,
            host: &mut host,
            persistent: PersistentHeapExecution {
                heap: &mut state.script_globals.heap,
                roots: &roots,
            },
            budget: &mut budget,
            inline_caches: Some(&state.inline_caches),
            bytecode_profiler: Some(&state.bytecode_profile),
        })?;
        Ok(state.script_globals.retain(state.id, result))
    }

    pub fn value_to_owned(&mut self, value: &VelaValue) -> VmResult<OwnedValue> {
        self.check_vela_value_runtime(value)?;
        persistent_value_to_owned(&value.value, &mut self.state.script_globals.heap)
    }

    #[cfg(feature = "serde")]
    pub fn from_value<T>(&self, value: &VelaValue) -> VmResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.check_vela_value_runtime(value)?;
        vela_vm::serde::from_runtime_value(&value.value, &self.state.script_globals.heap)
    }

    #[cfg(feature = "serde")]
    pub fn global_as<T>(&self, name: &str) -> VmResult<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        self.state.script_globals.value_as(name)
    }

    pub fn call_raw(
        &mut self,
        entry: &str,
        args: &[OwnedValue],
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
        access: &mut HostAccess,
    ) -> VmResult<OwnedValue> {
        let mut budget = options.budget();
        let mut adapter = GlobalStoreAdapter::new(&mut self.state.globals, adapter);
        let mut host = HostExecution {
            adapter: &mut adapter,
            access,
            script_globals: Some(&self.state.script_globals.values),
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
        let linked_program = self.image.linked_program();
        if options.managed_heap || !self.state.script_globals.is_empty() {
            let roots = self.state.script_globals.roots();
            vm.run_linked_program_host_call(LinkedProgramHostCall {
                program: linked_program,
                entry,
                args,
                host: &mut host,
                persistent: PersistentHeapExecution {
                    heap: &mut self.state.script_globals.heap,
                    roots: &roots,
                },
                budget: &mut budget,
                inline_caches: Some(&self.state.inline_caches),
                bytecode_profiler: Some(&self.state.bytecode_profile),
            })
        } else {
            vm.run_linked_program_host_budget_call(LinkedProgramHostBudgetCall {
                program: linked_program,
                entry,
                args,
                host: &mut host,
                budget: &mut budget,
                inline_caches: Some(&self.state.inline_caches),
                bytecode_profiler: Some(&self.state.bytecode_profile),
            })
        }
    }

    pub fn call_args_raw(
        &mut self,
        entry: &str,
        args: &mut CallArgs<'_>,
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
        access: &mut HostAccess,
    ) -> VmResult<OwnedValue> {
        let resolved = self.resolve_call_args(entry, args)?;
        let mut adapter = CallArgsAdapter::new(args, adapter);
        self.call_raw(entry, &resolved, options, &mut adapter, access)
    }

    fn call_runtime_args(call: RuntimeCallExecution<'_, '_, '_, '_, '_>) -> VmResult<VelaValue> {
        let mut budget = call.options.budget();
        let resolved = call.args.resolve_values(
            &call.target.name,
            &call.target.params,
            &call.target.param_defaults,
            call.runtime_id,
            &mut call.script_globals.heap,
            &mut budget,
        )?;
        let mut adapter = CallArgsAdapter::new(call.args, call.adapter);
        let mut adapter = GlobalStoreAdapter::new(call.globals, &mut adapter);
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: call.access,
            script_globals: Some(&call.script_globals.values),
        };
        let vm = runtime_vm(call.engine, call.registry_image, call.hot_reload);
        let roots = call.script_globals.roots();
        let result = vm.run_linked_runtime_code_call(LinkedRuntimeCodeCall {
            program: call.program,
            code: call.target.code,
            args: &resolved,
            host: &mut host,
            persistent: PersistentHeapExecution {
                heap: &mut call.script_globals.heap,
                roots: &roots,
            },
            budget: &mut budget,
            inline_caches: Some(call.inline_caches),
            bytecode_profiler: Some(call.bytecode_profile),
        })?;
        Ok(call.script_globals.retain(call.runtime_id, result))
    }

    pub fn call_raw_at_event_end_safe_point(
        &mut self,
        entry: &str,
        args: &[OwnedValue],
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
        access: &mut HostAccess,
    ) -> VmResult<EventCallSafePointReport> {
        let value = self.call_raw(entry, args, options, adapter, access)?;
        let reload = self.check_optional_reload();
        Ok(EventCallSafePointReport { value, reload })
    }

    pub fn call_args_raw_at_event_end_safe_point(
        &mut self,
        entry: &str,
        args: &mut CallArgs<'_>,
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
        access: &mut HostAccess,
    ) -> VmResult<EventCallSafePointReport> {
        let value = self.call_args_raw(entry, args, options, adapter, access)?;
        let reload = self.check_optional_reload();
        Ok(EventCallSafePointReport { value, reload })
    }

    fn resolve_call_args(&self, entry: &str, args: &CallArgs<'_>) -> VmResult<Vec<OwnedValue>> {
        let code = self
            .image
            .program_image()
            .function_by_name(entry)
            .ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownFunction {
                    name: entry.to_owned(),
                })
            })?;
        args.resolve(entry, &code.params, &code.param_defaults)
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

    fn value_type_name(&self, value: &VelaValue) -> Option<String> {
        value_type_name(
            &value.value,
            &self.state.script_globals.heap,
            self.image.engine().registry().as_ref(),
        )
    }

    fn current_hot_reload_version(&self) -> EngineResult<std::sync::Arc<ProgramVersion>> {
        self.hot_reload_version()
            .ok_or_else(|| EngineError::new(EngineErrorKind::RuntimeNotHotReloadEnabled))
    }

    fn check_optional_reload(&mut self) -> Option<HotReloadReport> {
        let hot_reload = self.hot_reload.as_mut()?;
        let report = hot_reload.check_reload();
        self.rebind_image_from_reload_report(report.as_ref());
        report
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

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    fn insert_owned_global(&mut self, name: String, value: OwnedValue) -> VmResult<()> {
        validate_global_contract(self.image.program_image(), &name, &value)?;
        self.state.script_globals.insert(name, value)
    }
}

fn validate_global_contract(image: &ProgramImage, name: &str, value: &OwnedValue) -> VmResult<()> {
    let Some(expected) = global_contract_type(image, name) else {
        return Ok(());
    };
    if expected == "Any" || owned_value_matches_contract(value, &expected) {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::TypeContractViolation {
        expected,
        actual: owned_value_contract_type_name(value),
        debug_name: name.to_owned(),
    }))
}

fn global_contract_type(image: &ProgramImage, name: &str) -> Option<String> {
    let graph = image.script_metadata()?;
    let leaf_name = name.rsplit("::").next().unwrap_or(name);
    let mut leaf_match = None;
    for module in graph.module_ids() {
        let module_path = graph.module_path(module)?.join();
        let declarations = graph.module(module)?;
        for declaration_name in declarations.names() {
            let declaration = declarations.get(declaration_name)?;
            let metadata = graph.declaration(declaration)?;
            if metadata.kind != DeclarationKind::Global {
                continue;
            }
            let symbol = if module_path.is_empty() {
                metadata.name.clone()
            } else {
                format!("{module_path}::{}", metadata.name)
            };
            if symbol == name {
                return graph
                    .global_metadata(declaration)
                    .map(|metadata| metadata.type_hint.display());
            }
            if metadata.name == leaf_name {
                let Some(metadata) = graph.global_metadata(declaration) else {
                    continue;
                };
                if leaf_match.is_some() {
                    return None;
                }
                leaf_match = Some(metadata.type_hint.display());
            }
        }
    }
    leaf_match
}

fn owned_value_matches_contract(value: &OwnedValue, expected: &str) -> bool {
    match value {
        OwnedValue::Null => expected == "null",
        OwnedValue::Bool(_) => expected == "bool",
        OwnedValue::Char(_) => expected == "char",
        OwnedValue::Scalar(value) => value.primitive_tag().name() == expected,
        OwnedValue::String(_) => expected == "String",
        OwnedValue::Bytes(_) => expected == "Bytes",
        OwnedValue::Array(_) => expected == "Array",
        OwnedValue::Map(_) => expected == "Map",
        OwnedValue::Set(_) => expected == "Set",
        OwnedValue::Record { type_name, .. } => type_name == expected,
        OwnedValue::Enum { enum_name, .. } => enum_name == expected,
        OwnedValue::Closure(_) => expected == "Closure",
        OwnedValue::Range(_) => expected == "Range",
        OwnedValue::Iterator(_) => expected == "Iterator",
        OwnedValue::Missing | OwnedValue::HostRef(_) | OwnedValue::PathProxy(_) => false,
    }
}

fn owned_value_contract_type_name(value: &OwnedValue) -> String {
    match value {
        OwnedValue::Missing => "missing".to_owned(),
        OwnedValue::Null => "null".to_owned(),
        OwnedValue::Bool(_) => "bool".to_owned(),
        OwnedValue::Char(_) => "char".to_owned(),
        OwnedValue::Scalar(value) => value.primitive_tag().name().to_owned(),
        OwnedValue::String(_) => "String".to_owned(),
        OwnedValue::Bytes(_) => "Bytes".to_owned(),
        OwnedValue::Array(_) => "Array".to_owned(),
        OwnedValue::Map(_) => "Map".to_owned(),
        OwnedValue::Set(_) => "Set".to_owned(),
        OwnedValue::Record { type_name, .. } => type_name.clone(),
        OwnedValue::Enum { enum_name, .. } => enum_name.clone(),
        OwnedValue::Closure(_) => "Closure".to_owned(),
        OwnedValue::Range(_) => "Range".to_owned(),
        OwnedValue::HostRef(_) => "host_ref".to_owned(),
        OwnedValue::PathProxy(_) => "path_proxy".to_owned(),
        OwnedValue::Iterator(_) => "Iterator".to_owned(),
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

fn value_type_name(
    value: &Value,
    heap: &ScriptHeap,
    registry: &vela_reflect::registry::TypeRegistry,
) -> Option<String> {
    match value {
        Value::HeapRef(reference) => match heap.get(*reference)? {
            HeapValue::Record { type_name, .. } => Some(type_name.clone()),
            HeapValue::Enum { enum_name, .. } => Some(enum_name.clone()),
            _ => None,
        },
        Value::HostRef(reference) => registry
            .type_of_host(*reference)
            .map(|desc| desc.key.name.clone()),
        _ => None,
    }
}

fn unknown_function(name: String) -> VmError {
    VmError::new(VmErrorKind::UnknownFunction { name })
}

fn unknown_method(method: String) -> VmError {
    VmError::new(VmErrorKind::UnknownMethod { method })
}

#[cfg(test)]
mod tests {
    use vela_bytecode::linked::{Instruction, InstructionKind};
    use vela_bytecode::script_methods::ScriptMethodTable;
    use vela_bytecode::{
        Constant, LinkedCodeObject, LinkedProgram, ProgramImage, Register, UnlinkedCodeObject,
        UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedProgram,
    };
    use vela_host::access::HostAccess;
    use vela_host::mock::MockStateAdapter;
    use vela_vm::owned_value::OwnedValue;

    use crate::engine::Engine;
    use crate::native::NativeFunctionId;

    use super::{CallOptions, OwnedImage, RuntimeImage, RuntimeImpl, RuntimeState};

    #[test]
    fn call_raw_executes_linked_program_image() {
        for options in [
            CallOptions::unbounded(),
            CallOptions::unbounded().with_managed_heap(false),
        ] {
            let mut runtime = linked_only_runtime();
            let mut adapter = MockStateAdapter::new();
            let mut access = HostAccess::new();

            let result = runtime.call_raw("main", &[], options, &mut adapter, &mut access);

            assert_eq!(
                result,
                Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
            );
        }
    }

    #[test]
    fn runtime_image_try_new_rejects_link_errors_before_execution() {
        let native_id = NativeFunctionId::new(91);
        let mut main = UnlinkedCodeObject::new("main", 1);
        main.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::CallNative {
                dst: Some(Register(0)),
                name: "test::answer".to_owned(),
                native: native_id,
                cache_site: None,
                args: Vec::new(),
            },
        ));
        main.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
            src: Register(0),
        }));
        let mut program = UnlinkedProgram::new();
        program.insert_function(main);

        let engine = Engine::builder().build().expect("engine should build");
        let result = RuntimeImage::try_new(engine, program);

        assert!(matches!(
            result,
            Err(vela_bytecode::linker::LinkError::UnresolvedNative { name, .. })
                if name == "test::answer"
        ));
    }

    fn linked_only_runtime() -> RuntimeImpl<OwnedImage> {
        let engine = Engine::builder().build().expect("engine should build");
        let program_image = ProgramImage::from_parts(
            std::iter::empty::<vela_bytecode::UnlinkedCodeObject>(),
            std::iter::empty::<String>(),
            ScriptMethodTable::new(),
            None,
        );
        let mut linked_program = LinkedProgram::new();
        let main_name = linked_program.intern_debug_name("main");
        let mut code = LinkedCodeObject::new(main_name, 1);
        let value = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(7)));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(0),
        }));
        let main = linked_program.push_function(code);
        linked_program.set_entry_point(main_name, main);

        let image = RuntimeImage::from_parts_for_test(engine, program_image, linked_program);
        let image = OwnedImage::from_image(image);
        let state = RuntimeState::for_image(&image);
        RuntimeImpl {
            image,
            hot_reload: None,
            state,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventCallSafePointReport {
    pub value: OwnedValue,
    pub reload: Option<HotReloadReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallOptions {
    pub instruction_budget: u64,
    pub memory_budget: usize,
    pub call_depth: usize,
    pub managed_heap: bool,
}

impl CallOptions {
    #[must_use]
    pub const fn new(instruction_budget: u64, memory_budget: usize, call_depth: usize) -> Self {
        Self {
            instruction_budget,
            memory_budget,
            call_depth,
            managed_heap: true,
        }
    }

    #[must_use]
    pub const fn unbounded() -> Self {
        Self::new(u64::MAX, usize::MAX, usize::MAX)
    }

    #[must_use]
    pub const fn with_managed_heap(mut self, managed_heap: bool) -> Self {
        self.managed_heap = managed_heap;
        self
    }

    fn budget(&self) -> ExecutionBudget {
        ExecutionBudget::new(self.instruction_budget, self.memory_budget, self.call_depth)
    }
}
