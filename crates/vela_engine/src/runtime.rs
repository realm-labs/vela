use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;

use vela_bytecode::Program;
use vela_common::{HostObjectId, SourceId};
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
    PersistentHeapExecution, ScriptGlobalLookup, owned_to_persistent_value,
    persistent_value_to_owned,
};

use crate::engine::Engine;
use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::reload::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};

#[derive(Clone)]
pub struct Runtime {
    engine: Engine,
    program: Program,
    hot_reload: Option<HotReloadRuntime>,
    globals: Rc<RefCell<RuntimeGlobalStore>>,
    script_globals: RuntimeScriptGlobalStore,
}

impl Runtime {
    #[must_use]
    pub fn new(engine: Engine, program: Program) -> Self {
        Self {
            engine,
            program,
            hot_reload: None,
            globals: Rc::new(RefCell::new(RuntimeGlobalStore::new())),
            script_globals: RuntimeScriptGlobalStore::new(),
        }
    }

    #[must_use]
    pub fn from_hot_reload_version(engine: Engine, version: ProgramVersion) -> Self {
        Self {
            engine,
            program: version.to_program(),
            hot_reload: Some(HotReloadRuntime::new(version)),
            globals: Rc::new(RefCell::new(RuntimeGlobalStore::new())),
            script_globals: RuntimeScriptGlobalStore::new(),
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

    pub fn insert_host_global<T>(&mut self, name: impl Into<String>, value: T) -> HostRef
    where
        T: ScriptHostObject + 'static,
    {
        self.globals.borrow_mut().insert_host(name, value)
    }

    #[must_use]
    pub fn host_global_ref(&self, name: &str) -> Option<HostRef> {
        self.globals.borrow().host_ref(name)
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
        Ok(Self::consume_reload_report(&mut self.program, hot_reload))
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
            globals: Rc::clone(&self.globals),
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
        let mut adapter = CallArgsAdapter {
            args,
            fallback: adapter,
        };
        self.call_raw(entry, &resolved, options, &mut adapter, access)
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

    fn current_hot_reload_version(&self) -> EngineResult<std::sync::Arc<ProgramVersion>> {
        self.hot_reload
            .as_ref()
            .map(HotReloadRuntime::current)
            .ok_or_else(|| EngineError::new(EngineErrorKind::RuntimeNotHotReloadEnabled))
    }

    fn check_optional_reload(&mut self) -> Option<HotReloadReport> {
        let hot_reload = self.hot_reload.as_mut()?;
        Self::consume_reload_report(&mut self.program, hot_reload)
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

pub struct CallArgs<'a> {
    entries: Vec<CallArg<'a>>,
    next_direct_object_id: u64,
}

const DIRECT_HOST_OBJECT_ID_BASE: u64 = 1 << 63;
const GLOBAL_HOST_OBJECT_ID_BASE: u64 = 1 << 62;

pub struct RuntimeGlobalStore {
    globals: BTreeMap<String, HostGlobalBinding>,
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
            next_host_object_id: GLOBAL_HOST_OBJECT_ID_BASE,
        }
    }

    pub fn insert_host<T>(&mut self, name: impl Into<String>, value: T) -> HostRef
    where
        T: ScriptHostObject + 'static,
    {
        let name = name.into();
        let host_ref = HostRef::new(
            value.host_type_id(),
            HostObjectId::new(self.next_host_object_id),
            1,
        );
        self.next_host_object_id = self.next_host_object_id.saturating_add(1);
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
    object: Box<dyn ScriptHostObject>,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeScriptGlobalStore {
    heap: ScriptHeap,
    values: RuntimeScriptGlobalValues,
}

impl RuntimeScriptGlobalStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.0.is_empty()
    }

    pub fn insert(&mut self, name: impl Into<String>, value: OwnedValue) -> VmResult<()> {
        let mut budget = ExecutionBudget::unbounded();
        let value = owned_to_persistent_value(value, &mut self.heap, Some(&mut budget))?;
        self.values.0.insert(name.into(), value);
        self.collect();
        Ok(())
    }

    pub fn value(&mut self, name: &str) -> VmResult<Option<OwnedValue>> {
        let Some(value) = self.values.0.get(name).copied() else {
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

    fn roots(&self) -> Vec<Value> {
        self.values.0.values().copied().collect()
    }

    fn collect(&mut self) {
        let mut roots = Vec::new();
        self.values
            .0
            .values()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        self.heap.collect_full(&roots);
    }
}

#[derive(Clone, Debug, Default)]
struct RuntimeScriptGlobalValues(BTreeMap<String, Value>);

impl ScriptGlobalLookup for RuntimeScriptGlobalValues {
    fn get_script_global(&self, name: &str) -> Option<Value> {
        self.0.get(name).copied()
    }
}

struct GlobalStoreAdapter<'call> {
    globals: Rc<RefCell<RuntimeGlobalStore>>,
    fallback: &'call mut dyn ScriptStateAdapter,
}

impl ScriptStateAdapter for GlobalStoreAdapter<'_> {
    fn global_ref(&self, name: &str) -> HostResult<HostRef> {
        self.globals
            .borrow()
            .host_ref(name)
            .or_else(|| self.fallback.global_ref(name).ok())
            .ok_or_else(|| HostError {
                kind: HostErrorKind::MissingGlobal {
                    name: name.to_owned(),
                },
                source_span: None,
            })
    }

    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        {
            let globals = self.globals.borrow();
            if let Some(global) = globals.binding(path) {
                return global.object.read_host_path(path);
            }
        }
        self.fallback.read_path(path)
    }

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        {
            let mut globals = self.globals.borrow_mut();
            if let Some(global) = globals.binding_mut(path) {
                return global.object.write_host_path(path, value);
            }
        }
        self.fallback.write_path(path, value)
    }

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()> {
        {
            let mut globals = self.globals.borrow_mut();
            if let Some(global) = globals.binding_mut(path) {
                return global.object.remove_host_path(path);
            }
        }
        self.fallback.remove_path(path)
    }

    fn call_method(
        &mut self,
        path: &HostPath,
        method: vela_common::HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        {
            let mut globals = self.globals.borrow_mut();
            if let Some(global) = globals.binding_mut(path) {
                return global.object.call_host_method(path, method, args);
            }
        }
        self.fallback.call_method(path, method, args)
    }
}

impl Default for CallArgs<'_> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_direct_object_id: DIRECT_HOST_OBJECT_ID_BASE,
        }
    }
}

impl<'a> CallArgs<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_positional(args: impl IntoIterator<Item = OwnedValue>) -> Self {
        Self {
            entries: args.into_iter().map(CallArg::Positional).collect(),
            next_direct_object_id: DIRECT_HOST_OBJECT_ID_BASE,
        }
    }

    pub fn push(&mut self, value: impl Into<OwnedValue>) -> &mut Self {
        self.entries.push(CallArg::Positional(value.into()));
        self
    }

    pub fn push_value(
        &mut self,
        name: impl Into<String>,
        value: impl Into<OwnedValue>,
    ) -> &mut Self {
        self.entries.push(CallArg::Named {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    pub fn push_host_handle(
        &mut self,
        name: impl Into<String>,
        host_ref: vela_host::path::HostRef,
    ) -> &mut Self {
        self.push_value(name, OwnedValue::HostRef(host_ref))
    }

    pub fn push_host_ref<T>(&mut self, name: impl Into<String>, value: &'a T) -> &mut Self
    where
        T: ScriptHostObject + 'a,
    {
        let host_ref = self.next_direct_host_ref(value.host_type_id());
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref,
            binding: HostArgBinding::Shared(value),
        });
        self
    }

    pub fn push_host_mut<T>(&mut self, name: impl Into<String>, value: &'a mut T) -> &mut Self
    where
        T: ScriptHostObject + 'a,
    {
        let host_ref = self.next_direct_host_ref(value.host_type_id());
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref,
            binding: HostArgBinding::Mutable(value),
        });
        self
    }

    #[must_use]
    pub fn with(mut self, value: impl Into<OwnedValue>) -> Self {
        self.push(value);
        self
    }

    #[must_use]
    pub fn with_value(mut self, name: impl Into<String>, value: impl Into<OwnedValue>) -> Self {
        self.push_value(name, value);
        self
    }

    #[must_use]
    pub fn with_host_handle(
        mut self,
        name: impl Into<String>,
        host_ref: vela_host::path::HostRef,
    ) -> Self {
        self.push_host_handle(name, host_ref);
        self
    }

    #[must_use]
    pub fn with_host_ref<T>(mut self, name: impl Into<String>, value: &'a T) -> Self
    where
        T: ScriptHostObject + 'a,
    {
        self.push_host_ref(name, value);
        self
    }

    #[must_use]
    pub fn with_host_mut<T>(mut self, name: impl Into<String>, value: &'a mut T) -> Self
    where
        T: ScriptHostObject + 'a,
    {
        self.push_host_mut(name, value);
        self
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn resolve(
        &self,
        entry: &str,
        params: &[String],
        param_defaults: &[bool],
    ) -> VmResult<Vec<OwnedValue>> {
        match self.mode()? {
            CallArgsMode::Empty | CallArgsMode::Positional => {
                Ok(self.entries.iter().map(CallArg::owned_value).collect())
            }
            CallArgsMode::Named => self.resolve_named(entry, params, param_defaults),
        }
    }

    fn mode(&self) -> VmResult<CallArgsMode> {
        let mut has_positional = false;
        let mut has_named = false;
        for entry in &self.entries {
            match entry {
                CallArg::Positional(_) => has_positional = true,
                CallArg::Named { .. } | CallArg::NamedHost { .. } => has_named = true,
            }
        }
        match (has_positional, has_named) {
            (false, false) => Ok(CallArgsMode::Empty),
            (true, false) => Ok(CallArgsMode::Positional),
            (false, true) => Ok(CallArgsMode::Named),
            (true, true) => Err(call_args_type_error(
                "mixed positional and named call arguments",
            )),
        }
    }

    fn resolve_named(
        &self,
        entry: &str,
        params: &[String],
        param_defaults: &[bool],
    ) -> VmResult<Vec<OwnedValue>> {
        let mut values = BTreeMap::new();
        for (index, arg) in self.entries.iter().enumerate() {
            let Some(name) = arg.name() else {
                continue;
            };
            if !params.iter().any(|param| param == name) {
                return Err(call_args_type_error("unknown named call argument"));
            }
            if values.insert(name.to_owned(), index).is_some() {
                return Err(call_args_type_error("duplicate named call argument"));
            }
        }

        let mut resolved = Vec::with_capacity(params.len());
        for (index, param) in params.iter().enumerate() {
            if let Some(arg_index) = values.remove(param) {
                resolved.push(self.entries[arg_index].owned_value());
            } else if param_defaults.get(index).copied().unwrap_or(false) {
                resolved.push(OwnedValue::Missing);
            } else {
                return Err(VmError {
                    kind: VmErrorKind::ArityMismatch {
                        name: entry.to_owned(),
                        expected: params.len(),
                        actual: self.entries.len(),
                    },
                    source_span: None,
                    call_stack: Default::default(),
                });
            }
        }
        Ok(resolved)
    }

    fn next_direct_host_ref(&mut self, type_id: vela_common::HostTypeId) -> HostRef {
        let object_id = HostObjectId::new(self.next_direct_object_id);
        self.next_direct_object_id = self.next_direct_object_id.saturating_add(1);
        HostRef::new(type_id, object_id, 1)
    }
}

impl From<Vec<OwnedValue>> for CallArgs<'_> {
    fn from(value: Vec<OwnedValue>) -> Self {
        Self::from_positional(value)
    }
}

enum CallArg<'a> {
    Positional(OwnedValue),
    Named {
        name: String,
        value: OwnedValue,
    },
    NamedHost {
        name: String,
        host_ref: HostRef,
        binding: HostArgBinding<'a>,
    },
}

impl CallArg<'_> {
    fn owned_value(&self) -> OwnedValue {
        match self {
            Self::Positional(value) | Self::Named { value, .. } => value.clone(),
            Self::NamedHost { host_ref, .. } => OwnedValue::HostRef(*host_ref),
        }
    }

    fn name(&self) -> Option<&str> {
        match self {
            Self::Positional(_) => None,
            Self::Named { name, .. } | Self::NamedHost { name, .. } => Some(name),
        }
    }
}

enum HostArgBinding<'a> {
    Shared(&'a dyn ScriptHostObject),
    Mutable(&'a mut dyn ScriptHostObject),
}

struct CallArgsAdapter<'call, 'args> {
    args: &'call mut CallArgs<'args>,
    fallback: &'call mut dyn ScriptStateAdapter,
}

impl<'call, 'args> CallArgsAdapter<'call, 'args> {
    fn direct_binding<'s>(&'s self, path: &HostPath) -> Option<&'s HostArgBinding<'args>> {
        for entry in &self.args.entries {
            if let CallArg::NamedHost {
                host_ref, binding, ..
            } = entry
                && *host_ref == path.root
            {
                return Some(binding);
            }
        }
        None
    }

    fn direct_binding_mut<'s>(
        &'s mut self,
        path: &HostPath,
    ) -> Option<&'s mut HostArgBinding<'args>> {
        for entry in &mut self.args.entries {
            if let CallArg::NamedHost {
                host_ref, binding, ..
            } = entry
                && *host_ref == path.root
            {
                return Some(binding);
            }
        }
        None
    }

    fn direct_access_error(path: &HostPath, action: &'static str) -> HostError {
        HostError {
            kind: HostErrorKind::PermissionDenied {
                path: path.clone(),
                action,
            },
            source_span: None,
        }
    }
}

impl ScriptStateAdapter for CallArgsAdapter<'_, '_> {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        match self.direct_binding(path) {
            Some(HostArgBinding::Shared(object)) => object.read_host_path(path),
            Some(HostArgBinding::Mutable(object)) => object.read_host_path(path),
            None => self.fallback.read_path(path),
        }
    }

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        match self.direct_binding_mut(path) {
            Some(HostArgBinding::Shared(_)) => Err(Self::direct_access_error(path, "write")),
            Some(HostArgBinding::Mutable(object)) => object.write_host_path(path, value),
            None => self.fallback.write_path(path, value),
        }
    }

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()> {
        match self.direct_binding_mut(path) {
            Some(HostArgBinding::Shared(_)) => Err(Self::direct_access_error(path, "write")),
            Some(HostArgBinding::Mutable(object)) => object.remove_host_path(path),
            None => self.fallback.remove_path(path),
        }
    }

    fn call_method(
        &mut self,
        path: &HostPath,
        method: vela_common::HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        match self.direct_binding_mut(path) {
            Some(HostArgBinding::Shared(_)) => Err(Self::direct_access_error(path, "call")),
            Some(HostArgBinding::Mutable(object)) => object.call_host_method(path, method, args),
            None => self.fallback.call_method(path, method, args),
        }
    }
}

struct EmptyStateAdapter;

impl ScriptStateAdapter for EmptyStateAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn write_path(&mut self, path: &HostPath, _value: HostValue) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn call_method(
        &mut self,
        _path: &HostPath,
        method: vela_common::HostMethodId,
        _args: &[HostValue],
    ) -> HostResult<HostValue> {
        Err(HostError {
            kind: HostErrorKind::UnsupportedMethod { method },
            source_span: None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallArgsMode {
    Empty,
    Positional,
    Named,
}

fn call_args_type_error(operation: &'static str) -> VmError {
    VmError {
        kind: VmErrorKind::TypeMismatch { operation },
        source_span: None,
        call_stack: Default::default(),
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
