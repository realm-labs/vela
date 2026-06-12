//! Register VM for Vela bytecode.

mod array_methods;
pub mod budget;
mod bytes_methods;
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
mod numeric_conversions;
mod numeric_ops;
mod option_result;
mod option_result_methods;
pub mod owned_value;
pub mod ranges;
mod record_fields;
mod reflection;
mod reflection_values;
mod runtime_checks;
mod runtime_type_guards;
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
mod standard_method_cache;
mod std_method_ids;
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
    add_numeric, binary_float_literal_numeric, binary_int_literal_numeric, div_numeric,
    greater_equal_numeric, greater_numeric, less_equal_numeric, less_numeric, mul_numeric,
    negate_numeric, rem_numeric, sub_numeric,
};
use owned_value::OwnedValue;
pub(crate) use reflection_values::{
    runtime_value_to_reflect, value_from_reflect, value_to_reflect,
};
pub(crate) use runtime_checks::{expect_arity, expect_host_ref, expect_string};
use runtime_checks::{expect_int, is_truthy, validate_jump};
#[cfg(test)]
pub(crate) use script_object::ScriptFields;
use small_storage::SmallStorage;
#[cfg(test)]
use vela_bytecode::UnlinkedProgram;
use vela_bytecode::{
    CacheSiteId, DebugNameId, FieldSlot, HostTargetPlanId, InstructionOffset, LinkedCodeObject,
    LinkedProgram, MethodDispatchHandle, Register, ScriptFunctionHandle, UnlinkedCodeObject,
    UnlinkedInstructionKind, UnlinkedProgramCode,
};
use vela_common::{GlobalSlot, HostMethodId, HostTypeId, ShapeId, Span};
use vela_def::{DefPath, FunctionId, MethodId, TypeId};
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
    pub(crate) check_param_guards: bool,
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
pub(crate) type BorrowedHostNativeFunction = Arc<
    dyn for<'host, 'heap, 'budget> Fn(
            &[Value],
            &HeapExecution<'heap>,
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
    borrowed_host_native_ids: HashMap<FunctionId, BorrowedHostNativeFunction>,
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

    fn record_field(&self, _site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        None
    }

    fn set_record_field(&self, _site: CacheSiteId, _entry: RecordFieldInlineCacheEntry) {}

    fn method_dispatch(&self, _site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        None
    }

    fn set_method_dispatch(&self, _site: CacheSiteId, _entry: MethodInlineCacheEntry) {}
}

pub trait VmBytecodeProfiler {
    fn record_instruction(&self, _function: DebugNameId, _offset: InstructionOffset) {}
}

pub(crate) fn validate_inline_cache_layout(
    inline_caches: Option<&dyn VmInlineCaches>,
    required: usize,
) -> VmResult<()> {
    let Some(inline_caches) = inline_caches else {
        return Ok(());
    };
    let actual = inline_caches.len();
    if actual < required {
        return Err(VmError::new(VmErrorKind::InlineCacheLayoutMismatch {
            required,
            actual,
        }));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostInlineCacheEntry {
    pub root_type: HostTypeId,
    pub plan_id: HostTargetPlanId,
    pub op: HostAccessOp,
    pub schema_epoch: HostSchemaEpoch,
    pub resolved: ResolvedHostAccess,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordFieldInlineCacheEntry {
    pub type_id: TypeId,
    pub shape_id: ShapeId,
    pub field: FieldSlot,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MethodInlineCacheEntry {
    pub dispatch: MethodDispatchHandle,
    pub debug_name: DebugNameId,
    pub target: MethodInlineCacheTarget,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MethodInlineCacheTarget {
    Script {
        method_id: MethodId,
        function: ScriptFunctionHandle,
    },
    Value {
        method_id: MethodId,
        standard_method: Option<StandardMethodInlineCacheEntry>,
    },
    Host {
        method_id: HostMethodId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StandardMethodInlineCacheEntry {
    pub receiver: StandardMethodReceiver,
    pub target: StandardMethodInlineCacheTarget,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StandardMethodReceiver {
    String,
    Bytes,
    Range,
    Array,
    Map,
    Set,
    Option,
    Result,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StandardMethodInlineCacheTarget {
    Len,
    IsEmpty,
    Contains,
    First,
    Last,
    IndexOf,
    StartsWith,
    EndsWith,
    Find,
    StripPrefix,
    StripSuffix,
    CharAt,
    Split,
    SplitOnce,
    SplitLines,
    SplitWhitespace,
    ParseInt,
    ParseFloat,
    ParseBool,
    ToUpper,
    ToLower,
    Trim,
    TrimStart,
    TrimEnd,
    Has,
    IsSubset,
    IsSuperset,
    IsDisjoint,
    Get,
    GetOr,
    Slice,
    Reverse,
    Repeat,
    Replace,
    ToHex,
    ReadU32Le,
    ReadU32Be,
    IsSome,
    IsNone,
    IsOk,
    IsErr,
    UnwrapOr,
}

pub struct LinkedRuntimeCodeCall<'program, 'args, 'host, 'heap, 'roots, 'budget, 'caches> {
    pub program: &'program LinkedProgram,
    pub code: &'program LinkedCodeObject,
    pub args: &'args [Value],
    pub host: &'host mut HostExecution<'host>,
    pub persistent: PersistentHeapExecution<'heap, 'roots>,
    pub budget: &'budget mut ExecutionBudget,
    pub inline_caches: Option<&'caches dyn VmInlineCaches>,
    pub bytecode_profiler: Option<&'caches dyn VmBytecodeProfiler>,
}

pub struct LinkedProgramHostCall<'program, 'entry, 'args, 'host, 'heap, 'roots, 'budget, 'caches> {
    pub program: &'program LinkedProgram,
    pub entry: &'entry str,
    pub args: &'args [OwnedValue],
    pub host: &'host mut HostExecution<'host>,
    pub persistent: PersistentHeapExecution<'heap, 'roots>,
    pub budget: &'budget mut ExecutionBudget,
    pub inline_caches: Option<&'caches dyn VmInlineCaches>,
    pub bytecode_profiler: Option<&'caches dyn VmBytecodeProfiler>,
}

pub struct LinkedProgramHostBudgetCall<'program, 'entry, 'args, 'host, 'budget, 'caches> {
    pub program: &'program LinkedProgram,
    pub entry: &'entry str,
    pub args: &'args [OwnedValue],
    pub host: &'host mut HostExecution<'host>,
    pub budget: &'budget mut ExecutionBudget,
    pub inline_caches: Option<&'caches dyn VmInlineCaches>,
    pub bytecode_profiler: Option<&'caches dyn VmBytecodeProfiler>,
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
        self.register_native_with_id(function_id_for_native_name(&name), function);
    }

    pub fn register_native_with_id(
        &mut self,
        id: FunctionId,
        function: impl Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static,
    ) {
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
        self.register_host_native_with_id(function_id_for_native_name(&name), function);
    }

    pub fn register_host_native_with_id(
        &mut self,
        id: FunctionId,
        function: impl for<'host> Fn(&[OwnedValue], &mut HostExecution<'host>) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) {
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
        self.register_budgeted_host_native_with_id(function_id_for_native_name(&name), function);
    }

    pub fn register_budgeted_host_native_with_id(
        &mut self,
        id: FunctionId,
        function: impl for<'host, 'budget> Fn(
            &[OwnedValue],
            &mut HostExecution<'host>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) {
        self.host_native_ids.insert(id, Arc::new(function));
    }

    pub(crate) fn register_borrowed_host_native(
        &mut self,
        name: impl Into<String>,
        function: impl for<'host, 'heap, 'budget> Fn(
            &[Value],
            &HeapExecution<'heap>,
            &mut HostExecution<'host>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) {
        let name = name.into();
        self.borrowed_host_native_ids
            .insert(function_id_for_native_name(&name), Arc::new(function));
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

    pub fn native_implementation_ids(&self) -> impl Iterator<Item = FunctionId> + '_ {
        self.native_ids
            .keys()
            .chain(self.host_native_ids.keys())
            .copied()
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
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
                bytecode_profiler: None,
            },
            None,
            Some(&mut heap_execution),
            Some(budget),
        );
        owned_heap_result(result, &mut heap_execution, budget)
    }

    pub fn run_linked_program_with_heap_and_budget(
        &self,
        program: &LinkedProgram,
        entry: &str,
        args: &[Value],
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = linked_program_entry(program, entry)?;
        self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                code,
                program,
                captures: &[],
                args,
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
                bytecode_profiler: None,
            },
            None,
            Some(heap),
            Some(budget),
        )
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
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches,
                bytecode_profiler: None,
            },
            Some(host),
            Some(&mut heap_execution),
            Some(budget),
        );
        owned_heap_result(result, &mut heap_execution, budget)
    }

    pub fn run_linked_program_host_budget_call(
        &self,
        call: LinkedProgramHostBudgetCall<'_, '_, '_, '_, '_, '_>,
    ) -> VmResult<OwnedValue> {
        let code = linked_program_entry(call.program, call.entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(call.args, &mut heap_execution, Some(call.budget))?;
        let result = self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                code,
                program: call.program,
                captures: &[],
                args: &args,
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: call.inline_caches,
                bytecode_profiler: call.bytecode_profiler,
            },
            Some(call.host),
            Some(&mut heap_execution),
            Some(call.budget),
        );
        owned_heap_result(result, &mut heap_execution, call.budget)
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
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: call.inline_caches,
                bytecode_profiler: call.bytecode_profiler,
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

    pub fn run_linked_runtime_code_call(
        &self,
        call: LinkedRuntimeCodeCall<'_, '_, '_, '_, '_, '_, '_>,
    ) -> VmResult<Value> {
        let mut heap_execution = HeapExecution::new(call.persistent.heap);
        heap_execution.protect_values(call.persistent.roots);
        heap_execution.protect_values(call.args);
        let result = self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                code: call.code,
                program: call.program,
                captures: &[],
                args: call.args,
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: call.inline_caches,
                bytecode_profiler: call.bytecode_profiler,
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
        self.execute_call(
            ExecutionCall {
                code,
                program,
                captures: &[],
                args,
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
            },
            host,
            heap,
            budget,
        )
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
