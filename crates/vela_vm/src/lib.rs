//! Register VM for Vela bytecode.

mod array_methods;
pub mod budget;
mod callback_method_dispatch;
mod closure_calls;
mod constant_loads;
pub mod error;
mod execution;
mod field_access;
mod frame;
pub mod heap;
pub mod heap_execution;
mod heap_values;
mod host_access;
mod host_values;
mod indexing;
pub mod iteration;
mod linked_execution;
mod map_methods;
mod math_stdlib;
mod method_runtime;
mod native_function_calls;
mod numeric_ops;
mod option_result;
mod option_result_methods;
pub mod owned_value;
pub mod ranges;
mod record_fields;
mod reflection;
mod reflection_values;
mod runtime_checks;
mod script_aggregate_construction;
mod script_builtin_methods;
mod script_function_calls;
mod script_method_calls;
mod script_methods;
mod script_object;
mod script_object_construction;
#[cfg(feature = "serde")]
pub mod serde;
#[cfg(all(test, feature = "serde"))]
mod serde_tests;
mod set_methods;
mod small_storage;
mod stdlib;
mod string_method_dispatch;
mod string_methods;
mod try_propagation;
pub mod value;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use error::{VmError, VmErrorKind, VmResult, VmStackFrame};
pub(crate) use frame::CallFrame;
use heap::{HeapValue, ScriptHeap};
use heap_execution::HeapExecution;
use heap_values::{
    allocate_heap_value, enum_variant_owner, owned_to_value, store_runtime_value,
    store_value_in_heap_if_needed, stored_runtime_value, value_from_constant, value_to_owned,
    values_equal,
};
use numeric_ops::{
    add_numeric, div_numeric, greater_equal_numeric, greater_numeric, less_equal_numeric,
    less_numeric, mul_numeric, negate_numeric, rem_numeric, sub_numeric,
};
use owned_value::OwnedValue;
pub(crate) use reflection_values::{value_from_reflect, value_to_reflect};
pub(crate) use runtime_checks::{expect_arity, expect_host_ref, expect_string};
use runtime_checks::{expect_int, is_truthy, validate_jump};
use script_methods::{ScriptMethodDispatch, call_method, call_method_id};
#[cfg(test)]
pub(crate) use script_object::ScriptFields;
use small_storage::SmallStorage;
use try_propagation::{TryPropagation, try_propagate_value};
use vela_bytecode::{
    CacheSiteId, Constant, HostTargetPlanId, InstructionOffset, LinkedCodeObject, LinkedProgram,
    Register, UnlinkedCodeObject, UnlinkedInstructionKind, UnlinkedProgram, UnlinkedProgramCode,
};
use vela_common::{GlobalSlot, HostTypeId, Span};
use vela_def::{DefPath, FunctionId};
use vela_host::adapter::ScriptStateAdapter;
use vela_host::resolved::{HostAccessOp, HostSchemaEpoch, ResolvedHostAccess};
#[cfg(test)]
use vela_reflect as reflect;
use vela_reflect::registry::TypeRegistry;

use budget::ExecutionBudget;
use value::Value;

pub(crate) struct ExecutionCall<'a> {
    pub(crate) code: &'a UnlinkedCodeObject,
    pub(crate) program: Option<&'a dyn UnlinkedProgramCode>,
    pub(crate) captures: &'a [Value],
    pub(crate) args: &'a [Value],
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: Option<InstructionOffset>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
}

impl ExecutionCall<'_> {
    fn stack_frame(&self) -> VmStackFrame {
        VmStackFrame::new(self.code.name.clone(), self.call_site)
            .with_bytecode_offset(self.call_site_offset)
    }
}

pub type NativeFunction =
    Arc<dyn Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static>;
pub type HostNativeFunction = Arc<
    dyn for<'host, 'budget> Fn(
            &[OwnedValue],
            &mut HostExecution<'host>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone, Default)]
pub struct Vm {
    native_ids: HashMap<FunctionId, NativeFunction>,
    host_native_ids: HashMap<FunctionId, HostNativeFunction>,
    type_registry: Option<Arc<TypeRegistry>>,
}

pub struct HostExecution<'host> {
    pub adapter: &'host mut dyn ScriptStateAdapter,
    pub access: &'host mut vela_host::access::HostAccess,
    pub script_globals: Option<&'host ScriptGlobalValues>,
}

#[derive(Clone, Debug, Default)]
pub struct ScriptGlobalValues {
    by_name: BTreeMap<String, Value>,
    slots: Vec<Option<Value>>,
    slot_by_name: BTreeMap<String, GlobalSlot>,
}

impl ScriptGlobalValues {
    #[must_use]
    pub fn with_layout(names: &[String]) -> Self {
        let mut values = Self::default();
        values.set_layout(names);
        values
    }

    pub fn set_layout(&mut self, names: &[String]) {
        self.slot_by_name.clear();
        self.slots.clear();
        self.slots.resize(names.len(), None);
        for (index, name) in names.iter().enumerate() {
            let slot = GlobalSlot::new(index);
            self.slot_by_name.insert(name.clone(), slot);
            if let Some(value) = self.by_name.get(name).copied() {
                self.slots[index] = Some(value);
            }
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    pub fn insert(&mut self, name: String, value: Value) {
        if let Some(slot) = self.slot_by_name.get(&name).copied() {
            self.slots[slot.get()] = Some(value);
        }
        self.by_name.insert(name, value);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Value> {
        self.by_name.get(name).copied()
    }

    #[must_use]
    pub fn get_resolved(&self, name: &str, slot: Option<GlobalSlot>) -> Option<Value> {
        if let Some(slot) = slot
            && slot.get() < self.slots.len()
        {
            return self.slots[slot.get()];
        }
        self.get(name)
    }

    #[must_use]
    pub fn get_slot(&self, slot: GlobalSlot) -> Option<Value> {
        self.slots.get(slot.get()).and_then(|value| *value)
    }

    pub fn values(&self) -> impl Iterator<Item = Value> + '_ {
        self.by_name.values().copied()
    }
}

pub struct PersistentHeapExecution<'heap, 'roots> {
    pub heap: &'heap mut ScriptHeap,
    pub roots: &'roots [Value],
}

pub trait VmInlineCaches {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn global_read_slot(&self, _site: CacheSiteId) -> Option<GlobalSlot> {
        None
    }

    fn set_global_read_slot(&self, _site: CacheSiteId, _slot: GlobalSlot) {}

    fn host_access(&self, _site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        None
    }

    fn set_host_access(&self, _site: CacheSiteId, _entry: HostInlineCacheEntry) {}
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostInlineCacheEntry {
    pub root_type: HostTypeId,
    pub plan_id: HostTargetPlanId,
    pub op: HostAccessOp,
    pub schema_epoch: HostSchemaEpoch,
    pub resolved: ResolvedHostAccess,
}

pub struct RuntimeMethodCall<'program, 'args, 'host, 'heap, 'roots, 'budget> {
    pub program: &'program dyn UnlinkedProgramCode,
    pub receiver: Value,
    pub method: &'args str,
    pub method_id: Option<vela_def::MethodId>,
    pub args: &'args [Value],
    pub host: &'host mut HostExecution<'host>,
    pub persistent: PersistentHeapExecution<'heap, 'roots>,
    pub budget: &'budget mut ExecutionBudget,
}

pub struct RuntimeCodeCall<'program, 'args, 'host, 'heap, 'roots, 'budget, 'caches> {
    pub program: &'program dyn UnlinkedProgramCode,
    pub code: &'program UnlinkedCodeObject,
    pub args: &'args [Value],
    pub host: &'host mut HostExecution<'host>,
    pub persistent: PersistentHeapExecution<'heap, 'roots>,
    pub budget: &'budget mut ExecutionBudget,
    pub inline_caches: Option<&'caches dyn VmInlineCaches>,
}

pub struct LinkedProgramHostCall<'program, 'entry, 'args, 'host, 'heap, 'roots, 'budget, 'caches> {
    pub program: &'program LinkedProgram,
    pub entry: &'entry str,
    pub args: &'args [OwnedValue],
    pub host: &'host mut HostExecution<'host>,
    pub persistent: PersistentHeapExecution<'heap, 'roots>,
    pub budget: &'budget mut ExecutionBudget,
    pub inline_caches: Option<&'caches dyn VmInlineCaches>,
}

impl Vm {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_native(
        &mut self,
        name: impl Into<String>,
        function: impl Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static,
    ) {
        let name = name.into();
        self.register_native_with_id(function_id_for_native_name(&name), name, function);
    }

    pub fn register_native_with_id(
        &mut self,
        id: FunctionId,
        name: impl Into<String>,
        function: impl Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static,
    ) {
        let _debug_name = name.into();
        self.native_ids.insert(id, Arc::new(function));
    }

    pub fn register_host_native(
        &mut self,
        name: impl Into<String>,
        function: impl for<'host> Fn(&[OwnedValue], &mut HostExecution<'host>) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) {
        let name = name.into();
        self.register_host_native_with_id(function_id_for_native_name(&name), name, function);
    }

    pub fn register_host_native_with_id(
        &mut self,
        id: FunctionId,
        name: impl Into<String>,
        function: impl for<'host> Fn(&[OwnedValue], &mut HostExecution<'host>) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) {
        let _debug_name = name.into();
        self.host_native_ids.insert(
            id,
            Arc::new(move |args, host, _budget| function(args, host)),
        );
    }

    pub fn register_budgeted_host_native(
        &mut self,
        name: impl Into<String>,
        function: impl for<'host, 'budget> Fn(
            &[OwnedValue],
            &mut HostExecution<'host>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) {
        let name = name.into();
        self.register_budgeted_host_native_with_id(
            function_id_for_native_name(&name),
            name,
            function,
        );
    }

    pub fn register_budgeted_host_native_with_id(
        &mut self,
        id: FunctionId,
        name: impl Into<String>,
        function: impl for<'host, 'budget> Fn(
            &[OwnedValue],
            &mut HostExecution<'host>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) {
        let _debug_name = name.into();
        self.host_native_ids.insert(id, Arc::new(function));
    }

    pub fn register_standard_natives(&mut self) {
        stdlib::register(self);
    }

    #[must_use]
    pub fn with_standard_natives(mut self) -> Self {
        self.register_standard_natives();
        self
    }

    pub fn register_type_registry(&mut self, registry: Arc<TypeRegistry>) {
        self.type_registry = Some(registry);
    }

    #[must_use]
    pub fn with_type_registry(mut self, registry: Arc<TypeRegistry>) -> Self {
        self.register_type_registry(registry);
        self
    }

    fn type_registry(&self) -> Option<&TypeRegistry> {
        self.type_registry.as_deref()
    }

    pub fn run(&self, code: &UnlinkedCodeObject) -> VmResult<OwnedValue> {
        let mut budget = ExecutionBudget::unbounded();
        self.run_with_budget(code, &mut budget)
    }

    pub fn run_with_budget(
        &self,
        code: &UnlinkedCodeObject,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let result = self.execute(
            code,
            None,
            &[],
            None,
            Some(&mut heap_execution),
            Some(budget),
        );
        owned_heap_result(result, &mut heap_execution, budget)
    }

    pub fn run_with_heap_and_budget(
        &self,
        code: &UnlinkedCodeObject,
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        self.execute(code, None, &[], None, Some(heap), Some(budget))
    }

    pub fn run_with_managed_heap_and_budget(
        &self,
        code: &UnlinkedCodeObject,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        self.run_with_budget(code, budget)
    }

    pub fn run_program(
        &self,
        program: &UnlinkedProgram,
        entry: &str,
        args: &[OwnedValue],
    ) -> VmResult<OwnedValue> {
        let code = program_entry(program, entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(args, &mut heap_execution, None)?;
        let result = self.execute(
            code,
            Some(program),
            &args,
            None,
            Some(&mut heap_execution),
            None,
        )?;
        value_to_owned(&result, Some(&heap_execution))
    }

    pub fn run_linked_program(
        &self,
        program: &LinkedProgram,
        entry: &str,
        args: &[OwnedValue],
    ) -> VmResult<OwnedValue> {
        let mut budget = ExecutionBudget::unbounded();
        self.run_linked_program_with_budget(program, entry, args, &mut budget)
    }

    pub fn run_linked_program_with_budget(
        &self,
        program: &LinkedProgram,
        entry: &str,
        args: &[OwnedValue],
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let code = linked_program_entry(program, entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(args, &mut heap_execution, Some(budget))?;
        let result = self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                code,
                program,
                captures: &[],
                args: &args,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
            },
            None,
            Some(&mut heap_execution),
            Some(budget),
        );
        owned_heap_result(result, &mut heap_execution, budget)
    }

    pub fn run_linked_program_with_host_budget_and_caches(
        &self,
        program: &LinkedProgram,
        entry: &str,
        args: &[OwnedValue],
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
        inline_caches: Option<&dyn VmInlineCaches>,
    ) -> VmResult<OwnedValue> {
        let code = linked_program_entry(program, entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(args, &mut heap_execution, Some(budget))?;
        let result = self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                code,
                program,
                captures: &[],
                args: &args,
                call_site: None,
                call_site_offset: None,
                inline_caches,
            },
            Some(host),
            Some(&mut heap_execution),
            Some(budget),
        );
        owned_heap_result(result, &mut heap_execution, budget)
    }

    pub fn run_program_with_budget(
        &self,
        program: &UnlinkedProgram,
        entry: &str,
        args: &[OwnedValue],
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let code = program_entry(program, entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(args, &mut heap_execution, Some(budget))?;
        let result = self.execute(
            code,
            Some(program),
            &args,
            None,
            Some(&mut heap_execution),
            Some(budget),
        );
        owned_heap_result(result, &mut heap_execution, budget)
    }

    pub fn run_program_runtime_with_heap_and_budget(
        &self,
        program: &UnlinkedProgram,
        entry: &str,
        args: &[Value],
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = program_entry(program, entry)?;
        self.execute(code, Some(program), args, None, Some(heap), Some(budget))
    }

    pub fn run_with_host(
        &self,
        code: &UnlinkedCodeObject,
        host: &mut HostExecution<'_>,
    ) -> VmResult<OwnedValue> {
        let mut budget = ExecutionBudget::unbounded();
        self.run_with_host_and_budget(code, host, &mut budget)
    }

    pub fn run_with_host_and_budget(
        &self,
        code: &UnlinkedCodeObject,
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let result = self.execute(
            code,
            None,
            &[],
            Some(host),
            Some(&mut heap_execution),
            Some(budget),
        );
        owned_heap_result(result, &mut heap_execution, budget)
    }

    pub fn run_with_host_heap_and_budget(
        &self,
        code: &UnlinkedCodeObject,
        host: &mut HostExecution<'_>,
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        self.execute(code, None, &[], Some(host), Some(heap), Some(budget))
    }

    pub fn run_program_with_host(
        &self,
        program: &UnlinkedProgram,
        entry: &str,
        args: &[OwnedValue],
        host: &mut HostExecution<'_>,
    ) -> VmResult<OwnedValue> {
        let code = program_entry(program, entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(args, &mut heap_execution, None)?;
        let result = self.execute(
            code,
            Some(program),
            &args,
            Some(host),
            Some(&mut heap_execution),
            None,
        )?;
        value_to_owned(&result, Some(&heap_execution))
    }

    pub fn run_program_with_host_and_budget(
        &self,
        program: &UnlinkedProgram,
        entry: &str,
        args: &[OwnedValue],
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let code = program_entry(program, entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(args, &mut heap_execution, Some(budget))?;
        let result = self.execute(
            code,
            Some(program),
            &args,
            Some(host),
            Some(&mut heap_execution),
            Some(budget),
        );
        owned_heap_result(result, &mut heap_execution, budget)
    }

    pub fn run_program_with_host_persistent_heap_and_budget(
        &self,
        program: &UnlinkedProgram,
        entry: &str,
        args: &[OwnedValue],
        host: &mut HostExecution<'_>,
        persistent: PersistentHeapExecution<'_, '_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let code = program_entry(program, entry)?;
        let mut heap_execution = HeapExecution::new(persistent.heap);
        let args = owned_args_to_runtime(args, &mut heap_execution, Some(budget))?;
        heap_execution.protect_values(persistent.roots);
        let result = self.execute(
            code,
            Some(program),
            &args,
            Some(host),
            Some(&mut heap_execution),
            Some(budget),
        );
        let result = result.and_then(|value| value_to_owned(&value, Some(&heap_execution)));
        let mut roots = Vec::new();
        persistent
            .roots
            .iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        heap_execution
            .heap
            .collect_full_with_budget(&roots, Some(budget));
        result
    }

    pub fn run_linked_program_host_call(
        &self,
        call: LinkedProgramHostCall<'_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> VmResult<OwnedValue> {
        let code = linked_program_entry(call.program, call.entry)?;
        let mut heap_execution = HeapExecution::new(call.persistent.heap);
        let args = owned_args_to_runtime(call.args, &mut heap_execution, Some(call.budget))?;
        heap_execution.protect_values(call.persistent.roots);
        let result = self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                code,
                program: call.program,
                captures: &[],
                args: &args,
                call_site: None,
                call_site_offset: None,
                inline_caches: call.inline_caches,
            },
            Some(call.host),
            Some(&mut heap_execution),
            Some(call.budget),
        );
        let result = result.and_then(|value| value_to_owned(&value, Some(&heap_execution)));
        let mut roots = Vec::new();
        call.persistent
            .roots
            .iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        heap_execution
            .heap
            .collect_full_with_budget(&roots, Some(call.budget));
        result
    }

    pub fn run_program_runtime_with_host_persistent_heap_and_budget(
        &self,
        program: &UnlinkedProgram,
        entry: &str,
        args: &[Value],
        host: &mut HostExecution<'_>,
        persistent: PersistentHeapExecution<'_, '_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = program_entry(program, entry)?;
        let mut heap_execution = HeapExecution::new(persistent.heap);
        heap_execution.protect_values(persistent.roots);
        heap_execution.protect_values(args);
        let result = self.execute(
            code,
            Some(program),
            args,
            Some(host),
            Some(&mut heap_execution),
            Some(budget),
        );
        let result = result?;
        let mut roots = Vec::new();
        persistent
            .roots
            .iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        result.trace_heap_refs(&mut roots);
        heap_execution
            .heap
            .collect_full_with_budget(&roots, Some(budget));
        Ok(result)
    }

    pub fn run_code_runtime_with_host_persistent_heap_and_budget<'host>(
        &self,
        program: &UnlinkedProgram,
        code: &UnlinkedCodeObject,
        args: &[Value],
        host: &'host mut HostExecution<'host>,
        persistent: PersistentHeapExecution<'_, '_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        self.run_runtime_code_call(RuntimeCodeCall {
            program,
            code,
            args,
            host,
            persistent,
            budget,
            inline_caches: None,
        })
    }

    pub fn run_runtime_code_call(
        &self,
        call: RuntimeCodeCall<'_, '_, '_, '_, '_, '_, '_>,
    ) -> VmResult<Value> {
        let mut heap_execution = HeapExecution::new(call.persistent.heap);
        heap_execution.protect_values(call.persistent.roots);
        heap_execution.protect_values(call.args);
        let result = self.execute_call(
            ExecutionCall {
                code: call.code,
                program: Some(call.program),
                captures: &[],
                args: call.args,
                call_site: None,
                call_site_offset: None,
                inline_caches: call.inline_caches,
            },
            Some(call.host),
            Some(&mut heap_execution),
            Some(call.budget),
        )?;
        let mut roots = Vec::new();
        call.persistent
            .roots
            .iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        result.trace_heap_refs(&mut roots);
        heap_execution
            .heap
            .collect_full_with_budget(&roots, Some(call.budget));
        Ok(result)
    }

    pub fn call_runtime_method(
        &self,
        call: RuntimeMethodCall<'_, '_, '_, '_, '_, '_>,
    ) -> VmResult<Value> {
        let mut heap_execution = HeapExecution::new(call.persistent.heap);
        heap_execution.protect_values(call.persistent.roots);
        heap_execution.protect_values(&[call.receiver]);
        heap_execution.protect_values(call.args);
        let caller_roots = Vec::new();
        let dispatch = ScriptMethodDispatch {
            vm: self,
            program: Some(call.program),
            linked_program: None,
            host: Some(call.host),
            heap: Some(&mut heap_execution),
            budget: Some(call.budget),
            caller_roots,
        };
        let mut receiver = call.receiver;
        let result = if let Some(method_id) = call.method_id {
            call_method_id(&mut receiver, call.method, method_id, call.args, dispatch)
        } else {
            call_method(&mut receiver, call.method, None, call.args, dispatch)
        }?;
        let result =
            store_value_in_heap_if_needed(result, Some(&mut heap_execution), Some(call.budget))?;
        let mut roots = Vec::new();
        call.persistent
            .roots
            .iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        receiver.trace_heap_refs(&mut roots);
        result.trace_heap_refs(&mut roots);
        heap_execution
            .heap
            .collect_full_with_budget(&roots, Some(call.budget));
        Ok(result)
    }

    pub fn run_program_with_host_managed_heap_and_budget(
        &self,
        program: &UnlinkedProgram,
        entry: &str,
        args: &[OwnedValue],
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        self.run_program_with_host_and_budget(program, entry, args, host, budget)
    }

    pub fn run_program_runtime_with_host_heap_and_budget(
        &self,
        program: &UnlinkedProgram,
        entry: &str,
        args: &[Value],
        host: &mut HostExecution<'_>,
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = program_entry(program, entry)?;
        self.execute(
            code,
            Some(program),
            args,
            Some(host),
            Some(heap),
            Some(budget),
        )
    }

    fn execute(
        &self,
        code: &UnlinkedCodeObject,
        program: Option<&dyn UnlinkedProgramCode>,
        args: &[Value],
        host: Option<&mut HostExecution<'_>>,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        self.execute_call(
            ExecutionCall {
                code,
                program,
                captures: &[],
                args,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
            },
            host,
            heap,
            budget,
        )
    }

    pub(crate) fn execute_call(
        &self,
        call: ExecutionCall<'_>,
        host: Option<&mut HostExecution<'_>>,
        heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        if let Some(budget) = &mut budget {
            budget
                .enter_call()
                .map_err(|error| error.with_call_frame(call.stack_frame()))?;
        }
        let frame = call.stack_frame();
        let fallback_span = call.call_site.or_else(|| {
            call.code
                .instructions
                .first()
                .and_then(|instruction| instruction.span)
        });
        let result = self
            .execute_body(call, host, heap, budget.as_deref_mut())
            .map_err(|error| {
                error
                    .with_source_span_if_absent(fallback_span)
                    .with_call_frame(frame)
            });
        if let Some(budget) = budget {
            budget.exit_call();
        }
        result
    }

    pub(crate) fn execute_code_object(
        &self,
        code: &UnlinkedCodeObject,
        program: Option<&dyn UnlinkedProgramCode>,
        args: &[Value],
        host: Option<&mut HostExecution<'_>>,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        self.execute(code, program, args, host, heap, budget)
    }
}

fn owned_args_to_runtime(
    args: &[OwnedValue],
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Vec<Value>> {
    args.iter()
        .cloned()
        .map(|arg| owned_to_value(arg, heap, budget.as_deref_mut()))
        .collect::<VmResult<Vec<_>>>()
}

fn owned_heap_result(
    result: VmResult<Value>,
    heap: &mut HeapExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<OwnedValue> {
    let result = result.and_then(|value| value_to_owned(&value, Some(heap)));
    heap.heap.collect_full_with_budget(&[], Some(budget));
    result
}

pub fn owned_to_persistent_value(
    value: OwnedValue,
    heap: &mut ScriptHeap,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let mut heap_execution = HeapExecution::new(heap);
    owned_to_value(value, &mut heap_execution, budget)
}

pub fn persistent_value_to_owned(value: &Value, heap: &mut ScriptHeap) -> VmResult<OwnedValue> {
    let heap_execution = HeapExecution::new(heap);
    value_to_owned(value, Some(&heap_execution))
}

fn program_entry<'program>(
    program: &'program (impl UnlinkedProgramCode + ?Sized),
    entry: &str,
) -> VmResult<&'program UnlinkedCodeObject> {
    program.function(entry).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: entry.to_owned(),
        })
    })
}

fn linked_program_entry<'program>(
    program: &'program LinkedProgram,
    entry: &str,
) -> VmResult<&'program LinkedCodeObject> {
    let function = program.entry_point_by_name(entry).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: entry.to_owned(),
        })
    })?;
    program.function(function).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: entry.to_owned(),
        })
    })
}

fn function_id_for_native_name(name: &str) -> FunctionId {
    if let Some((module, function)) = name.rsplit_once("::")
        && let Some(id) = vela_stdlib::std_function_id(module, function)
    {
        return id;
    }
    let mut segments = name.split("::").collect::<Vec<_>>();
    let function = segments.pop().unwrap_or(name);
    FunctionId::from_def_id(DefPath::function("host", segments, function).id())
}

#[cfg(test)]
mod tests;
