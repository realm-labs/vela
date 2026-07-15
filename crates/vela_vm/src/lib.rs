#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Register VM for Vela bytecode.

mod array_methods;
mod async_resume;
pub mod backend_conformance;
pub mod budget;
mod bytes_methods;
mod callback_method_dispatch;
mod char_methods;
mod closure_calls;
mod collection_mutation;
mod constant_loads;
mod container_contracts;
mod dynamic_method_resolution;
mod equality;
pub mod error;
mod execution_reentry;
mod execution_session;
mod field_access;
mod format_strings;
mod frame;
pub mod heap;
pub mod heap_execution;
mod heap_values;
mod host_access;
mod host_values;
mod i64_ops;
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
mod resumable_callbacks;
mod runtime_checks;
mod runtime_type_guards;
mod script_aggregate_construction;
mod script_builtin_methods;
mod script_function_calls;
mod script_map;
mod script_method_calls;
mod script_methods;
mod script_object;
mod script_object_construction;
mod script_set;
#[cfg(feature = "serde")]
pub mod serde;
#[cfg(all(test, feature = "serde"))]
mod serde_tests;
mod set_methods;
mod small_storage;
mod standard_method_cache;
mod std_method_ids;
mod stdlib;
mod string_methods;
#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod test_support;
mod try_propagation;
mod tuple_fields;
pub mod value;
mod value_key;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub(crate) use equality::{identity_equal, identity_not_equal};
use error::{VmError, VmErrorKind, VmResult};
pub(crate) use frame::CallFrame;
use heap::{HeapValue, ScriptHeap};
use heap_execution::HeapExecution;
use heap_values::{
    allocate_heap_value, enum_variant_owner, owned_to_value, store_runtime_value,
    store_value_in_heap_if_needed, stored_runtime_value, value_from_constant, value_to_owned,
};
pub use heap_values::{
    allocate_zero_field_record, owned_to_persistent_value, persistent_value_to_owned,
};
use owned_value::OwnedValue;
pub(crate) use reflection_values::{
    runtime_value_to_reflect, value_from_reflect, value_to_reflect,
};
use runtime_checks::expect_int;
pub(crate) use runtime_checks::{expect_arity, expect_host_ref, expect_string};
#[cfg(test)]
pub(crate) use script_object::ScriptFields;
use small_storage::SmallStorage;
use vela_bytecode::{
    CacheSiteId, DebugNameId, FieldSlot, HostTargetPlanId, InstructionOffset, LinkedArtifact,
    LinkedProgram, MethodDispatchHandle, ScriptFunctionHandle,
};
#[cfg(test)]
use vela_bytecode::{Register, UnlinkedCodeObject, UnlinkedInstructionKind, UnlinkedProgram};
use vela_common::{HostMethodId, HostTypeId, ShapeId, StateSlot};
use vela_def::{DefPath, FunctionId, MethodId, TypeId};
use vela_host::adapter::ScriptStateAdapter;
use vela_host::resolved::{HostAccessOp, HostSchemaEpoch, ResolvedHostAccess};
#[cfg(test)]
use vela_reflect as reflect;
use vela_reflect::registry::TypeRegistry;

use budget::ExecutionBudget;
use value::Value;

pub use async_resume::PreparedAsyncCall;
pub use execution_reentry::LinkedExecutionReentry;
pub use execution_session::{LinkedExecutionSession, LinkedExecutionStart};
pub use linked_execution::LinkedDriveOutcome;

pub type NativeFunction =
    Arc<dyn Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static>;
pub type NativeCallFuture<'call> =
    Pin<Box<dyn Future<Output = VmResult<OwnedValue>> + Send + 'call>>;
pub type AsyncNativeFunction =
    Arc<dyn for<'call> Fn(&'call [OwnedValue]) -> NativeCallFuture<'call> + Send + Sync + 'static>;
pub type AsyncHostNativeFunction = Arc<
    dyn for<'call, 'host, 'budget> Fn(
            &'call [OwnedValue],
            &'call mut HostExecution<'host>,
            Option<&'call mut ExecutionBudget>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
>;
pub type AsyncHostMethodFunction = Arc<
    dyn for<'call, 'host, 'budget> Fn(
            &'call vela_host::path::HostPath,
            &'call [OwnedValue],
            &'call mut HostExecution<'host>,
            Option<&'call mut ExecutionBudget>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
>;
pub type AsyncDirectHostMethodFunction = Arc<
    dyn for<'host> Fn(
            vela_host::path::HostRef,
            vela_host::lease::ErasedHostLease<'host>,
            Vec<OwnedValue>,
        ) -> NativeCallFuture<'host>
        + Send
        + Sync
        + 'static,
>;
pub(crate) enum ConditionalAsyncNativeFunction {
    Pure(AsyncNativeFunction),
    Host(AsyncHostNativeFunction),
    HostMethod {
        function: AsyncHostMethodFunction,
        receiver: vela_host::path::HostPath,
    },
    DirectHostMethod {
        function: AsyncDirectHostMethodFunction,
        receiver: vela_host::path::HostPath,
        lease_kind: vela_host::lease::HostLeaseKind,
    },
}

pub(crate) enum ConditionalHostNativeOutcome {
    Complete(OwnedValue),
    Async {
        function: ConditionalAsyncNativeFunction,
        args: Vec<OwnedValue>,
        diagnostic_name: String,
    },
}

pub(crate) type ConditionalHostNativeFunction = Arc<
    dyn for<'host, 'budget> Fn(
            &[OwnedValue],
            &mut HostExecution<'host>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<ConditionalHostNativeOutcome>
        + Send
        + Sync
        + 'static,
>;
pub type BorrowedNativeFunction = Arc<
    dyn for<'heap, 'budget> Fn(
            &[Value],
            &HeapExecution<'heap>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
>;
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
    async_native_ids: HashMap<FunctionId, AsyncNativeFunction>,
    async_host_native_ids: HashMap<FunctionId, AsyncHostNativeFunction>,
    async_host_method_ids: HashMap<HostMethodId, AsyncHostMethodFunction>,
    async_direct_host_method_ids: HashMap<
        HostMethodId,
        (
            vela_host::lease::HostLeaseKind,
            AsyncDirectHostMethodFunction,
        ),
    >,
    conditional_host_native_ids: HashMap<FunctionId, ConditionalHostNativeFunction>,
    borrowed_native_ids: HashMap<FunctionId, BorrowedNativeFunction>,
    host_native_ids: HashMap<FunctionId, HostNativeFunction>,
    borrowed_host_native_ids: HashMap<FunctionId, BorrowedHostNativeFunction>,
    type_registry: Option<Arc<TypeRegistry>>,
}

pub struct HostExecution<'host> {
    pub adapter: &'host mut (dyn ScriptStateAdapter + Send),
    pub access: &'host mut vela_host::access::HostAccess,
    pub state_values: Option<&'host mut VmStateValues>,
}

#[derive(Clone, Debug, Default)]
pub struct VmStateValues {
    by_id: BTreeMap<vela_def::StateId, Value>,
}

impl VmStateValues {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn insert(&mut self, state: vela_def::StateId, value: Value) {
        self.by_id.insert(state, value);
    }

    #[must_use]
    pub fn get(&self, state: vela_def::StateId) -> Option<Value> {
        self.by_id.get(&state).copied()
    }

    pub fn values(&self) -> impl Iterator<Item = Value> + '_ {
        self.by_id.values().copied()
    }

    pub fn retain(&mut self, mut keep: impl FnMut(vela_def::StateId, Value) -> bool) {
        self.by_id.retain(|state, value| keep(*state, *value));
    }
}

pub struct PersistentHeapExecution<'heap, 'roots> {
    pub heap: &'heap mut ScriptHeap,
    pub roots: &'roots [Value],
}

pub trait VmInlineCaches {
    fn for_generation(
        &self,
        _generation: vela_bytecode::ExecutableGenerationId,
    ) -> Option<&dyn VmInlineCaches> {
        None
    }

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn state_read_slot(&self, _site: CacheSiteId) -> Option<StateSlot> {
        None
    }

    fn set_state_read_slot(&self, _site: CacheSiteId, _slot: StateSlot) {}

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

    fn dynamic_method_dispatch(&self, _site: CacheSiteId) -> Option<DynamicMethodInlineCacheEntry> {
        None
    }

    fn set_dynamic_method_dispatch(
        &self,
        _site: CacheSiteId,
        _entry: DynamicMethodInlineCacheEntry,
    ) {
    }

    fn native_call(&self, _site: CacheSiteId) -> Option<NativeInlineCacheEntry> {
        None
    }

    fn set_native_call(&self, _site: CacheSiteId, _entry: NativeInlineCacheEntry) {}
}

pub trait VmBytecodeProfiler {
    fn for_generation(
        &self,
        _generation: vela_bytecode::ExecutableGenerationId,
    ) -> Option<&dyn VmBytecodeProfiler> {
        None
    }

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
    pub target: HostInlineCacheTarget,
    pub op: HostAccessOp,
    pub schema_epoch: HostSchemaEpoch,
    pub resolved: ResolvedHostAccess,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HostInlineCacheTarget {
    TargetPlan(HostTargetPlanId),
    RootObject,
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DynamicMethodInlineCacheEntry {
    pub method_name: DebugNameId,
    pub receiver_guard: DynamicReceiverGuard,
    pub target: DynamicMethodInlineCacheTarget,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DynamicReceiverGuard {
    StdValue {
        receiver: StandardMethodReceiver,
    },
    ScriptType {
        type_name: String,
        shape_id: Option<ShapeId>,
    },
    HostType {
        type_id: HostTypeId,
        schema_epoch: HostSchemaEpoch,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DynamicMethodInlineCacheTarget {
    Script {
        dispatch: MethodDispatchHandle,
        function: ScriptFunctionHandle,
    },
    Host {
        method_id: HostMethodId,
    },
    StandardValue {
        method_id: MethodId,
        standard_method: Option<StandardMethodInlineCacheEntry>,
    },
}

#[derive(Clone)]
pub struct NativeInlineCacheEntry {
    native: FunctionId,
    target: native_function_calls::NativeCallTarget,
}

impl NativeInlineCacheEntry {
    pub(crate) const fn new(
        native: FunctionId,
        target: native_function_calls::NativeCallTarget,
    ) -> Self {
        Self { native, target }
    }

    #[must_use]
    pub const fn native_id(&self) -> FunctionId {
        self.native
    }

    pub(crate) fn matches(&self, native: FunctionId) -> bool {
        self.native == native
    }

    pub(crate) fn target(&self) -> native_function_calls::NativeCallTarget {
        self.target.clone()
    }
}

impl fmt::Debug for NativeInlineCacheEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeInlineCacheEntry")
            .field("native", &self.native)
            .field("target", &self.target.kind())
            .finish()
    }
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
    CallbackValue {
        method_id: MethodId,
        callback_method: CallbackMethodInlineCacheEntry,
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
    Char,
    Range,
    Array,
    Map,
    Set,
    Iterator,
    Option,
    Result,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CallbackMethodInlineCacheEntry {
    pub receiver: StandardMethodReceiver,
    pub target: CallbackMethodInlineCacheTarget,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallbackMethodInlineCacheTarget {
    Next,
    Map,
    MapErr,
    AndThen,
    OrElse,
    Filter,
    Find,
    Any,
    All,
    Count,
    Sum,
    GroupBy,
    SortBy,
    MapValues,
    CollectArray,
    CollectSet,
    CollectMap,
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
    Split,
    SplitOnce,
    SplitLines,
    SplitWhitespace,
    ParseI8,
    ParseI16,
    ParseI32,
    ParseI64,
    ParseU8,
    ParseU16,
    ParseU32,
    ParseU64,
    ParseF32,
    ParseF64,
    ParseBool,
    ParseChar,
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
    Add,
    Set,
    Remove,
    Extend,
    Keys,
    Values,
    Entries,
    Merge,
    Union,
    Intersection,
    Difference,
    SymmetricDifference,
    Slice,
    Push,
    Pop,
    Insert,
    RemoveAt,
    Clear,
    Reverse,
    Distinct,
    Join,
    Sort,
    Min,
    Max,
    Sum,
    Repeat,
    Replace,
    ToHex,
    ReadU32Le,
    ReadU32Be,
    ToString,
    IsWhitespace,
    IsAscii,
    IsAsciiDigit,
    IsSome,
    IsNone,
    IsOk,
    IsErr,
    UnwrapOr,
    OkOr,
    ToOption,
    ToErrorOption,
    Flatten,
    Iter,
    Chars,
    Bytes,
    Next,
    Count,
    Take,
    Skip,
    CollectArray,
}

pub struct LinkedRuntimeCodeCall<'program, 'args, 'host, 'heap, 'roots, 'budget, 'caches> {
    pub artifact: &'program Arc<LinkedArtifact>,
    pub function: ScriptFunctionHandle,
    pub args: &'args [Value],
    pub host: &'host mut HostExecution<'host>,
    pub persistent: PersistentHeapExecution<'heap, 'roots>,
    pub budget: &'budget mut ExecutionBudget,
    pub inline_caches: Option<&'caches dyn VmInlineCaches>,
    pub bytecode_profiler: Option<&'caches dyn VmBytecodeProfiler>,
}

pub struct LinkedProgramHostCall<'program, 'entry, 'args, 'host, 'heap, 'roots, 'budget, 'caches> {
    pub artifact: &'program Arc<LinkedArtifact>,
    pub entry: &'entry str,
    pub args: &'args [OwnedValue],
    pub host: &'host mut HostExecution<'host>,
    pub persistent: PersistentHeapExecution<'heap, 'roots>,
    pub budget: &'budget mut ExecutionBudget,
    pub inline_caches: Option<&'caches dyn VmInlineCaches>,
    pub bytecode_profiler: Option<&'caches dyn VmBytecodeProfiler>,
}

pub struct LinkedProgramHostBudgetCall<'program, 'entry, 'args, 'host, 'budget, 'caches> {
    pub artifact: &'program Arc<LinkedArtifact>,
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

    pub fn register_async_native(
        &mut self,
        name: impl Into<String>,
        function: impl for<'call> Fn(&'call [OwnedValue]) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) {
        let name = name.into();
        self.register_async_native_with_id(function_id_for_native_name(&name), function);
    }

    pub fn register_async_native_with_id(
        &mut self,
        id: FunctionId,
        function: impl for<'call> Fn(&'call [OwnedValue]) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) {
        self.async_native_ids.insert(id, Arc::new(function));
    }

    pub fn register_async_host_native_with_id(
        &mut self,
        id: FunctionId,
        function: impl for<'call, 'host, 'budget> Fn(
            &'call [OwnedValue],
            &'call mut HostExecution<'host>,
            Option<&'call mut ExecutionBudget>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) {
        self.async_host_native_ids.insert(id, Arc::new(function));
    }

    pub fn register_async_host_method_with_id(
        &mut self,
        id: HostMethodId,
        function: impl for<'call, 'host, 'budget> Fn(
            &'call vela_host::path::HostPath,
            &'call [OwnedValue],
            &'call mut HostExecution<'host>,
            Option<&'call mut ExecutionBudget>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) {
        self.async_host_method_ids.insert(id, Arc::new(function));
    }

    pub fn register_async_direct_host_method_with_id(
        &mut self,
        id: HostMethodId,
        lease_kind: vela_host::lease::HostLeaseKind,
        function: impl for<'host> Fn(
            vela_host::path::HostRef,
            vela_host::lease::ErasedHostLease<'host>,
            Vec<OwnedValue>,
        ) -> NativeCallFuture<'host>
        + Send
        + Sync
        + 'static,
    ) {
        self.async_direct_host_method_ids
            .insert(id, (lease_kind, Arc::new(function)));
    }

    pub(crate) fn register_conditional_host_native_with_id(
        &mut self,
        id: FunctionId,
        function: impl for<'host, 'budget> Fn(
            &[OwnedValue],
            &mut HostExecution<'host>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<ConditionalHostNativeOutcome>
        + Send
        + Sync
        + 'static,
    ) {
        self.conditional_host_native_ids
            .insert(id, Arc::new(function));
    }

    pub fn register_borrowed_native(
        &mut self,
        name: impl Into<String>,
        function: impl for<'heap, 'budget> Fn(
            &[Value],
            &HeapExecution<'heap>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) {
        let name = name.into();
        self.register_borrowed_native_with_id(function_id_for_native_name(&name), function);
    }

    pub fn register_borrowed_native_with_id(
        &mut self,
        id: FunctionId,
        function: impl for<'heap, 'budget> Fn(
            &[Value],
            &HeapExecution<'heap>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) {
        self.borrowed_native_ids.insert(id, Arc::new(function));
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
            .chain(self.async_native_ids.keys())
            .chain(self.async_host_native_ids.keys())
            .chain(self.conditional_host_native_ids.keys())
            .chain(self.borrowed_native_ids.keys())
            .chain(self.host_native_ids.keys())
            .chain(self.borrowed_host_native_ids.keys())
            .copied()
    }

    pub fn run_linked_program(
        &self,
        artifact: &Arc<LinkedArtifact>,
        entry: &str,
        args: &[OwnedValue],
    ) -> VmResult<OwnedValue> {
        let mut budget = ExecutionBudget::unbounded();
        self.run_linked_program_with_budget(artifact, entry, args, &mut budget)
    }

    pub fn run_linked_program_with_budget(
        &self,
        artifact: &Arc<LinkedArtifact>,
        entry: &str,
        args: &[OwnedValue],
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let function = linked_program_entry(artifact.program(), entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(args, &mut heap_execution, Some(budget))?;
        let result = self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                owner: Arc::clone(artifact),
                function,
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
        artifact: &Arc<LinkedArtifact>,
        entry: &str,
        args: &[Value],
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let function = linked_program_entry(artifact.program(), entry)?;
        self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                owner: Arc::clone(artifact),
                function,
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
        artifact: &Arc<LinkedArtifact>,
        entry: &str,
        args: &[OwnedValue],
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
        inline_caches: Option<&dyn VmInlineCaches>,
    ) -> VmResult<OwnedValue> {
        let function = linked_program_entry(artifact.program(), entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(args, &mut heap_execution, Some(budget))?;
        let result = self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                owner: Arc::clone(artifact),
                function,
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
        let function = linked_program_entry(call.artifact.program(), call.entry)?;
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let args = owned_args_to_runtime(call.args, &mut heap_execution, Some(call.budget))?;
        let result = self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                owner: Arc::clone(call.artifact),
                function,
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
        let function = linked_program_entry(call.artifact.program(), call.entry)?;
        let mut heap_execution = HeapExecution::new(call.persistent.heap);
        let args = owned_args_to_runtime(call.args, &mut heap_execution, Some(call.budget))?;
        heap_execution.protect_values(call.persistent.roots);
        let result = self.execute_linked_call(
            linked_execution::LinkedExecutionCall {
                owner: Arc::clone(call.artifact),
                function,
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
                owner: Arc::clone(call.artifact),
                function: call.function,
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

fn linked_program_entry(program: &LinkedProgram, entry: &str) -> VmResult<ScriptFunctionHandle> {
    let function = program.entry_point_by_name(entry).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: entry.to_owned(),
        })
    })?;
    program.function(function).map(|_| function).ok_or_else(|| {
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
