use std::collections::BTreeMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use vela_bytecode::Program;
use vela_common::{GlobalSlot, HostObjectId, SourceId};
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::object::ScriptHostObject;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_hot_reload::error::HotReloadResult;
use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_hot_reload::version::{HotUpdate, ProgramVersion};
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap::ScriptHeap;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;
use vela_vm::{
    PersistentHeapExecution, ScriptGlobalValues, owned_to_persistent_value,
    persistent_value_to_owned,
};

use crate::engine::Engine;
use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::reload::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};

mod call_args;

pub use call_args::CallArgs;

use call_args::{CallArgsAdapter, EmptyStateAdapter, call_args_type_error};

pub struct Runtime {
    id: u64,
    engine: Engine,
    program: Program,
    hot_reload: Option<HotReloadRuntime>,
    globals: RuntimeGlobalStore,
    script_globals: RuntimeScriptGlobalStore,
}

static NEXT_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

fn next_runtime_id() -> u64 {
    NEXT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed)
}

impl Runtime {
    #[must_use]
    pub fn new(engine: Engine, program: Program) -> Self {
        let global_names = program.global_names().to_vec();
        Self {
            id: next_runtime_id(),
            engine,
            program,
            hot_reload: None,
            globals: RuntimeGlobalStore::with_global_layout(&global_names),
            script_globals: RuntimeScriptGlobalStore::with_global_layout(&global_names),
        }
    }

    #[must_use]
    pub fn from_hot_reload_version(engine: Engine, version: ProgramVersion) -> Self {
        let program = version.to_program();
        let global_names = program.global_names().to_vec();
        Self {
            id: next_runtime_id(),
            engine,
            program,
            hot_reload: Some(HotReloadRuntime::new(version)),
            globals: RuntimeGlobalStore::with_global_layout(&global_names),
            script_globals: RuntimeScriptGlobalStore::with_global_layout(&global_names),
        }
    }

    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    fn set_global_layout_from_program(&mut self) {
        let names = self.program.global_names();
        self.globals.set_global_layout(names);
        self.script_globals.set_global_layout(names);
    }

    pub fn insert_host_global<T>(&mut self, name: impl Into<String>, value: T) -> HostRef
    where
        T: ScriptHostObject + Send + 'static,
    {
        self.globals.insert_host(name, value)
    }

    #[must_use]
    pub fn host_global_ref(&self, name: &str) -> Option<HostRef> {
        self.globals.host_ref(name)
    }

    pub fn insert_global(
        &mut self,
        name: impl Into<String>,
        value: impl Into<OwnedValue>,
    ) -> VmResult<()> {
        self.script_globals.insert(name, value.into())
    }

    pub fn set_global(
        &mut self,
        name: impl Into<String>,
        value: impl Into<OwnedValue>,
    ) -> VmResult<()> {
        self.insert_global(name, value)
    }

    pub fn global(&mut self, name: &str) -> VmResult<Option<OwnedValue>> {
        self.script_globals.value(name)
    }

    pub fn update_global(
        &mut self,
        name: &str,
        update: impl FnOnce(&mut OwnedValue),
    ) -> VmResult<()> {
        self.script_globals.update(name, update)
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

    pub fn stage_hot_reload_update(&mut self, source: SourceId, text: &str) -> EngineResult<()> {
        let update = self.compile_hot_reload_update(source, text)?;
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
        let report = Self::consume_reload_report(&mut self.program, hot_reload);
        if report
            .as_ref()
            .and_then(|report| report.version())
            .is_some()
        {
            self.set_global_layout_from_program();
        }
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
        if let Some(version) = report.version() {
            self.program = version.to_program();
            self.set_global_layout_from_program();
        }
        Ok(report)
    }

    pub fn compile_hot_reload_update(
        &self,
        source: SourceId,
        text: &str,
    ) -> EngineResult<HotReloadResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self
            .engine
            .compile_hot_reload_update(&previous, source, text))
    }

    pub fn compile_hot_reload_update_file(
        &self,
        path: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self.engine.compile_hot_reload_update_file(&previous, path))
    }

    pub fn compile_hot_reload_update_dir(
        &self,
        root: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self.engine.compile_hot_reload_update_dir(&previous, root))
    }

    pub fn compile_hot_reload_update_changed_file(
        &self,
        root: impl AsRef<Path>,
        changed_file: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<HotUpdate>> {
        let previous = self.current_hot_reload_version()?;
        Ok(self
            .engine
            .compile_hot_reload_update_changed_file(&previous, root, changed_file))
    }

    pub fn stage_hot_reload_update_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let previous = self.current_hot_reload_version()?;
        let update = self.engine.compile_hot_reload_update_file(&previous, path);
        self.stage_hot_reload_source_update_result(update)
    }

    pub fn stage_hot_reload_update_dir(
        &mut self,
        root: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let previous = self.current_hot_reload_version()?;
        let update = self.engine.compile_hot_reload_update_dir(&previous, root);
        self.stage_hot_reload_source_update_result(update)
    }

    pub fn stage_hot_reload_update_changed_file(
        &mut self,
        root: impl AsRef<Path>,
        changed_file: impl AsRef<Path>,
    ) -> EngineResult<EngineHotReloadSourceResult<()>> {
        let previous = self.current_hot_reload_version()?;
        let update =
            self.engine
                .compile_hot_reload_update_changed_file(&previous, root, changed_file);
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

    pub fn call(
        &mut self,
        entry: &str,
        args: CallArgs<'_>,
        options: CallOptions,
    ) -> VmResult<CallOutput> {
        let mut adapter = EmptyStateAdapter;
        self.call_with_adapter(entry, args, options, &mut adapter)
    }

    pub fn call_value(
        &mut self,
        entry: &str,
        args: CallArgs<'_>,
        options: CallOptions,
    ) -> VmResult<VelaValue> {
        let mut adapter = EmptyStateAdapter;
        self.call_value_with_adapter(entry, args, options, &mut adapter)
    }

    pub fn call_with_adapter(
        &mut self,
        entry: &str,
        mut args: CallArgs<'_>,
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
    ) -> VmResult<CallOutput> {
        let mut access = HostAccess::new();
        let value = self.call_args_raw(entry, &mut args, options, adapter, &mut access)?;
        Ok(CallOutput { value })
    }

    pub fn call_value_with_adapter(
        &mut self,
        entry: &str,
        mut args: CallArgs<'_>,
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
    ) -> VmResult<VelaValue> {
        let mut access = HostAccess::new();
        self.call_value_args_raw(entry, &mut args, options, adapter, &mut access)
    }

    pub fn value_to_owned(&mut self, value: &VelaValue) -> VmResult<OwnedValue> {
        self.check_vela_value_runtime(value)?;
        persistent_value_to_owned(&value.value, &mut self.script_globals.heap)
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
        let mut adapter = GlobalStoreAdapter {
            globals: &mut self.globals,
            fallback: adapter,
        };
        let mut host = HostExecution {
            adapter: &mut adapter,
            access,
            script_globals: Some(&self.script_globals.values),
        };
        let vm = if let Some(hot_reload) = &self.hot_reload {
            let current = hot_reload.current();
            self.engine
                .into_vm_for_program_with_abi(&self.program, current.abi())
        } else {
            self.engine.into_vm_for_program(&self.program)
        };
        if options.managed_heap || !self.script_globals.is_empty() {
            let roots = self.script_globals.roots();
            vm.run_program_with_host_persistent_heap_and_budget(
                &self.program,
                entry,
                args,
                &mut host,
                PersistentHeapExecution {
                    heap: &mut self.script_globals.heap,
                    roots: &roots,
                },
                &mut budget,
            )
        } else {
            vm.run_program_with_host_and_budget(&self.program, entry, args, &mut host, &mut budget)
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

    pub fn call_value_args_raw(
        &mut self,
        entry: &str,
        args: &mut CallArgs<'_>,
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
        access: &mut HostAccess,
    ) -> VmResult<VelaValue> {
        let mut budget = options.budget();
        let resolved = self.resolve_call_value_args(entry, args, &mut budget)?;
        let mut adapter = CallArgsAdapter::new(args, adapter);
        let mut adapter = GlobalStoreAdapter {
            globals: &mut self.globals,
            fallback: &mut adapter,
        };
        let mut host = HostExecution {
            adapter: &mut adapter,
            access,
            script_globals: Some(&self.script_globals.values),
        };
        let vm = if let Some(hot_reload) = &self.hot_reload {
            let current = hot_reload.current();
            self.engine
                .into_vm_for_program_with_abi(&self.program, current.abi())
        } else {
            self.engine.into_vm_for_program(&self.program)
        };
        let roots = self.script_globals.roots();
        let result = vm.run_program_runtime_with_host_persistent_heap_and_budget(
            &self.program,
            entry,
            &resolved,
            &mut host,
            PersistentHeapExecution {
                heap: &mut self.script_globals.heap,
                roots: &roots,
            },
            &mut budget,
        )?;
        Ok(self.script_globals.retain(self.id, result))
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
        let code = self.program.function(entry).ok_or_else(|| VmError {
            kind: VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            },
            source_span: None,
            call_stack: Default::default(),
        })?;
        args.resolve(entry, &code.params, &code.param_defaults)
    }

    fn resolve_call_value_args(
        &mut self,
        entry: &str,
        args: &CallArgs<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Vec<Value>> {
        let (params, param_defaults) = {
            let code = self.program.function(entry).ok_or_else(|| VmError {
                kind: VmErrorKind::UnknownFunction {
                    name: entry.to_owned(),
                },
                source_span: None,
                call_stack: Default::default(),
            })?;
            (code.params.clone(), code.param_defaults.clone())
        };
        args.resolve_values(
            entry,
            &params,
            &param_defaults,
            self.id,
            &mut self.script_globals.heap,
            budget,
        )
    }

    fn check_vela_value_runtime(&self, value: &VelaValue) -> VmResult<()> {
        if value.runtime_id == self.id {
            return Ok(());
        }
        Err(call_args_type_error("VelaValue belongs to another Runtime"))
    }

    fn current_hot_reload_version(&self) -> EngineResult<std::sync::Arc<ProgramVersion>> {
        self.hot_reload
            .as_ref()
            .map(HotReloadRuntime::current)
            .ok_or_else(|| EngineError::new(EngineErrorKind::RuntimeNotHotReloadEnabled))
    }

    fn check_optional_reload(&mut self) -> Option<HotReloadReport> {
        let hot_reload = self.hot_reload.as_mut()?;
        let report = Self::consume_reload_report(&mut self.program, hot_reload);
        if report
            .as_ref()
            .and_then(|report| report.version())
            .is_some()
        {
            self.set_global_layout_from_program();
        }
        report
    }

    fn consume_reload_report(
        program: &mut Program,
        hot_reload: &mut HotReloadRuntime,
    ) -> Option<HotReloadReport> {
        let report = hot_reload.check_reload()?;
        if let Some(version) = report.version() {
            *program = version.to_program();
        }
        Some(report)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallOutput {
    value: OwnedValue,
}

impl CallOutput {
    #[must_use]
    pub const fn new(value: OwnedValue) -> Self {
        Self { value }
    }

    #[must_use]
    pub const fn value(&self) -> &OwnedValue {
        &self.value
    }

    #[must_use]
    pub fn into_value(self) -> OwnedValue {
        self.value
    }
}

impl Deref for CallOutput {
    type Target = OwnedValue;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl AsRef<OwnedValue> for CallOutput {
    fn as_ref(&self) -> &OwnedValue {
        &self.value
    }
}

pub struct VelaValue {
    runtime_id: u64,
    value: Value,
    root_id: u64,
    roots: Arc<Mutex<RuntimeValueRoots>>,
}

impl VelaValue {
    const fn runtime_id(&self) -> u64 {
        self.runtime_id
    }

    const fn value(&self) -> Value {
        self.value
    }
}

impl Clone for VelaValue {
    fn clone(&self) -> Self {
        self.roots
            .lock()
            .expect("runtime value roots mutex poisoned")
            .clone_root(self.root_id);
        Self {
            runtime_id: self.runtime_id,
            value: self.value,
            root_id: self.root_id,
            roots: Arc::clone(&self.roots),
        }
    }
}

impl Drop for VelaValue {
    fn drop(&mut self) {
        self.roots
            .lock()
            .expect("runtime value roots mutex poisoned")
            .release(self.root_id);
    }
}

impl std::fmt::Debug for VelaValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VelaValue")
            .field("value", &self.value)
            .finish()
    }
}

impl PartialEq for VelaValue {
    fn eq(&self, other: &Self) -> bool {
        self.runtime_id == other.runtime_id && self.value == other.value
    }
}

#[derive(Debug, Default)]
struct RuntimeValueRoots {
    next_id: u64,
    values: BTreeMap<u64, RuntimeValueRoot>,
}

#[derive(Debug)]
struct RuntimeValueRoot {
    value: Value,
    refs: usize,
}

impl RuntimeValueRoots {
    fn retain(roots: &Arc<Mutex<Self>>, runtime_id: u64, value: Value) -> VelaValue {
        let mut roots_mut = roots.lock().expect("runtime value roots mutex poisoned");
        let root_id = roots_mut.next_id;
        roots_mut.next_id = roots_mut.next_id.saturating_add(1);
        roots_mut
            .values
            .insert(root_id, RuntimeValueRoot { value, refs: 1 });
        drop(roots_mut);
        VelaValue {
            runtime_id,
            value,
            root_id,
            roots: Arc::clone(roots),
        }
    }

    fn clone_root(&mut self, root_id: u64) {
        if let Some(root) = self.values.get_mut(&root_id) {
            root.refs = root.refs.saturating_add(1);
        }
    }

    fn release(&mut self, root_id: u64) {
        let Some(root) = self.values.get_mut(&root_id) else {
            return;
        };
        root.refs = root.refs.saturating_sub(1);
        if root.refs == 0 {
            self.values.remove(&root_id);
        }
    }

    fn values(&self) -> impl Iterator<Item = Value> + '_ {
        self.values.values().map(|root| root.value)
    }
}

const GLOBAL_HOST_OBJECT_ID_BASE: u64 = 1 << 62;

pub struct RuntimeGlobalStore {
    globals: BTreeMap<String, HostGlobalBinding>,
    slots: Vec<Option<HostRef>>,
    slot_by_name: BTreeMap<String, GlobalSlot>,
    next_host_object_id: u64,
}

impl Default for RuntimeGlobalStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeGlobalStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            globals: BTreeMap::new(),
            slots: Vec::new(),
            slot_by_name: BTreeMap::new(),
            next_host_object_id: GLOBAL_HOST_OBJECT_ID_BASE,
        }
    }

    #[must_use]
    pub fn with_global_layout(names: &[String]) -> Self {
        let mut store = Self::new();
        store.set_global_layout(names);
        store
    }

    pub fn set_global_layout(&mut self, names: &[String]) {
        self.slot_by_name.clear();
        self.slots.clear();
        self.slots.resize(names.len(), None);
        for (index, name) in names.iter().enumerate() {
            let slot = GlobalSlot::new(index);
            self.slot_by_name.insert(name.clone(), slot);
            if let Some(host_ref) = self.host_ref(name) {
                self.slots[index] = Some(host_ref);
            }
        }
    }

    pub fn insert_host<T>(&mut self, name: impl Into<String>, value: T) -> HostRef
    where
        T: ScriptHostObject + Send + 'static,
    {
        let name = name.into();
        let host_ref = HostRef::new(
            value.host_type_id(),
            HostObjectId::new(self.next_host_object_id),
            1,
        );
        self.next_host_object_id = self.next_host_object_id.saturating_add(1);
        if let Some(slot) = self.slot_by_name.get(&name).copied() {
            self.slots[slot.get()] = Some(host_ref);
        }
        self.globals.insert(
            name,
            HostGlobalBinding {
                host_ref,
                object: Box::new(value),
            },
        );
        host_ref
    }

    #[must_use]
    pub fn host_ref(&self, name: &str) -> Option<HostRef> {
        self.globals.get(name).map(|global| global.host_ref)
    }

    #[must_use]
    pub fn host_ref_by_slot(&self, slot: GlobalSlot) -> Option<HostRef> {
        self.slots.get(slot.get()).and_then(|host_ref| *host_ref)
    }

    fn binding(&self, path: &HostPath) -> Option<&HostGlobalBinding> {
        self.globals
            .values()
            .find(|global| global.host_ref == path.root)
    }

    fn binding_mut(&mut self, path: &HostPath) -> Option<&mut HostGlobalBinding> {
        self.globals
            .values_mut()
            .find(|global| global.host_ref == path.root)
    }
}

struct HostGlobalBinding {
    host_ref: HostRef,
    object: Box<dyn ScriptHostObject + Send>,
}

#[derive(Debug, Default)]
pub struct RuntimeScriptGlobalStore {
    heap: ScriptHeap,
    values: ScriptGlobalValues,
    retained_values: Arc<Mutex<RuntimeValueRoots>>,
}

impl RuntimeScriptGlobalStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_global_layout(names: &[String]) -> Self {
        Self {
            heap: ScriptHeap::default(),
            values: ScriptGlobalValues::with_layout(names),
            retained_values: Arc::new(Mutex::new(RuntimeValueRoots::default())),
        }
    }

    pub fn set_global_layout(&mut self, names: &[String]) {
        self.values.set_layout(names);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn insert(&mut self, name: impl Into<String>, value: OwnedValue) -> VmResult<()> {
        let mut budget = ExecutionBudget::unbounded();
        let value = owned_to_persistent_value(value, &mut self.heap, Some(&mut budget))?;
        self.values.insert(name.into(), value);
        self.collect();
        Ok(())
    }

    pub fn value(&mut self, name: &str) -> VmResult<Option<OwnedValue>> {
        let Some(value) = self.values.get(name) else {
            return Ok(None);
        };
        persistent_value_to_owned(&value, &mut self.heap).map(Some)
    }

    pub fn update(&mut self, name: &str, update: impl FnOnce(&mut OwnedValue)) -> VmResult<()> {
        let mut value = self.value(name)?.ok_or_else(|| VmError {
            kind: VmErrorKind::Host(HostErrorKind::MissingGlobal {
                name: name.to_owned(),
            }),
            source_span: None,
            call_stack: Default::default(),
        })?;
        update(&mut value);
        self.insert(name.to_owned(), value)
    }

    fn retain(&mut self, runtime_id: u64, value: Value) -> VelaValue {
        RuntimeValueRoots::retain(&self.retained_values, runtime_id, value)
    }

    fn roots(&self) -> Vec<Value> {
        let mut roots = self.values.values().collect::<Vec<_>>();
        roots.extend(
            self.retained_values
                .lock()
                .expect("runtime value roots mutex poisoned")
                .values(),
        );
        roots
    }

    fn collect(&mut self) {
        let mut roots = Vec::new();
        self.values
            .values()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        self.heap.collect_full(&roots);
    }
}

struct GlobalStoreAdapter<'call> {
    globals: &'call mut RuntimeGlobalStore,
    fallback: &'call mut dyn ScriptStateAdapter,
}

impl ScriptStateAdapter for GlobalStoreAdapter<'_> {
    fn global_ref(&self, name: &str) -> HostResult<HostRef> {
        self.globals
            .host_ref(name)
            .or_else(|| self.fallback.global_ref(name).ok())
            .ok_or_else(|| HostError {
                kind: HostErrorKind::MissingGlobal {
                    name: name.to_owned(),
                },
                source_span: None,
            })
    }

    fn global_ref_by_slot(&self, slot: GlobalSlot, name: &str) -> HostResult<HostRef> {
        self.globals
            .host_ref_by_slot(slot)
            .or_else(|| self.fallback.global_ref_by_slot(slot, name).ok())
            .ok_or_else(|| HostError {
                kind: HostErrorKind::MissingGlobal {
                    name: name.to_owned(),
                },
                source_span: None,
            })
    }

    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        if let Some(global) = self.globals.binding(path) {
            return global.object.read_host_path(path);
        }
        self.fallback.read_path(path)
    }

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        if let Some(global) = self.globals.binding_mut(path) {
            return global.object.write_host_path(path, value);
        }
        self.fallback.write_path(path, value)
    }

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()> {
        if let Some(global) = self.globals.binding_mut(path) {
            return global.object.remove_host_path(path);
        }
        self.fallback.remove_path(path)
    }

    fn call_method(
        &mut self,
        path: &HostPath,
        method: vela_common::HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        if let Some(global) = self.globals.binding_mut(path) {
            return global.object.call_host_method(path, method, args);
        }
        self.fallback.call_method(path, method, args)
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
