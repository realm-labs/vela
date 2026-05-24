//! Register VM for Vela bytecode.

mod array_methods;
pub mod heap;
mod host_values;
mod indexing;
mod iteration;
mod map_methods;
mod ranges;
mod record_fields;
mod reflection;
mod script_methods;
mod script_object;
mod set_methods;
mod stdlib;
mod try_propagation;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

use heap::{GcBudget, GcRef, GcStepStats, HeapSlot, HeapValue, ScriptHeap};
use host_values::{value_from_host, value_to_host};
pub use iteration::IteratorState;
pub use ranges::RangeValue;
use script_methods::{call_method, call_method_id};
use script_object::ScriptFields;
use try_propagation::{TryPropagation, try_propagate_value};
use vela_bytecode::{
    CallArgument, CodeObject, Constant, HostPathSegment, InstructionKind, Program, Register,
};
use vela_common::{Span, SymbolInterner};
use vela_host::{HostError, HostErrorKind, HostPath, HostRef, PatchTx, ScriptStateAdapter};
use vela_reflect::{self as reflect, ReflectError, ReflectErrorKind, TypeRegistry};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Missing,
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Set(Vec<Value>),
    Record {
        type_name: String,
        fields: ScriptFields<Value>,
    },
    Enum {
        enum_name: String,
        variant: String,
        fields: ScriptFields<Value>,
    },
    Closure(ClosureValue),
    Range(RangeValue),
    HeapRef(GcRef),
    HostRef(HostRef),
    Iterator(IteratorState),
}

impl Value {
    pub fn trace_heap_refs(&self, refs: &mut Vec<GcRef>) {
        match self {
            Self::HeapRef(reference) => refs.push(*reference),
            Self::Array(values) => values.iter().for_each(|value| value.trace_heap_refs(refs)),
            Self::Set(values) => values.iter().for_each(|value| value.trace_heap_refs(refs)),
            Self::Map(values) => values
                .values()
                .for_each(|value| value.trace_heap_refs(refs)),
            Self::Record { fields, .. } | Self::Enum { fields, .. } => {
                fields
                    .values()
                    .for_each(|value| value.trace_heap_refs(refs));
            }
            Self::Closure(closure) => closure
                .captures
                .iter()
                .for_each(|value| value.trace_heap_refs(refs)),
            Self::Iterator(iterator) => iterator.trace_heap_refs(refs),
            Self::Null
            | Self::Missing
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::Range(_)
            | Self::HostRef(_) => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClosureValue {
    code: Arc<CodeObject>,
    captures: Vec<Value>,
}

struct ExecutionCall<'a> {
    code: &'a CodeObject,
    program: Option<&'a Program>,
    captures: &'a [Value],
    args: &'a [Value],
}

impl From<&Constant> for Value {
    fn from(value: &Constant) -> Self {
        match value {
            Constant::Null => Self::Null,
            Constant::Bool(value) => Self::Bool(*value),
            Constant::Int(value) => Self::Int(*value),
            Constant::Float(value) => Self::Float(*value),
            Constant::String(value) => Self::String(value.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VmError {
    pub kind: VmErrorKind,
    pub source_span: Option<Span>,
}

impl VmError {
    fn new(kind: VmErrorKind) -> Self {
        Self {
            kind,
            source_span: None,
        }
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for VmError {}

#[derive(Clone, Debug, PartialEq)]
pub enum VmErrorKind {
    RegisterOutOfBounds {
        register: Register,
    },
    ConstantOutOfBounds {
        constant: usize,
    },
    InstructionOutOfBounds {
        offset: usize,
    },
    TypeMismatch {
        operation: &'static str,
    },
    DivisionByZero,
    UnknownNative {
        name: String,
    },
    PermissionDenied {
        native: String,
        permission: String,
    },
    UnknownFunction {
        name: String,
    },
    UnknownMethod {
        method: String,
    },
    ArityMismatch {
        name: String,
        expected: usize,
        actual: usize,
    },
    Host(HostErrorKind),
    Reflect(ReflectErrorKind),
    UnknownRecordField {
        type_name: String,
        field: String,
    },
    UnknownEnumField {
        enum_name: String,
        variant: String,
        field: String,
    },
    IndexOutOfBounds {
        index: i64,
        len: usize,
    },
    UnknownMapKey {
        key: String,
    },
    BudgetExceeded {
        budget: ExecutionBudgetKind,
        limit: u64,
    },
    MissingReturn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionBudgetKind {
    Instructions,
    MemoryBytes,
    CallDepth,
    Patches,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBudget {
    pub instruction_limit: u64,
    pub memory_limit_bytes: usize,
    pub max_call_depth: usize,
    pub max_patches: usize,
    instructions_executed: u64,
    memory_bytes_allocated: usize,
    current_call_depth: usize,
}

impl ExecutionBudget {
    #[must_use]
    pub fn new(
        instruction_limit: u64,
        memory_limit_bytes: usize,
        max_call_depth: usize,
        max_patches: usize,
    ) -> Self {
        Self {
            instruction_limit,
            memory_limit_bytes,
            max_call_depth,
            max_patches,
            instructions_executed: 0,
            memory_bytes_allocated: 0,
            current_call_depth: 0,
        }
    }

    #[must_use]
    pub fn unbounded() -> Self {
        Self::new(u64::MAX, usize::MAX, usize::MAX, usize::MAX)
    }

    #[must_use]
    pub fn instructions_executed(&self) -> u64 {
        self.instructions_executed
    }

    #[must_use]
    pub fn memory_bytes_allocated(&self) -> usize {
        self.memory_bytes_allocated
    }

    #[must_use]
    pub fn current_call_depth(&self) -> usize {
        self.current_call_depth
    }

    fn charge_instruction(&mut self) -> VmResult<()> {
        if self.instructions_executed >= self.instruction_limit {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::Instructions,
                limit: self.instruction_limit,
            }));
        }
        self.instructions_executed = self.instructions_executed.saturating_add(1);
        Ok(())
    }

    pub(crate) fn charge_memory(&mut self, bytes: usize) -> VmResult<()> {
        let next = self.memory_bytes_allocated.saturating_add(bytes);
        if next > self.memory_limit_bytes {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::MemoryBytes,
                limit: u64::try_from(self.memory_limit_bytes).unwrap_or(u64::MAX),
            }));
        }
        self.memory_bytes_allocated = next;
        Ok(())
    }

    pub(crate) fn release_memory(&mut self, bytes: usize) {
        self.memory_bytes_allocated = self.memory_bytes_allocated.saturating_sub(bytes);
    }

    fn enter_call(&mut self) -> VmResult<()> {
        if self.current_call_depth >= self.max_call_depth {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::CallDepth,
                limit: u64::try_from(self.max_call_depth).unwrap_or(u64::MAX),
            }));
        }
        self.current_call_depth = self.current_call_depth.saturating_add(1);
        Ok(())
    }

    fn exit_call(&mut self) {
        self.current_call_depth = self.current_call_depth.saturating_sub(1);
    }

    fn check_patch_count(&self, patch_count: usize) -> VmResult<()> {
        if patch_count > self.max_patches {
            Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::Patches,
                limit: u64::try_from(self.max_patches).unwrap_or(u64::MAX),
            }))
        } else {
            Ok(())
        }
    }

    fn reserve_patch(&self, current_patch_count: usize) -> VmResult<()> {
        self.check_patch_count(current_patch_count.saturating_add(1))
    }
}

pub type VmResult<T> = Result<T, VmError>;

impl From<HostError> for VmError {
    fn from(value: HostError) -> Self {
        Self {
            kind: VmErrorKind::Host(value.kind),
            source_span: value.source_span,
        }
    }
}

impl From<ReflectError> for VmError {
    fn from(value: ReflectError) -> Self {
        Self::new(VmErrorKind::Reflect(value.kind))
    }
}

pub type NativeFunction = Arc<dyn Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static>;
pub type HostNativeFunction = Arc<
    dyn for<'host> Fn(&[Value], &mut HostExecution<'host>) -> VmResult<Value>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone, Default)]
pub struct Vm {
    natives: HashMap<String, NativeFunction>,
    host_natives: HashMap<String, HostNativeFunction>,
    type_registry: Option<Arc<TypeRegistry>>,
    host_path_symbols: RefCell<SymbolInterner>,
}

pub struct HostExecution<'host> {
    pub adapter: &'host mut dyn ScriptStateAdapter,
    pub tx: &'host mut PatchTx,
}

pub struct HeapExecution<'heap> {
    pub heap: &'heap mut ScriptHeap,
    protected_roots: Vec<GcRef>,
    safe_point_gc_budget: GcBudget,
    gc_in_progress: bool,
    last_gc_step: Option<GcStepStats>,
}

impl<'heap> HeapExecution<'heap> {
    #[must_use]
    pub fn new(heap: &'heap mut ScriptHeap) -> Self {
        let max_pause_micros = heap.gc_config().max_pause_micros;
        Self {
            heap,
            protected_roots: Vec::new(),
            safe_point_gc_budget: GcBudget::micros(max_pause_micros),
            gc_in_progress: false,
            last_gc_step: None,
        }
    }

    #[must_use]
    pub fn with_safe_point_gc_budget(mut self, budget: GcBudget) -> Self {
        self.safe_point_gc_budget = budget;
        self
    }

    #[must_use]
    pub fn last_gc_step(&self) -> Option<&GcStepStats> {
        self.last_gc_step.as_ref()
    }

    fn push_protected_roots(&mut self, roots: Vec<GcRef>) -> usize {
        let previous_len = self.protected_roots.len();
        self.protected_roots.extend(roots);
        previous_len
    }

    fn truncate_protected_roots(&mut self, len: usize) {
        self.protected_roots.truncate(len);
    }

    fn collect_at_safe_point(
        &mut self,
        frame_roots: Vec<GcRef>,
        budget: Option<&mut ExecutionBudget>,
    ) {
        if !self.gc_in_progress && !self.heap.should_collect() {
            return;
        }

        let mut roots = self.protected_roots.clone();
        roots.extend(frame_roots);
        let stats = self
            .heap
            .step_gc_with_budget(&roots, self.safe_point_gc_budget, budget);
        self.gc_in_progress = !stats.complete;
        self.last_gc_step = Some(stats);
    }
}

impl Vm {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_native(
        &mut self,
        name: impl Into<String>,
        function: impl Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static,
    ) {
        self.natives.insert(name.into(), Arc::new(function));
    }

    pub fn register_host_native(
        &mut self,
        name: impl Into<String>,
        function: impl for<'host> Fn(&[Value], &mut HostExecution<'host>) -> VmResult<Value>
        + Send
        + Sync
        + 'static,
    ) {
        self.host_natives.insert(name.into(), Arc::new(function));
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

    pub fn run(&self, code: &CodeObject) -> VmResult<Value> {
        self.execute(code, None, &[], None, None, None)
    }

    pub fn run_with_budget(
        &self,
        code: &CodeObject,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        self.execute(code, None, &[], None, None, Some(budget))
    }

    pub fn run_with_heap_and_budget(
        &self,
        code: &CodeObject,
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        self.execute(code, None, &[], None, Some(heap), Some(budget))
    }

    pub fn run_with_managed_heap_and_budget(
        &self,
        code: &CodeObject,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        self.execute_with_managed_heap_and_budget(code, None, &[], None, budget)
    }

    pub fn run_program(&self, program: &Program, entry: &str, args: &[Value]) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute(code, Some(program), args, None, None, None)
    }

    pub fn run_program_with_budget(
        &self,
        program: &Program,
        entry: &str,
        args: &[Value],
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute(code, Some(program), args, None, None, Some(budget))
    }

    pub fn run_program_with_heap_and_budget(
        &self,
        program: &Program,
        entry: &str,
        args: &[Value],
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute(code, Some(program), args, None, Some(heap), Some(budget))
    }

    pub fn run_program_with_managed_heap_and_budget(
        &self,
        program: &Program,
        entry: &str,
        args: &[Value],
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute_with_managed_heap_and_budget(code, Some(program), args, None, budget)
    }

    pub fn run_with_host(
        &self,
        code: &CodeObject,
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        self.execute(code, None, &[], Some(host), None, None)
    }

    pub fn run_with_host_and_budget(
        &self,
        code: &CodeObject,
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        self.execute(code, None, &[], Some(host), None, Some(budget))
    }

    pub fn run_with_host_heap_and_budget(
        &self,
        code: &CodeObject,
        host: &mut HostExecution<'_>,
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        self.execute(code, None, &[], Some(host), Some(heap), Some(budget))
    }

    pub fn run_with_host_managed_heap_and_budget(
        &self,
        code: &CodeObject,
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        self.execute_with_managed_heap_and_budget(code, None, &[], Some(host), budget)
    }

    pub fn run_program_with_host(
        &self,
        program: &Program,
        entry: &str,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute(code, Some(program), args, Some(host), None, None)
    }

    pub fn run_program_with_host_and_budget(
        &self,
        program: &Program,
        entry: &str,
        args: &[Value],
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute(code, Some(program), args, Some(host), None, Some(budget))
    }

    pub fn run_program_with_host_heap_and_budget(
        &self,
        program: &Program,
        entry: &str,
        args: &[Value],
        host: &mut HostExecution<'_>,
        heap: &mut HeapExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute(
            code,
            Some(program),
            args,
            Some(host),
            Some(heap),
            Some(budget),
        )
    }

    pub fn run_program_with_host_managed_heap_and_budget(
        &self,
        program: &Program,
        entry: &str,
        args: &[Value],
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute_with_managed_heap_and_budget(code, Some(program), args, Some(host), budget)
    }

    fn execute_with_managed_heap_and_budget(
        &self,
        code: &CodeObject,
        program: Option<&Program>,
        args: &[Value],
        host: Option<&mut HostExecution<'_>>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let result = self.execute(
            code,
            program,
            args,
            host,
            Some(&mut heap_execution),
            Some(budget),
        );
        finish_managed_heap_result(result, &mut heap_execution, budget)
    }

    fn execute(
        &self,
        code: &CodeObject,
        program: Option<&Program>,
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
            },
            host,
            heap,
            budget,
        )
    }

    fn execute_call(
        &self,
        call: ExecutionCall<'_>,
        host: Option<&mut HostExecution<'_>>,
        heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        if let Some(budget) = &mut budget {
            budget.enter_call()?;
        }
        let result = self.execute_body(call, host, heap, budget.as_deref_mut());
        if let Some(budget) = budget {
            budget.exit_call();
        }
        result
    }

    pub(crate) fn execute_closure_value(
        &self,
        closure: &ClosureValue,
        program: Option<&Program>,
        args: &[Value],
        host: Option<&mut HostExecution<'_>>,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        self.execute_call(
            ExecutionCall {
                code: &closure.code,
                program,
                captures: &closure.captures,
                args,
            },
            host,
            heap,
            budget,
        )
    }

    pub(crate) fn execute_code_object(
        &self,
        code: &CodeObject,
        program: Option<&Program>,
        args: &[Value],
        host: Option<&mut HostExecution<'_>>,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        self.execute(code, program, args, host, heap, budget)
    }

    fn execute_body(
        &self,
        call: ExecutionCall<'_>,
        mut host: Option<&mut HostExecution<'_>>,
        mut heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        let code = call.code;
        let program = call.program;
        let captures = call.captures;
        let args = call.args;
        if captures.len() != usize::from(code.capture_count) {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: code.name.clone(),
                expected: usize::from(code.capture_count),
                actual: captures.len(),
            }));
        }
        if args.len() > code.params.len() {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: code.name.clone(),
                expected: code.params.len(),
                actual: args.len(),
            }));
        }

        let mut frame = CallFrame::new(code.register_count);
        for (index, capture) in captures.iter().enumerate() {
            frame.write(
                Register(u16::try_from(index).map_err(|_| {
                    VmError::new(VmErrorKind::RegisterOutOfBounds {
                        register: Register(u16::MAX),
                    })
                })?),
                capture.clone(),
            )?;
        }
        let param_offset = usize::from(code.capture_count);
        for (index, arg) in args.iter().enumerate() {
            frame.write(
                Register(
                    u16::try_from(param_offset.saturating_add(index)).map_err(|_| {
                        VmError::new(VmErrorKind::RegisterOutOfBounds {
                            register: Register(u16::MAX),
                        })
                    })?,
                ),
                arg.clone(),
            )?;
        }
        for index in args.len()..code.params.len() {
            frame.write(
                Register(
                    u16::try_from(param_offset.saturating_add(index)).map_err(|_| {
                        VmError::new(VmErrorKind::RegisterOutOfBounds {
                            register: Register(u16::MAX),
                        })
                    })?,
                ),
                Value::Missing,
            )?;
        }
        let defaults = normalized_param_defaults(code);
        let actual = args
            .iter()
            .filter(|arg| !matches!(arg, Value::Missing))
            .count();
        for (index, has_default) in defaults.iter().enumerate() {
            let register = Register(u16::try_from(param_offset.saturating_add(index)).map_err(
                |_| {
                    VmError::new(VmErrorKind::RegisterOutOfBounds {
                        register: Register(u16::MAX),
                    })
                },
            )?);
            if !has_default && matches!(frame.read(register)?, Value::Missing) {
                return Err(VmError::new(VmErrorKind::ArityMismatch {
                    name: code.name.clone(),
                    expected: code.params.len(),
                    actual,
                }));
            }
        }
        let mut ip = 0_usize;

        while ip < code.instructions.len() {
            let instruction = &code.instructions[ip];
            if let Some(budget) = budget.as_deref_mut() {
                budget.charge_instruction()?;
            }
            ip = ip.saturating_add(1);

            match &instruction.kind {
                InstructionKind::LoadConst { dst, constant } => {
                    let constant_value = code.constants.get(constant.0).ok_or(VmError {
                        kind: VmErrorKind::ConstantOutOfBounds {
                            constant: constant.0,
                        },
                        source_span: instruction.span,
                    })?;
                    let value = value_from_constant(
                        constant_value,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Move { dst, src } => {
                    let value = frame.read(*src)?.clone();
                    frame.write(*dst, value)?;
                }
                InstructionKind::Not { dst, src } => {
                    let value = Value::Bool(!is_truthy(frame.read(*src)?));
                    frame.write(*dst, value)?;
                }
                InstructionKind::Negate { dst, src } => {
                    let value = negate_numeric(frame.read(*src)?)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Add { dst, lhs, rhs } => {
                    let value =
                        binary_numeric(frame.read(*lhs)?, frame.read(*rhs)?, "add", |a, b| a + b)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Sub { dst, lhs, rhs } => {
                    let value =
                        binary_numeric(frame.read(*lhs)?, frame.read(*rhs)?, "sub", |a, b| a - b)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Mul { dst, lhs, rhs } => {
                    let value =
                        binary_numeric(frame.read(*lhs)?, frame.read(*rhs)?, "mul", |a, b| a * b)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Div { dst, lhs, rhs } => {
                    let value = div_numeric(frame.read(*lhs)?, frame.read(*rhs)?)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Rem { dst, lhs, rhs } => {
                    let value = rem_numeric(frame.read(*lhs)?, frame.read(*rhs)?)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Equal { dst, lhs, rhs } => {
                    let value = Value::Bool(values_equal(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                InstructionKind::NotEqual { dst, lhs, rhs } => {
                    let value = Value::Bool(!values_equal(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                InstructionKind::Less { dst, lhs, rhs } => {
                    let value =
                        compare_numeric(frame.read(*lhs)?, frame.read(*rhs)?, "less", |a, b| {
                            a < b
                        })?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::LessEqual { dst, lhs, rhs } => {
                    let value = compare_numeric(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        "less_equal",
                        |a, b| a <= b,
                    )?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::Greater { dst, lhs, rhs } => {
                    let value = compare_numeric(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        "greater",
                        |a, b| a > b,
                    )?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::GreaterEqual { dst, lhs, rhs } => {
                    let value = compare_numeric(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        "greater_equal",
                        |a, b| a >= b,
                    )?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::JumpIfFalse { condition, target } => {
                    if !is_truthy(frame.read(*condition)?) {
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                InstructionKind::JumpIfNotMissing { value, target } => {
                    if !matches!(frame.read(*value)?, Value::Missing) {
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                InstructionKind::Jump { target } => {
                    validate_jump(code, target.0)?;
                    ip = target.0;
                }
                InstructionKind::CallNative { dst, name, args } => {
                    let values = args
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    let values = materialize_values(&values, heap.as_deref())?;
                    let result = if let Some(native) = self.natives.get(name) {
                        native(&values)?
                    } else if let Some(native) = self.host_natives.get(name) {
                        let host = host.as_deref_mut().ok_or_else(|| {
                            VmError::new(VmErrorKind::TypeMismatch {
                                operation: "host context",
                            })
                        })?;
                        native(&values, host)?
                    } else {
                        return Err(VmError::new(VmErrorKind::UnknownNative {
                            name: name.clone(),
                        }));
                    };
                    if let (Some(budget), Some(host)) = (budget.as_deref(), host.as_deref()) {
                        budget.check_patch_count(host.tx.patches().len())?;
                    }
                    if let Some(dst) = dst {
                        let result = store_value_in_heap_if_needed(
                            result,
                            heap.as_deref_mut(),
                            budget.as_deref_mut(),
                        )?;
                        frame.write(*dst, result)?;
                    }
                }
                InstructionKind::CallFunction { dst, name, args } => {
                    let program = program.ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction { name: name.clone() })
                    })?;
                    let function = program.function(name).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction { name: name.clone() })
                    })?;
                    let values = args
                        .iter()
                        .map(|arg| match arg {
                            CallArgument::Register(register) => frame.read(*register).cloned(),
                            CallArgument::Missing => Ok(Value::Missing),
                        })
                        .collect::<VmResult<Vec<_>>>()?;
                    let protected_root_len = heap
                        .as_deref_mut()
                        .map(|heap| heap.push_protected_roots(frame.heap_roots()));
                    let result = self.execute(
                        function,
                        Some(program),
                        &values,
                        host.as_deref_mut(),
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    );
                    if let (Some(heap), Some(protected_root_len)) =
                        (heap.as_deref_mut(), protected_root_len)
                    {
                        heap.truncate_protected_roots(protected_root_len);
                    }
                    let result = result?;
                    frame.write(*dst, result)?;
                }
                InstructionKind::MakeClosure {
                    dst,
                    code,
                    captures,
                } => {
                    let captures = captures
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    frame.write(
                        *dst,
                        Value::Closure(ClosureValue {
                            code: Arc::new((**code).clone()),
                            captures,
                        }),
                    )?;
                }
                InstructionKind::CallClosure { dst, callee, args } => {
                    let closure = expect_closure(frame.read(*callee)?, "closure call")?;
                    let values = args
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    let protected_root_len = heap
                        .as_deref_mut()
                        .map(|heap| heap.push_protected_roots(frame.heap_roots()));
                    let result = self.execute_call(
                        ExecutionCall {
                            code: &closure.code,
                            program,
                            captures: &closure.captures,
                            args: &values,
                        },
                        host.as_deref_mut(),
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    );
                    if let (Some(heap), Some(protected_root_len)) =
                        (heap.as_deref_mut(), protected_root_len)
                    {
                        heap.truncate_protected_roots(protected_root_len);
                    }
                    let result = result?;
                    frame.write(*dst, result)?;
                }
                InstructionKind::CallMethod {
                    dst,
                    receiver,
                    method,
                    args,
                } => {
                    let values = args
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    let mut receiver_value = frame.read(*receiver)?.clone();
                    let result = call_method(
                        &mut receiver_value,
                        method,
                        &values,
                        self,
                        program,
                        host.as_deref_mut(),
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        frame.heap_roots(),
                    )?;
                    let result = store_value_in_heap_if_needed(
                        result,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*receiver, receiver_value)?;
                    frame.write(*dst, result)?;
                }
                InstructionKind::CallMethodId {
                    dst,
                    receiver,
                    method,
                    method_id,
                    args,
                } => {
                    let values = args
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    let receiver_value = frame.read(*receiver)?.clone();
                    let result = call_method_id(
                        &receiver_value,
                        method,
                        *method_id,
                        &values,
                        self,
                        program,
                        host.as_deref_mut(),
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        frame.heap_roots(),
                    )?;
                    let result = store_value_in_heap_if_needed(
                        result,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*dst, result)?;
                }
                InstructionKind::TryPropagate { dst, src } => {
                    match try_propagate_value(frame.read(*src)?, heap.as_deref())? {
                        TryPropagation::Continue(value) => frame.write(*dst, value)?,
                        TryPropagation::Return(value) => return Ok(value),
                    }
                }
                InstructionKind::MakeArray { dst, elements } => {
                    let values = elements
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    let value = if let Some(heap) = heap.as_deref_mut() {
                        let slots = values_to_heap_slots(&values, heap, budget.as_deref_mut())?;
                        allocate_heap_value(HeapValue::Array(slots), heap, budget.as_deref_mut())?
                    } else {
                        Value::Array(values)
                    };
                    frame.write(*dst, value)?;
                }
                InstructionKind::MakeMap { dst, entries } => {
                    let mut values = BTreeMap::new();
                    for (key, register) in entries {
                        values.insert(key.clone(), frame.read(*register)?.clone());
                    }
                    let value = if let Some(heap) = heap.as_deref_mut() {
                        let slots = values_to_heap_map(&values, heap, budget.as_deref_mut())?;
                        allocate_heap_value(HeapValue::Map(slots), heap, budget.as_deref_mut())?
                    } else {
                        Value::Map(values)
                    };
                    frame.write(*dst, value)?;
                }
                InstructionKind::MakeRange {
                    dst,
                    start,
                    end,
                    inclusive,
                } => {
                    let start = expect_int(frame.read(*start)?, "range")?;
                    let end = expect_int(frame.read(*end)?, "range")?;
                    frame.write(*dst, Value::Range(RangeValue::new(start, end, *inclusive)))?;
                }
                InstructionKind::MakeRecord {
                    dst,
                    type_name,
                    fields,
                } => {
                    let values = ScriptFields::from_pairs(
                        type_name,
                        fields
                            .iter()
                            .map(|(name, register)| {
                                Ok((name.clone(), frame.read(*register)?.clone()))
                            })
                            .collect::<VmResult<Vec<_>>>()?,
                    );
                    let value = if let Some(heap) = heap.as_deref_mut() {
                        let slots =
                            values_to_heap_fields(type_name, &values, heap, budget.as_deref_mut())?;
                        allocate_heap_value(
                            HeapValue::Record {
                                type_name: type_name.clone(),
                                fields: slots,
                            },
                            heap,
                            budget.as_deref_mut(),
                        )?
                    } else {
                        Value::Record {
                            type_name: type_name.clone(),
                            fields: values,
                        }
                    };
                    frame.write(*dst, value)?;
                }
                InstructionKind::MakeEnum {
                    dst,
                    enum_name,
                    variant,
                    fields,
                } => {
                    let owner = enum_variant_owner(enum_name, variant);
                    let values = ScriptFields::from_pairs(
                        &owner,
                        fields
                            .iter()
                            .map(|(name, register)| {
                                Ok((name.clone(), frame.read(*register)?.clone()))
                            })
                            .collect::<VmResult<Vec<_>>>()?,
                    );
                    let value = if let Some(heap) = heap.as_deref_mut() {
                        let slots =
                            values_to_heap_fields(&owner, &values, heap, budget.as_deref_mut())?;
                        allocate_heap_value(
                            HeapValue::Enum {
                                enum_name: enum_name.clone(),
                                variant: variant.clone(),
                                fields: slots,
                            },
                            heap,
                            budget.as_deref_mut(),
                        )?
                    } else {
                        Value::Enum {
                            enum_name: enum_name.clone(),
                            variant: variant.clone(),
                            fields: values,
                        }
                    };
                    frame.write(*dst, value)?;
                }
                InstructionKind::GetRecordField { dst, record, field } => {
                    let value =
                        get_record_field_value(frame.read(*record)?, field, heap.as_deref())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::GetRecordSlot {
                    dst,
                    record,
                    field,
                    slot,
                } => {
                    let value =
                        get_record_slot_value(frame.read(*record)?, field, *slot, heap.as_deref())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::SetRecordField { record, field, src } => {
                    let mut record_value = frame.read(*record)?.clone();
                    record_fields::set_record_field_value(
                        &mut record_value,
                        field,
                        frame.read(*src)?,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*record, record_value)?;
                }
                InstructionKind::SetRecordSlot {
                    record,
                    field,
                    slot,
                    src,
                } => {
                    let mut record_value = frame.read(*record)?.clone();
                    record_fields::set_record_slot_value(
                        &mut record_value,
                        field,
                        *slot,
                        frame.read(*src)?,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*record, record_value)?;
                }
                InstructionKind::GetEnumField { dst, value, field } => {
                    let value = get_enum_field_value(frame.read(*value)?, field, heap.as_deref())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::GetEnumSlot {
                    dst,
                    value,
                    field,
                    slot,
                } => {
                    let value =
                        get_enum_slot_value(frame.read(*value)?, field, *slot, heap.as_deref())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::GetIndex { dst, base, index } => {
                    let value = indexing::get_index(
                        frame.read(*base)?,
                        frame.read(*index)?,
                        heap.as_deref(),
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::SetIndex { base, index, src } => {
                    let mut base_value = frame.read(*base)?.clone();
                    indexing::set_index(
                        &mut base_value,
                        frame.read(*index)?,
                        frame.read(*src)?,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*base, base_value)?;
                }
                InstructionKind::IterInit { dst, iterable } => {
                    let iterator =
                        iteration::make_iterator(frame.read(*iterable)?, heap.as_deref())?;
                    frame.write(*dst, Value::Iterator(iterator))?;
                }
                InstructionKind::IterNext {
                    iterator,
                    dst,
                    jump_if_done,
                } => {
                    let value = frame.read(*iterator)?.clone();
                    let Value::Iterator(mut iterator_state) = value else {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "iterator",
                        }));
                    };
                    match iterator_state.next() {
                        Some(value) => {
                            frame.write(*iterator, Value::Iterator(iterator_state))?;
                            frame.write(*dst, value)?;
                        }
                        None => {
                            frame.write(*iterator, Value::Iterator(iterator_state))?;
                            validate_jump(code, jump_if_done.0)?;
                            ip = jump_if_done.0;
                        }
                    }
                }
                InstructionKind::EnumTagEqual {
                    dst,
                    value,
                    enum_name,
                    variant,
                } => {
                    let matches =
                        enum_tag_equal(frame.read(*value)?, enum_name, variant, heap.as_deref());
                    frame.write(*dst, Value::Bool(matches))?;
                }
                InstructionKind::GetHostField { dst, root, field } => {
                    let root = expect_host_ref(frame.read(*root)?, "get_host_field")?;
                    let path = HostPath::new(root).field(*field);
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    frame.write(*dst, value_from_host(value))?;
                }
                InstructionKind::GetHostPath {
                    dst,
                    root,
                    segments,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "get_host_path")?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    frame.write(*dst, value_from_host(value))?;
                }
                InstructionKind::SetHostField { root, field, src } => {
                    let root = expect_host_ref(frame.read(*root)?, "set_host_field")?;
                    let value =
                        value_to_host(frame.read(*src)?, "set_host_field", heap.as_deref())?;
                    let path = HostPath::new(root).field(*field);
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx.set_path(path, value, instruction.span)?;
                }
                InstructionKind::SetHostPath {
                    root,
                    segments,
                    src,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "set_host_path")?;
                    let value = value_to_host(frame.read(*src)?, "set_host_path", heap.as_deref())?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx.set_path(path, value, instruction.span)?;
                }
                InstructionKind::AddHostField { root, field, rhs } => {
                    let root = expect_host_ref(frame.read(*root)?, "add_host_field")?;
                    let value =
                        value_to_host(frame.read(*rhs)?, "add_host_field", heap.as_deref())?;
                    let path = HostPath::new(root).field(*field);
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let base_value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx
                        .add_path(path, value, base_value, instruction.span)?;
                }
                InstructionKind::SubHostField { root, field, rhs } => {
                    let root = expect_host_ref(frame.read(*root)?, "sub_host_field")?;
                    let value =
                        value_to_host(frame.read(*rhs)?, "sub_host_field", heap.as_deref())?;
                    let path = HostPath::new(root).field(*field);
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let base_value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx
                        .sub_path(path, value, base_value, instruction.span)?;
                }
                InstructionKind::AddHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "add_host_path")?;
                    let value = value_to_host(frame.read(*rhs)?, "add_host_path", heap.as_deref())?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let base_value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx
                        .add_path(path, value, base_value, instruction.span)?;
                }
                InstructionKind::SubHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "sub_host_path")?;
                    let value = value_to_host(frame.read(*rhs)?, "sub_host_path", heap.as_deref())?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let base_value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx
                        .sub_path(path, value, base_value, instruction.span)?;
                }
                InstructionKind::PushHostPath {
                    root,
                    segments,
                    value,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "push_host_path")?;
                    let value =
                        value_to_host(frame.read(*value)?, "push_host_path", heap.as_deref())?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let base_value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx
                        .push_path(path, value, base_value, instruction.span)?;
                }
                InstructionKind::RemoveHostPath { root, segments } => {
                    let root = expect_host_ref(frame.read(*root)?, "remove_host_path")?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx.remove_path(path, instruction.span)?;
                }
                InstructionKind::CallHostMethod {
                    dst,
                    root,
                    segments,
                    method,
                    args,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "call_host_method")?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let values = args
                        .iter()
                        .map(|register| {
                            value_to_host(
                                frame.read(*register)?,
                                "call_host_method",
                                heap.as_deref(),
                            )
                        })
                        .collect::<VmResult<Vec<_>>>()?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    let return_value = host
                        .adapter
                        .preview_method_return(&path, *method, &values)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    host.tx
                        .call_method(path, *method, values, instruction.span)?;
                    if let Some(dst) = dst {
                        frame.write(*dst, value_from_host(return_value))?;
                    }
                }
                InstructionKind::Return { src } => return Ok(frame.read(*src)?.clone()),
            }

            if let Some(heap) = heap.as_deref_mut() {
                heap.collect_at_safe_point(frame.heap_roots(), budget.as_deref_mut());
            }
        }

        Err(VmError::new(VmErrorKind::MissingReturn))
    }
}

#[derive(Clone, Debug)]
struct CallFrame {
    registers: Vec<Value>,
}

impl CallFrame {
    fn new(register_count: u16) -> Self {
        Self {
            registers: vec![Value::Null; usize::from(register_count)],
        }
    }

    fn read(&self, register: Register) -> VmResult<&Value> {
        self.registers
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))
    }

    fn write(&mut self, register: Register, value: Value) -> VmResult<()> {
        let slot = self
            .registers
            .get_mut(usize::from(register.0))
            .ok_or(VmError {
                kind: VmErrorKind::RegisterOutOfBounds { register },
                source_span: None,
            })?;
        *slot = value;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn heap_roots(&self) -> Vec<GcRef> {
        let mut roots = Vec::new();
        self.registers
            .iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        roots
    }
}

fn value_from_constant(
    constant: &Constant,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match (constant, heap) {
        (Constant::String(value), Some(heap)) => {
            allocate_heap_value(HeapValue::String(value.clone()), heap, budget)
        }
        _ => Ok(Value::from(constant)),
    }
}

fn normalized_param_defaults(code: &CodeObject) -> Vec<bool> {
    let mut defaults = code.param_defaults.clone();
    defaults.resize(code.params.len(), false);
    defaults
}

fn allocate_heap_value(
    value: HeapValue,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let reference = if let Some(budget) = budget {
        heap.heap.allocate_with_budget(value, budget)?
    } else {
        heap.heap.allocate(value)
    };
    Ok(Value::HeapRef(reference))
}

fn values_to_heap_slots(
    values: &[Value],
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Vec<HeapSlot>> {
    values
        .iter()
        .map(|value| value_to_heap_slot(value, heap, budget.as_deref_mut()))
        .collect()
}

fn values_to_heap_map(
    values: &BTreeMap<String, Value>,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<BTreeMap<String, HeapSlot>> {
    values
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value_to_heap_slot(value, heap, budget.as_deref_mut())?,
            ))
        })
        .collect()
}

fn values_to_heap_fields(
    owner: &str,
    values: &ScriptFields<Value>,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<ScriptFields<HeapSlot>> {
    values
        .iter()
        .map(|(key, value)| {
            Ok((
                key.to_owned(),
                value_to_heap_slot(value, heap, budget.as_deref_mut())?,
            ))
        })
        .collect::<VmResult<Vec<_>>>()
        .map(|fields| ScriptFields::from_pairs(owner, fields))
}

fn enum_variant_owner(enum_name: &str, variant: &str) -> String {
    format!("{enum_name}.{variant}")
}

pub(crate) fn value_to_heap_slot(
    value: &Value,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<HeapSlot> {
    match value {
        Value::Null => Ok(HeapSlot::Null),
        Value::Bool(value) => Ok(HeapSlot::Bool(*value)),
        Value::Int(value) => Ok(HeapSlot::Int(*value)),
        Value::Float(value) => Ok(HeapSlot::Float(*value)),
        Value::HeapRef(reference) => Ok(HeapSlot::Ref(*reference)),
        Value::HostRef(reference) => Ok(HeapSlot::HostRef(*reference)),
        Value::String(value) => {
            let Value::HeapRef(reference) =
                allocate_heap_value(HeapValue::String(value.clone()), heap, budget)?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Array(values) => {
            let slots = values_to_heap_slots(values, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) =
                allocate_heap_value(HeapValue::Array(slots), heap, budget)?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Set(values) => {
            let slots = values_to_heap_slots(values, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) =
                allocate_heap_value(HeapValue::Set(slots), heap, budget)?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Map(values) => {
            let slots = values_to_heap_map(values, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) =
                allocate_heap_value(HeapValue::Map(slots), heap, budget)?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Record { type_name, fields } => {
            let slots = values_to_heap_fields(type_name, fields, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) = allocate_heap_value(
                HeapValue::Record {
                    type_name: type_name.clone(),
                    fields: slots,
                },
                heap,
                budget,
            )?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let owner = enum_variant_owner(enum_name, variant);
            let slots = values_to_heap_fields(&owner, fields, heap, budget.as_deref_mut())?;
            let Value::HeapRef(reference) = allocate_heap_value(
                HeapValue::Enum {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    fields: slots,
                },
                heap,
                budget,
            )?
            else {
                unreachable!("heap allocation always returns a heap ref");
            };
            Ok(HeapSlot::Ref(reference))
        }
        Value::Range(_) | Value::Closure(_) | Value::Iterator(_) | Value::Missing => {
            Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "heap slot",
            }))
        }
    }
}

fn value_from_heap_slot(slot: &HeapSlot) -> Value {
    match slot {
        HeapSlot::Null => Value::Null,
        HeapSlot::Bool(value) => Value::Bool(*value),
        HeapSlot::Int(value) => Value::Int(*value),
        HeapSlot::Float(value) => Value::Float(*value),
        HeapSlot::Ref(reference) => Value::HeapRef(*reference),
        HeapSlot::HostRef(reference) => Value::HostRef(*reference),
    }
}

fn materialize_values(values: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<Vec<Value>> {
    values
        .iter()
        .map(|value| materialize_value(value, heap))
        .collect()
}

fn materialize_value(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    match value {
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "heap ref",
                }));
            };
            materialize_heap_value(heap_value, heap)
        }
        Value::Array(values) => Ok(Value::Array(materialize_values(values, heap)?)),
        Value::Set(values) => Ok(Value::Set(materialize_values(values, heap)?)),
        Value::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), materialize_value(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(Value::Map),
        Value::Record { type_name, fields } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), materialize_value(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| Value::Record {
                type_name: type_name.clone(),
                fields: ScriptFields::from_pairs(type_name, fields),
            }),
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), materialize_value(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| Value::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: ScriptFields::from_pairs(&enum_variant_owner(enum_name, variant), fields),
            }),
        Value::Closure(closure) => closure
            .captures
            .iter()
            .map(|capture| materialize_value(capture, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(|captures| {
                Value::Closure(ClosureValue {
                    code: Arc::clone(&closure.code),
                    captures,
                })
            }),
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::Range(_)
        | Value::HostRef(_) => Ok(value.clone()),
        Value::Iterator(_) | Value::Missing => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "materialize",
        })),
    }
}

fn materialize_heap_value(value: &HeapValue, heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    match value {
        HeapValue::String(value) => Ok(Value::String(value.clone())),
        HeapValue::Array(values) => values
            .iter()
            .map(|value| materialize_heap_slot(value, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(Value::Array),
        HeapValue::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), materialize_heap_slot(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(Value::Map),
        HeapValue::Record { type_name, fields } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), materialize_heap_slot(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| Value::Record {
                type_name: type_name.clone(),
                fields: ScriptFields::from_pairs(type_name, fields),
            }),
        HeapValue::Enum {
            enum_name,
            variant,
            fields,
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.to_owned(), materialize_heap_slot(value, heap)?)))
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| Value::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: ScriptFields::from_pairs(&enum_variant_owner(enum_name, variant), fields),
            }),
        HeapValue::Set(values) => values
            .iter()
            .map(|value| materialize_heap_slot(value, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(Value::Set),
    }
}

fn materialize_heap_slot(slot: &HeapSlot, heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    match slot {
        HeapSlot::Ref(reference) => materialize_value(&Value::HeapRef(*reference), heap),
        _ => Ok(value_from_heap_slot(slot)),
    }
}

fn values_equal(lhs: &Value, rhs: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
    let lhs = materialize_value(lhs, heap)?;
    let rhs = materialize_value(rhs, heap)?;
    Ok(lhs == rhs)
}

fn store_value_in_heap_if_needed(
    value: Value,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let Some(heap) = heap else {
        return Ok(value);
    };
    match value {
        Value::String(_)
        | Value::Array(_)
        | Value::Set(_)
        | Value::Map(_)
        | Value::Record { .. }
        | Value::Enum { .. } => {
            let slot = value_to_heap_slot(&value, heap, budget)?;
            Ok(value_from_heap_slot(&slot))
        }
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::HeapRef(_)
        | Value::HostRef(_)
        | Value::Range(_)
        | Value::Closure(_)
        | Value::Iterator(_) => Ok(value),
        Value::Missing => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "missing value",
        })),
    }
}

fn finish_managed_heap_result(
    result: VmResult<Value>,
    heap: &mut HeapExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<Value> {
    let result = result.and_then(|value| materialize_value(&value, Some(heap)));
    heap.heap.collect_full_with_budget(&[], Some(budget));
    result
}

fn get_record_field_value(
    value: &Value,
    field: &str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::Record { type_name, fields } => fields.get(field).cloned().ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownRecordField {
                type_name: type_name.clone(),
                field: field.to_owned(),
            })
        }),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Record { type_name, fields }) =
                heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "record field",
                }));
            };
            fields.get(field).map(value_from_heap_slot).ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownRecordField {
                    type_name: type_name.clone(),
                    field: field.to_owned(),
                })
            })
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "record field",
        })),
    }
}

fn get_record_slot_value(
    value: &Value,
    field: &str,
    slot: usize,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::Record { type_name, fields } => {
            fields.get_slot(slot, field).cloned().ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownRecordField {
                    type_name: type_name.clone(),
                    field: field.to_owned(),
                })
            })
        }
        Value::HeapRef(reference) => {
            let Some(HeapValue::Record { type_name, fields }) =
                heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "record slot",
                }));
            };
            fields
                .get_slot(slot, field)
                .map(value_from_heap_slot)
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::UnknownRecordField {
                        type_name: type_name.clone(),
                        field: field.to_owned(),
                    })
                })
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "record slot",
        })),
    }
}

fn get_enum_field_value(
    value: &Value,
    field: &str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => fields.get(field).cloned().ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownEnumField {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                field: field.to_owned(),
            })
        }),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Enum {
                enum_name,
                variant,
                fields,
            }) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "enum field",
                }));
            };
            fields.get(field).map(value_from_heap_slot).ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownEnumField {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    field: field.to_owned(),
                })
            })
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "enum field",
        })),
    }
}

fn get_enum_slot_value(
    value: &Value,
    field: &str,
    slot: usize,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    match value {
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => fields.get_slot(slot, field).cloned().ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownEnumField {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                field: field.to_owned(),
            })
        }),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Enum {
                enum_name,
                variant,
                fields,
            }) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "enum slot",
                }));
            };
            fields
                .get_slot(slot, field)
                .map(value_from_heap_slot)
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::UnknownEnumField {
                        enum_name: enum_name.clone(),
                        variant: variant.clone(),
                        field: field.to_owned(),
                    })
                })
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "enum slot",
        })),
    }
}

fn enum_tag_equal(
    value: &Value,
    enum_name: &str,
    variant: &str,
    heap: Option<&HeapExecution<'_>>,
) -> bool {
    match value {
        Value::Enum {
            enum_name: value_enum,
            variant: value_variant,
            ..
        } => value_enum == enum_name && value_variant == variant,
        Value::HeapRef(reference) => matches!(
            heap.and_then(|heap| heap.heap.get(*reference)),
            Some(HeapValue::Enum {
                enum_name: value_enum,
                variant: value_variant,
                ..
            }) if value_enum == enum_name && value_variant == variant
        ),
        _ => false,
    }
}

fn binary_numeric(
    lhs: &Value,
    rhs: &Value,
    operation: &'static str,
    int_op: impl FnOnce(i64, i64) -> i64,
) -> VmResult<Value> {
    match (lhs, rhs) {
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(int_op(*lhs, *rhs))),
        (Value::Float(lhs), Value::Float(rhs)) => {
            Ok(Value::Float(int_op_float(*lhs, *rhs, operation)?))
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn negate_numeric(value: &Value) -> VmResult<Value> {
    match value {
        Value::Int(value) => value.checked_neg().map(Value::Int).ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "negate",
            })
        }),
        Value::Float(value) => Ok(Value::Float(-value)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "negate",
        })),
    }
}

fn int_op_float(lhs: f64, rhs: f64, operation: &'static str) -> VmResult<f64> {
    match operation {
        "add" => Ok(lhs + rhs),
        "sub" => Ok(lhs - rhs),
        "mul" => Ok(lhs * rhs),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn div_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (Value::Int(_), Value::Int(0)) => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs / rhs)),
        (Value::Float(_), Value::Float(rhs)) if *rhs == 0.0 => {
            Err(VmError::new(VmErrorKind::DivisionByZero))
        }
        (Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs / rhs)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation: "div" })),
    }
}

fn rem_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (Value::Int(_), Value::Int(0)) => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs % rhs)),
        (Value::Float(_), Value::Float(rhs)) if *rhs == 0.0 => {
            Err(VmError::new(VmErrorKind::DivisionByZero))
        }
        (Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs % rhs)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation: "rem" })),
    }
}

fn expect_host_ref(value: &Value, operation: &'static str) -> VmResult<HostRef> {
    match value {
        Value::HostRef(value) => Ok(*value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn host_path_from_segments(
    root: HostRef,
    segments: &[HostPathSegment],
    frame: &CallFrame,
    heap: Option<&HeapExecution<'_>>,
    symbols: &mut SymbolInterner,
) -> VmResult<HostPath> {
    let mut path = HostPath::new(root);
    for segment in segments {
        path = match segment {
            HostPathSegment::Field(field) => path.field(*field),
            HostPathSegment::Value(register) => {
                match materialize_value(frame.read(*register)?, heap)? {
                    Value::Int(index) => {
                        let index = u32::try_from(index).map_err(|_| {
                            VmError::new(VmErrorKind::TypeMismatch {
                                operation: "host path index",
                            })
                        })?;
                        path.index(index)
                    }
                    Value::String(key) => path.key(symbols.intern(key)),
                    _ => {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host path index",
                        }));
                    }
                }
            }
        };
    }
    Ok(path)
}

fn expect_closure(value: &Value, operation: &'static str) -> VmResult<ClosureValue> {
    match value {
        Value::Closure(closure) => Ok(closure.clone()),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn value_to_reflect(value: &Value, operation: &'static str) -> VmResult<reflect::ReflectValue> {
    match value {
        Value::HostRef(host_ref) => Ok(reflect::ReflectValue::HostRef(*host_ref)),
        Value::Map(values) => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.clone(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::ReflectValue::Record(values))
        }
        Value::Record {
            type_name,
            fields: values,
        } => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.to_owned(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::ReflectValue::ScriptRecord {
                type_name: type_name.clone(),
                fields: values,
            })
        }
        Value::Enum {
            enum_name,
            variant,
            fields: values,
        } => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.to_owned(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::ReflectValue::ScriptEnum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: values,
            })
        }
        Value::Array(_) | Value::Set(_) | Value::Range(_) | Value::Closure(_) | Value::Missing => {
            Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
        }
        Value::HeapRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        Value::Iterator(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_) => Ok(
            reflect::ReflectValue::Host(value_to_host(value, operation, None)?),
        ),
    }
}

fn value_from_reflect(value: reflect::ReflectValue) -> VmResult<Value> {
    match value {
        reflect::ReflectValue::Host(value) => Ok(value_from_host(value)),
        reflect::ReflectValue::HostRef(host_ref) => Ok(Value::HostRef(host_ref)),
        reflect::ReflectValue::Record(values) => {
            let values = values
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(Value::Map(values))
        }
        reflect::ReflectValue::ScriptRecord { type_name, fields } => {
            let fields = fields
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(Value::Record {
                fields: ScriptFields::from_pairs(&type_name, fields),
                type_name,
            })
        }
        reflect::ReflectValue::ScriptEnum {
            enum_name,
            variant,
            fields,
        } => {
            let fields = fields
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(Value::Enum {
                fields: ScriptFields::from_pairs(&format!("{enum_name}.{variant}"), fields),
                enum_name,
                variant,
            })
        }
    }
}

fn expect_string<'a>(value: &'a Value, operation: &'static str) -> VmResult<&'a str> {
    match value {
        Value::String(value) => Ok(value),
        Value::Null
        | Value::Missing
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::Array(_)
        | Value::Set(_)
        | Value::Map(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::Range(_)
        | Value::Closure(_)
        | Value::HeapRef(_)
        | Value::Iterator(_)
        | Value::HostRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn expect_int(value: &Value, operation: &'static str) -> VmResult<i64> {
    match value {
        Value::Int(value) => Ok(*value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn expect_arity(name: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::ArityMismatch {
            name: name.to_owned(),
            expected,
            actual: args.len(),
        }))
    }
}

fn compare_numeric(
    lhs: &Value,
    rhs: &Value,
    operation: &'static str,
    compare: impl FnOnce(f64, f64) -> bool,
) -> VmResult<bool> {
    match (lhs, rhs) {
        (Value::Int(lhs), Value::Int(rhs)) => Ok(compare(*lhs as f64, *rhs as f64)),
        (Value::Float(lhs), Value::Float(rhs)) => Ok(compare(*lhs, *rhs)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Missing | Value::Null | Value::Bool(false))
}

fn validate_jump(code: &CodeObject, offset: usize) -> VmResult<()> {
    if offset <= code.instructions.len() {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::InstructionOutOfBounds { offset }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heap::{HeapValue, ScriptHeap};
    use std::collections::BTreeMap;
    use std::num::NonZeroU32;
    use std::sync::Arc;
    use vela_bytecode::compiler::{
        CompilerOptions, compile_function_source, compile_module_sources, compile_program_source,
        compile_program_source_with_options,
    };
    use vela_bytecode::{ConstantId, Instruction, InstructionOffset};
    use vela_common::{
        FieldId, FunctionId, HostMethodId, HostObjectId, HostTypeId, MethodId, SourceId, Symbol,
        TypeId, VariantId,
    };
    use vela_hir::{ModuleGraph, ModulePath, ModuleSource};
    use vela_host::{HostErrorKind, HostValue, MockStateAdapter, PatchOp};
    use vela_reflect::{
        FieldAccess, FieldDesc, FunctionAccess, FunctionDesc, MethodAccess, MethodDesc, ModuleDesc,
        TraitDesc, TraitMethodDesc, TypeDesc, TypeKey, TypeKind, VariantDesc,
    };

    #[test]
    fn runs_basic_arithmetic() {
        let mut code = CodeObject::new("calc", 5);
        let two = code.push_constant(Constant::Int(2));
        let three = code.push_constant(Constant::Int(3));
        let four = code.push_constant(Constant::Int(4));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: two,
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: three,
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(2),
            constant: four,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Mul {
            dst: Register(3),
            lhs: Register(1),
            rhs: Register(2),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Add {
            dst: Register(4),
            lhs: Register(0),
            rhs: Register(3),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(4),
        }));

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(14)));
    }

    #[test]
    fn branches_on_false_conditions() {
        let mut code = CodeObject::new("branch", 3);
        let false_id = code.push_constant(Constant::Bool(false));
        let one = code.push_constant(Constant::Int(1));
        let two = code.push_constant(Constant::Int(2));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: false_id,
        }));
        code.push_instruction(Instruction::new(InstructionKind::JumpIfFalse {
            condition: Register(0),
            target: InstructionOffset(4),
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: one,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Jump {
            target: InstructionOffset(5),
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: two,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(2)));
    }

    #[test]
    fn calls_registered_native_functions() {
        let mut vm = Vm::new();
        vm.register_native("log", |args| {
            assert_eq!(args, [Value::String("level up".into())]);
            Ok(Value::Null)
        });

        let mut code = CodeObject::new("native", 2);
        code.push_constant(Constant::String("level up".into()));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: ConstantId(0),
        }));
        code.push_instruction(Instruction::new(InstructionKind::CallNative {
            dst: Some(Register(1)),
            name: "log".into(),
            args: vec![Register(0)],
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));

        assert_eq!(vm.run(&code), Ok(Value::Null));
    }

    #[test]
    fn instruction_budget_stops_dispatch_before_next_instruction() {
        let mut code = CodeObject::new("budgeted", 2);
        let one = code.push_constant(Constant::Int(1));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: one,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Move {
            dst: Register(1),
            src: Register(0),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));
        let mut budget = ExecutionBudget::new(2, usize::MAX, usize::MAX, usize::MAX);

        let error = Vm::new()
            .run_with_budget(&code, &mut budget)
            .expect_err("third instruction exceeds budget");

        assert_eq!(
            error.kind,
            VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::Instructions,
                limit: 2,
            }
        );
        assert_eq!(budget.instructions_executed(), 2);
        assert_eq!(budget.current_call_depth(), 0);
    }

    #[test]
    fn call_depth_budget_stops_recursive_scripts() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn recurse() {
    return recurse();
}

fn main() {
    return recurse();
}
"#,
        )
        .expect("compile recursive source");
        let mut budget = ExecutionBudget::new(100, usize::MAX, 2, usize::MAX);

        let error = Vm::new()
            .run_program_with_budget(&program, "main", &[], &mut budget)
            .expect_err("recursive call exceeds call depth");

        assert_eq!(
            error.kind,
            VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::CallDepth,
                limit: 2,
            }
        );
        assert_eq!(budget.current_call_depth(), 0);
    }

    #[test]
    fn call_frame_registers_expose_heap_roots_for_gc() {
        let mut heap = ScriptHeap::new();
        let rooted = heap.allocate(HeapValue::String("rooted".into()));
        let garbage = heap.allocate(HeapValue::String("garbage".into()));
        let mut frame = CallFrame::new(2);
        frame
            .write(Register(0), Value::HeapRef(rooted))
            .expect("write heap root");

        let roots = frame.heap_roots();
        let stats = heap.collect_full(&roots);

        assert_eq!(roots, vec![rooted]);
        assert_eq!(stats.marked, 1);
        assert_eq!(stats.swept, 1);
        assert!(heap.contains(rooted));
        assert!(!heap.contains(garbage));
    }

    #[test]
    fn nested_values_expose_heap_roots_for_gc() {
        let mut heap = ScriptHeap::new();
        let rooted = heap.allocate(HeapValue::String("nested".into()));
        let garbage = heap.allocate(HeapValue::String("garbage".into()));
        let mut fields = BTreeMap::new();
        fields.insert("item".into(), Value::HeapRef(rooted));
        let mut frame = CallFrame::new(1);
        frame
            .write(
                Register(0),
                Value::Record {
                    type_name: "Reward".into(),
                    fields: ScriptFields::from_pairs("Reward", fields),
                },
            )
            .expect("write nested root");

        let stats = heap.collect_full(&frame.heap_roots());

        assert_eq!(stats.marked, 1);
        assert_eq!(stats.swept, 1);
        assert!(heap.contains(rooted));
        assert!(!heap.contains(garbage));
    }

    #[test]
    fn record_slot_bytecode_reads_and_writes_by_slot() {
        let mut code = CodeObject::new("slot_record", 3);
        let count = code.push_constant(Constant::Int(2));
        let updated = code.push_constant(Constant::Int(5));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: count,
        }));
        code.push_instruction(Instruction::new(InstructionKind::MakeRecord {
            dst: Register(1),
            type_name: "Reward".into(),
            fields: vec![
                ("item_id".into(), Register(0)),
                ("count".into(), Register(0)),
            ],
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: updated,
        }));
        code.push_instruction(Instruction::new(InstructionKind::SetRecordSlot {
            record: Register(1),
            field: "count".into(),
            slot: 0,
            src: Register(0),
        }));
        code.push_instruction(Instruction::new(InstructionKind::GetRecordSlot {
            dst: Register(2),
            record: Register(1),
            field: "count".into(),
            slot: 0,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(5)));
    }

    #[test]
    fn enum_slot_bytecode_reads_by_slot() {
        let mut code = CodeObject::new("slot_enum", 3);
        let amount = code.push_constant(Constant::Int(7));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: amount,
        }));
        code.push_instruction(Instruction::new(InstructionKind::MakeEnum {
            dst: Register(1),
            enum_name: "Damage".into(),
            variant: "Physical".into(),
            fields: vec![("amount".into(), Register(0))],
        }));
        code.push_instruction(Instruction::new(InstructionKind::GetEnumSlot {
            dst: Register(2),
            value: Register(1),
            field: "amount".into(),
            slot: 0,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
    }

    #[test]
    fn runs_compiled_arithmetic_source() {
        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { let base = 2; return base + 3 * 4; }",
            "main",
        )
        .expect("compile arithmetic source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(14)));
    }

    #[test]
    fn runs_compiled_radix_ints_and_exponent_floats() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let base = 0x10 + 0b10;
    let scaled = 3.5e+1 / 2.5;
    if base == 18 && scaled == 14.0 {
        return scaled;
    }
    return 0.0;
}
"#,
            "main",
        )
        .expect("compile numeric literal source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Float(14.0)));
    }

    #[test]
    fn runs_compiled_shebang_source() {
        let code = compile_function_source(
            SourceId::new(1),
            "#!/usr/bin/env vela\nfn main() { return 7; }\n",
            "main",
        )
        .expect("compile shebang source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
    }

    #[test]
    fn runs_compiled_unicode_string_escapes() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"fn main() { return "\u{41}\u{7a}"; }"#,
            "main",
        )
        .expect("compile unicode escaped string source");

        assert_eq!(Vm::new().run(&code), Ok(Value::String("Az".into())));
    }

    #[test]
    fn runs_compiled_unary_operator_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    if !false {
        return -5;
    }
    return 0;
}
"#,
            "main",
        )
        .expect("compile unary operator source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(-5)));
    }

    #[test]
    fn runs_compiled_logical_short_circuit_source() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn and_case() {
    return false && fail();
}

fn or_case() {
    return true || fail();
}

fn truthy_case() {
    return true && 5 && ("reward" || fail());
}
"#,
        )
        .expect("compile logical short-circuit source");

        assert_eq!(
            Vm::new().run_program(&program, "and_case", &[]),
            Ok(Value::Bool(false))
        );
        assert_eq!(
            Vm::new().run_program(&program, "or_case", &[]),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            Vm::new().run_program(&program, "truthy_case", &[]),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn runs_compiled_local_assignment_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 1;
    value += 4;
    value *= 3;
    value -= 5;
    value /= 2;
    value %= 5;
    let copy = (value = value + 10);
    return value + copy;
}
"#,
            "main",
        )
        .expect("compile local assignment source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
    }

    #[test]
    fn runs_compiled_index_read_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = [2, 4, 8];
    let rewards = { "xp": 6 };
    return values[1] + rewards["xp"];
}
"#,
            "main",
        )
        .expect("compile index read source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(10)));
    }

    #[test]
    fn managed_heap_execution_reads_heap_index_values() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn array_case() {
    let names = ["gold", "xp"];
    return names[1];
}

fn map_case() {
    let rewards = { "gold": 7 };
    return rewards["gold"];
}
"#,
        )
        .expect("compile heap index source");
        let mut budget = ExecutionBudget::unbounded();

        assert_eq!(
            Vm::new()
                .run_program_with_managed_heap_and_budget(&program, "array_case", &[], &mut budget)
                .expect("run heap array index"),
            Value::String("xp".into())
        );
        assert_eq!(
            Vm::new()
                .run_program_with_managed_heap_and_budget(&program, "map_case", &[], &mut budget)
                .expect("run heap map index"),
            Value::Int(7)
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn runs_compiled_index_write_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = [2, 4, 8];
    let rewards = { "xp": 6 };
    values[1] = 10;
    values[2] += 5;
    rewards["xp"] += values[1];
    rewards["gold"] = 3;
    let copy = (values[0] = rewards["gold"]);
    return values[0] + values[1] + values[2] + rewards["xp"] + copy;
}
"#,
            "main",
        )
        .expect("compile index write source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(45)));
    }

    #[test]
    fn runs_compiled_record_field_write_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let reward = Reward { item_id: "gold", count: 2 };
    reward.count += 3;
    reward.item_id = "xp";
    return reward.count + reward.item_id.len();
}
"#,
            "main",
        )
        .expect("compile record field write source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
    }

    #[test]
    fn managed_heap_execution_writes_heap_index_values() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn array_case() {
    let names = ["gold", "xp"];
    names[0] = "silver";
    return names[0];
}

fn map_case() {
    let rewards = { "gold": 7 };
    rewards["gold"] += 5;
    rewards["xp"] = 3;
    return rewards["gold"] + rewards["xp"];
}
"#,
        )
        .expect("compile heap index write source");
        let mut budget = ExecutionBudget::unbounded();

        assert_eq!(
            Vm::new()
                .run_program_with_managed_heap_and_budget(&program, "array_case", &[], &mut budget)
                .expect("run heap array index write"),
            Value::String("silver".into())
        );
        assert_eq!(
            Vm::new()
                .run_program_with_managed_heap_and_budget(&program, "map_case", &[], &mut budget)
                .expect("run heap map index write"),
            Value::Int(15)
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_execution_writes_heap_record_fields() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let reward = Reward { item_id: "gold", count: 2 };
    reward.count += 5;
    reward.item_id = "xp";
    return reward.count + reward.item_id.len();
}
"#,
        )
        .expect("compile heap record field writes");
        let mut budget = ExecutionBudget::unbounded();

        assert_eq!(
            Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
            Ok(Value::Int(9))
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn runs_compiled_for_in_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3] {
        total += value;
    }
    let rewards = { "gold": 4, "xp": 6 };
    for reward in rewards {
        total += reward;
    }
    return total;
}
"#,
            "main",
        )
        .expect("compile for-in source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(16)));
    }

    #[test]
    fn runs_compiled_range_for_in_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let total = 0;
    for value in 1..4 {
        total += value;
    }
    for value in 4..=5 {
        total += value;
    }
    return total;
}
"#,
            "main",
        )
        .expect("compile range for-in source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(15)));
    }

    #[test]
    fn runs_compiled_script_value_methods() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = [1, 2, 3];
    let rewards = {"gold": 4, "xp": 6};
    let empty = [];
    values.push(4);
    let popped = values.pop();
    rewards.set("quest", 8);
    let removed = rewards.remove("gold");
    let keys = rewards.keys();
    let amounts = rewards.values();
    let entries = rewards.entries();
    if empty.is_empty() && values.len() == 3 && popped == 4 && rewards.len() == 2 && ("gold").len() == 4
        && ("gold").contains("ol") && ("quest").starts_with("que") && ("quest").ends_with("st")
        && removed == 4 && rewards.has("quest") && rewards.get("xp") == 6 && rewards.get_or("missing", 10) == 10
        && keys[0] == "quest" && keys[1] == "xp"
        && amounts[0] == 8 && amounts[1] == 6
        && entries[0].key == "quest" && entries[1].value == 6 {
        return values.len();
    }
    return 0;
}
"#,
        )
        .expect("compile script value methods");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(3))
        );
    }

    #[test]
    fn runs_compiled_script_impl_method_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
        )
        .expect("compile script impl method dispatch");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(12))
        );
    }

    #[test]
    fn runs_compiled_typed_parameter_method_id_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main(player: Player) {
    return player.bonus(5);
}
"#,
        )
        .expect("compile typed parameter method id dispatch");
        let player = Value::Record {
            type_name: "Player".to_owned(),
            fields: ScriptFields::from_pairs("Player", [("level".to_owned(), Value::Int(7))]),
        };

        assert_eq!(
            Vm::new().run_program(&program, "main", &[player]),
            Ok(Value::Int(12))
        );
    }

    #[test]
    fn runs_compiled_immediate_script_method_id_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
        )
        .expect("compile immediate script method id dispatch");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(12))
        );
    }

    #[test]
    fn runs_compiled_trait_default_method_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource {
    fn bonus(self, amount) -> int { return self.level + amount; }
    fn label(self) -> string { return self.name; }
}
struct Player { level: int, name: string }

impl BonusSource for Player {}

fn main() {
    let player = Player { level: 7, name: "hero" };
    return player.bonus(5) + player.label().len();
}
"#,
        )
        .expect("compile trait default method dispatch");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(16))
        );
    }

    #[test]
    fn runs_compiled_self_method_id_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource {
    fn label(self) -> string;
    fn summary(self) -> string { return self.label(); }
}
struct Player { name: string }

impl BonusSource for Player {
    fn label(self) -> string {
        return self.name;
    }
}

fn main() {
    return Player { name: "hero" }.summary();
}
"#,
        )
        .expect("compile self method id dispatch");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::String("hero".to_owned()))
        );
    }

    #[test]
    fn runs_compiled_captured_receiver_method_id_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    let bonus = |ignored| player.bonus(5);
    return bonus(null);
}
"#,
        )
        .expect("compile captured receiver method id dispatch");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(12))
        );
    }

    #[test]
    fn runs_compiled_binding_pattern_receiver_method_id_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return match player {
        bound => bound.bonus(5),
    };
}
"#,
        )
        .expect("compile binding pattern receiver method id dispatch");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(12))
        );
    }

    #[test]
    fn runs_compiled_host_ref_script_impl_method_dispatch() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return reflect.get(self, "level") + amount;
    }
}

fn main(player) {
    return player.bonus(5);
}
"#,
        )
        .expect("compile host ref script impl method dispatch");
        let mut adapter = host_adapter(host_ref, HostValue::Int(7));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Ok(Value::Int(12))
        );
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn host_ref_script_impl_dispatch_uses_registered_type_registry() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return amount + 7;
    }
}

fn main(player) {
    return player.bonus(5);
}
"#,
        )
        .expect("compile host ref script impl method dispatch");
        let mut adapter = host_adapter(host_ref, HostValue::Int(7));
        let mut tx = PatchTx::new();
        let vm = Vm::new().with_type_registry(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Ok(Value::Int(12))
        );
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn runs_compiled_record_variant_field_method_id_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

enum Event {
    Grant { player: Player },
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let event = Event.Grant { player: Player { level: 7 } };
    return match event {
        Event.Grant { player } => player.bonus(5),
        _ => 0,
    };
}
"#,
        )
        .expect("compile record variant field method id dispatch");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(12))
        );
    }

    #[test]
    fn runs_compiled_tuple_variant_field_method_id_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

enum Event {
    Grant(player: Player),
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let event = Event.Grant(Player { level: 7 });
    return match event {
        Event.Grant(player) => player.bonus(5),
        _ => 0,
    };
}
"#,
        )
        .expect("compile tuple variant field method id dispatch");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(12))
        );
    }

    #[test]
    fn explicit_impl_method_overrides_trait_default_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource {
    fn bonus(self, amount) -> int { return self.level + amount; }
}
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return amount * 2;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
        )
        .expect("compile explicit impl method override");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(10))
        );
    }

    #[test]
    fn runs_compiled_module_qualified_script_impl_method_dispatch() {
        let program = compile_module_sources(&[ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.combat"),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

pub fn main() {
    let player = Player { level: 10 };
    return player.bonus(4);
}
"#,
        )])
        .expect("compile module-qualified script impl method dispatch");

        assert_eq!(
            Vm::new().run_program(&program, "game.combat.main", &[]),
            Ok(Value::Int(14))
        );
    }

    #[test]
    fn runs_compiled_module_typed_parameter_method_id_dispatch() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.model"),
                r#"
pub trait BonusSource { fn bonus(self, amount) -> int; }
pub struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.combat"),
                r#"
use game.model.Player

pub fn main(player: Player) {
    return player.bonus(5);
}
"#,
            ),
        ])
        .expect("compile module typed parameter method id dispatch");
        let player = Value::Record {
            type_name: "game.model.Player".to_owned(),
            fields: ScriptFields::from_pairs(
                "game.model.Player",
                [("level".to_owned(), Value::Int(7))],
            ),
        };

        assert_eq!(
            Vm::new().run_program(&program, "game.combat.main", &[player]),
            Ok(Value::Int(12))
        );
    }

    #[test]
    fn runs_compiled_break_continue_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3, 4, 5] {
        if value == 2 {
            continue;
        }
        if value == 5 {
            break;
        }
        total += value;
    }
    return total;
}
"#,
            "main",
        )
        .expect("compile break and continue source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
    }

    #[test]
    fn runs_compiled_block_and_if_expression_values() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = {
        let base = 2;
        base + 3;
    };
    let selected = if value > 4 {
        value;
    } else {
        0;
    };
    return selected;
}
"#,
            "main",
        )
        .expect("compile block and if expression values");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(5)));
    }

    #[test]
    fn runs_compiled_returning_block_initializer() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let ignored = {
        return 7;
    };
    return 0;
}
"#,
            "main",
        )
        .expect("compile returning block initializer");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
    }

    #[test]
    fn runs_compiled_returning_if_and_match_initializers() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn if_case(flag) {
    let ignored = if flag {
        return 7;
    } else {
        return 8;
    };
    return 0;
}

fn match_case(value) {
    let ignored = match value {
        1 => { return 10; },
        _ => { return 11; },
    };
    return 0;
}
"#,
        )
        .expect("compile returning if and match initializers");

        assert_eq!(
            Vm::new().run_program(&program, "if_case", &[Value::Bool(true)]),
            Ok(Value::Int(7))
        );
        assert_eq!(
            Vm::new().run_program(&program, "if_case", &[Value::Bool(false)]),
            Ok(Value::Int(8))
        );
        assert_eq!(
            Vm::new().run_program(&program, "match_case", &[Value::Int(1)]),
            Ok(Value::Int(10))
        );
        assert_eq!(
            Vm::new().run_program(&program, "match_case", &[Value::Int(2)]),
            Ok(Value::Int(11))
        );
    }

    #[test]
    fn runs_compiled_match_expression_values() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    let value = match damage {
        Damage.Magical { amount } => amount + 100,
        Damage.Physical { amount } => {
            amount + 1;
        },
        _ => 0,
    };
    return value;
}
"#,
            "main",
        )
        .expect("compile match expression values");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
    }

    #[test]
    fn runs_compiled_literal_match_patterns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 2;
    return match value {
        1 => 10,
        2 => 20,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("compile literal match patterns");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
    }

    #[test]
    fn managed_heap_execution_runs_string_literal_match_patterns() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let label = "xp";
    return match label {
        "gold" => 1,
        "xp" => 2,
        _ => 0,
    };
}
"#,
        )
        .expect("compile heap string literal match patterns");
        let mut budget = ExecutionBudget::unbounded();

        assert_eq!(
            Vm::new()
                .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
                .expect("run heap string literal match patterns"),
            Value::Int(2)
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn runs_compiled_binding_match_patterns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 7;
    return match value {
        bound => bound + 1,
    };
}
"#,
            "main",
        )
        .expect("compile binding match patterns");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
    }

    #[test]
    fn binding_match_assignment_does_not_mutate_scrutinee() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 7;
    match value {
        bound => {
            bound = 100;
        }
    }
    return value;
}
"#,
            "main",
        )
        .expect("compile binding match assignment");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
    }

    #[test]
    fn runs_compiled_match_guards() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 7;
    return match value {
        bound if bound < 5 => 10,
        bound if bound == 7 => bound + 1,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("compile match guards");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
    }

    #[test]
    fn match_guards_can_read_record_pattern_bindings() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    return match damage {
        Damage.Physical { amount } if amount > 10 => 100,
        Damage.Physical { amount } if amount == 7 => amount + 1,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("compile tuple variant literal pattern");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
    }

    #[test]
    fn runs_compiled_record_variant_field_patterns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
enum Reward {
    Grant { kind, amount }
}

fn main() {
    let reward = Reward.Grant { kind: "xp", amount: 7 };
    return match reward {
        Reward.Grant { kind: "gold", amount } => amount,
        Reward.Grant { kind: "xp", amount } => amount + 1,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("compile record variant field patterns");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
    }

    #[test]
    fn managed_heap_execution_runs_nested_record_variant_field_patterns() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
enum Reward {
    Grant { payload }
}

enum Payload {
    Xp(amount)
    Gold(amount)
}

fn main() {
    let reward = Reward.Grant { payload: Payload.Xp(7) };
    return match reward {
        Reward.Grant { payload: Payload.Gold(amount) } => amount,
        Reward.Grant { payload: Payload.Xp(amount) } => amount + 1,
        _ => 0,
    };
}
"#,
        )
        .expect("compile nested record variant field patterns");
        let mut budget = ExecutionBudget::new(10_000, 32_000, 32, 32);

        assert_eq!(
            Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
            Ok(Value::Int(8))
        );
    }

    #[test]
    fn runs_compiled_tuple_variant_constructor_and_patterns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
enum Damage {
    Physical(amount, bonus),
    Magical(amount),
}

fn main() {
    let damage = Damage.Physical(7, 2);
    return match damage {
        Damage.Physical(amount, bonus) => amount + bonus,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("compile tuple variant constructor and pattern");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(9)));
    }

    #[test]
    fn managed_heap_execution_runs_tuple_variant_literal_patterns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
enum Damage {
    Typed(kind, amount),
}

fn main() {
    let damage = Damage.Typed("fire", 7);
    return match damage {
        Damage.Typed("frost", amount) => amount + 100,
        Damage.Typed("fire", amount) => amount + 1,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("compile guarded record pattern");

        let mut budget = ExecutionBudget::new(10_000, 32_000, 32, 32);
        assert_eq!(
            Vm::new().run_with_managed_heap_and_budget(&code, &mut budget),
            Ok(Value::Int(8))
        );
    }

    #[test]
    fn managed_heap_execution_runs_for_in_source() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn sum() {
    let total = 0;
    for value in [1, 2, 3] {
        total += value;
    }
    for reward in { "gold": 4, "xp": 6 } {
        total += reward;
    }
    return total;
}

fn last_name() {
    let name = "";
    for value in ["gold", "xp"] {
        name = value;
    }
    return name;
}
"#,
        )
        .expect("compile heap for-in source");
        let mut budget = ExecutionBudget::unbounded();

        assert_eq!(
            Vm::new()
                .run_program_with_managed_heap_and_budget(&program, "sum", &[], &mut budget)
                .expect("run heap for-in sum"),
            Value::Int(16)
        );
        assert_eq!(
            Vm::new()
                .run_program_with_managed_heap_and_budget(&program, "last_name", &[], &mut budget)
                .expect("run heap for-in string"),
            Value::String("xp".into())
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_execution_runs_range_for_in_source() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let total = 0;
    for value in 2..=4 {
        total += value;
    }
    return total;
}
"#,
        )
        .expect("compile heap range for-in source");
        let mut budget = ExecutionBudget::unbounded();

        assert_eq!(
            Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
            Ok(Value::Int(9))
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_execution_runs_script_value_methods() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let names = ["gold", "xp"];
    let rewards = {"gold": 4, "xp": 6};
    names.push("quest");
    let popped = names.pop();
    rewards.set("quest", "done");
    let removed = rewards.remove("gold");
    let keys = rewards.keys();
    let amounts = rewards.values();
    let entries = rewards.entries();
    if names.len() == 2 && popped == "quest" && popped.contains("ue") && popped.starts_with("que")
        && popped.ends_with("st") && removed == 4 && rewards.is_empty() == false && ("quest").len() == 5
        && rewards.has("quest") && rewards.get("xp") == 6 && rewards.get_or("missing", "fallback") == "fallback"
        && keys[0] == "quest" && keys[1] == "xp"
        && amounts[0] == "done" && amounts[1] == 6
        && entries[0].key == "quest" && entries[1].value == 6 {
        return names[0].len();
    }
    return 0;
}
"#,
        )
        .expect("compile heap script value methods");
        let mut budget = ExecutionBudget::unbounded();

        assert_eq!(
            Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
            Ok(Value::Int(4))
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_execution_runs_script_impl_method_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 8 };
    return player.bonus(6);
}
"#,
        )
        .expect("compile heap script impl method dispatch");
        let mut budget = ExecutionBudget::unbounded();

        assert_eq!(
            Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
            Ok(Value::Int(14))
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_execution_runs_trait_default_method_dispatch() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource {
    fn bonus(self, amount) -> int { return self.level + amount; }
    fn label(self) -> string { return self.name; }
}
struct Player { level: int, name: string }

impl BonusSource for Player {}

fn main() {
    let player = Player { level: 8, name: "hero" };
    return player.bonus(6) + player.label().len();
}
"#,
        )
        .expect("compile heap trait default method dispatch");
        let mut budget = ExecutionBudget::unbounded();

        assert_eq!(
            Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
            Ok(Value::Int(18))
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn runs_compiled_const_expression_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
const BASE: int = 10;
const BONUS: int = BASE + 5 * 2;

fn main() {
    return BONUS;
}
"#,
            "main",
        )
        .expect("compile const expression source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
    }

    #[test]
    fn runs_compiled_native_call_source() {
        let mut vm = Vm::new();
        vm.register_native("log", |args| {
            assert_eq!(args, [Value::String("compiled".into())]);
            Ok(Value::Int(7))
        });

        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { return log(\"compiled\"); }",
            "main",
        )
        .expect("compile native call source");

        assert_eq!(vm.run(&code), Ok(Value::Int(7)));
    }

    #[test]
    fn heap_execution_materializes_native_args_and_stores_result() {
        let mut vm = Vm::new();
        vm.register_native("echo_label", |args| {
            assert_eq!(args, [Value::String("compiled".into())]);
            Ok(Value::String("native-result".into()))
        });
        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { return echo_label(\"compiled\"); }",
            "main",
        )
        .expect("compile native call source");
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = vm
            .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
            .expect("run heap native call");

        let Value::HeapRef(result_ref) = result else {
            panic!("expected heap-backed native result");
        };
        assert_eq!(
            heap.get(result_ref),
            Some(&HeapValue::String("native-result".into()))
        );
    }

    #[test]
    fn runs_compiled_script_function_calls() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn add_bonus(value) {
    return value + 5;
}

fn main() {
    let base = 10;
    return add_bonus(base) * 2;
}
"#,
        )
        .expect("compile program source");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(30))
        );
    }

    #[test]
    fn runs_compiled_named_args_and_parameter_defaults() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn grant(base, amount = 10, bonus = amount + 1) {
    return base + amount + bonus;
}

fn main() {
    return grant(bonus = 5, base = 1);
}
"#,
        )
        .expect("compile named args and parameter defaults");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(16))
        );
    }

    #[test]
    fn runs_entrypoint_parameter_defaults() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main(value = 7) {
    return value + 1;
}
"#,
            "main",
        )
        .expect("compile entrypoint default");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
    }

    #[test]
    fn runs_compiled_lambdas_with_captures_after_outer_return() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn make_adder(base) {
    return |value| value + base;
}

fn main() {
    let add = make_adder(10);
    return add(5);
}
"#,
        )
        .expect("compile captured lambda");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(15))
        );
    }

    #[test]
    fn runs_immediate_lambda_calls_and_block_returns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let direct = (|value| value + 1)(4);
    let block = |value| { return value + direct; };
    return block(6);
}
"#,
            "main",
        )
        .expect("compile immediate lambda call");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(11)));
    }

    #[test]
    fn runs_try_propagation_for_option_values() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
enum Option {
    Some(value)
    None
}

fn maybe(value) {
    if value > 0 {
        return Option.Some(value);
    }
    return Option.None {};
}

fn present() {
    let value = maybe(4)?;
    return Option.Some(value + 1);
}

fn missing() {
    let value = maybe(0)?;
    return Option.Some(value + 1);
}
"#,
        )
        .expect("compile option propagation");

        assert_eq!(
            Vm::new().run_program(&program, "present", &[]),
            Ok(Value::Enum {
                enum_name: "Option".into(),
                variant: "Some".into(),
                fields: ScriptFields::from_pairs("Option.Some", [("0".into(), Value::Int(5))]),
            })
        );
        assert_eq!(
            Vm::new().run_program(&program, "missing", &[]),
            Ok(Value::Enum {
                enum_name: "Option".into(),
                variant: "None".into(),
                fields: ScriptFields::from_pairs("Option.None", BTreeMap::new()),
            })
        );
    }

    #[test]
    fn managed_heap_execution_runs_try_propagation_for_result_values() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
enum Result {
    Ok(value)
    Err(message)
}

fn checked(value) {
    if value > 0 {
        return Result.Ok(value);
    }
    return Result.Err("bad");
}

fn ok_case() {
    let value = checked(3)?;
    return Result.Ok(value + 7);
}

fn err_case() {
    let value = checked(0)?;
    return Result.Ok(value + 7);
}
"#,
        )
        .expect("compile result propagation");
        let mut budget = ExecutionBudget::new(10_000, 4096, 64, 16);

        assert_eq!(
            Vm::new().run_program_with_managed_heap_and_budget(
                &program,
                "ok_case",
                &[],
                &mut budget
            ),
            Ok(Value::Enum {
                enum_name: "Result".into(),
                variant: "Ok".into(),
                fields: ScriptFields::from_pairs("Result.Ok", [("0".into(), Value::Int(10))]),
            })
        );

        let mut budget = ExecutionBudget::new(10_000, 4096, 64, 16);
        assert_eq!(
            Vm::new().run_program_with_managed_heap_and_budget(
                &program,
                "err_case",
                &[],
                &mut budget
            ),
            Ok(Value::Enum {
                enum_name: "Result".into(),
                variant: "Err".into(),
                fields: ScriptFields::from_pairs(
                    "Result.Err",
                    [("0".into(), Value::String("bad".into()))],
                ),
            })
        );
    }

    #[test]
    fn managed_heap_execution_runs_string_parameter_defaults() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn choose(prefix = "quest", suffix = "done") {
    return prefix == "quest" && suffix == "done";
}

fn main() {
    return choose(suffix = "done");
}
"#,
        )
        .expect("compile heap parameter defaults");
        let mut budget = ExecutionBudget::new(10_000, 32_000, 32, 32);

        assert_eq!(
            Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn runs_compiled_cross_module_imported_script_call() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.reward.grant as give_reward

fn main() {
    return give_reward(4);
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.reward"),
                r#"
pub fn grant(amount) {
    return amount + 1;
}
"#,
            ),
        ])
        .expect("compile imported cross-module script call");

        assert_eq!(
            Vm::new().run_program(&program, "game.main.main", &[]),
            Ok(Value::Int(5))
        );
    }

    #[test]
    fn runs_compiled_same_named_cross_module_functions() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.reward.main as reward_main

fn main() {
    return reward_main();
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.reward"),
                r#"
pub fn main() {
    return 7;
}
"#,
            ),
        ])
        .expect("compile same-named cross-module functions");

        assert_eq!(
            Vm::new().run_program(&program, "game.main.main", &[]),
            Ok(Value::Int(7))
        );
    }

    #[test]
    fn runs_compiled_cross_module_imported_const_expression() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.tuning.BONUS as REWARD

fn main() {
    return REWARD + 1;
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.tuning"),
                r#"
use game.base.BASE as START

pub const BONUS: int = START + 1;
"#,
            ),
            ModuleSource::new(
                SourceId::new(3),
                ModulePath::from_dotted("game.base"),
                r#"
pub const BASE: int = 4;
"#,
            ),
        ])
        .expect("compile imported cross-module const expression");

        assert_eq!(
            Vm::new().run_program(&program, "game.main.main", &[]),
            Ok(Value::Int(6))
        );
    }

    #[test]
    fn runs_compiled_cross_module_imported_type_constructors() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.reward.Reward as Prize
use game.damage.Damage as Hit

fn make_reward() {
    return Prize { count: 2 };
}

fn make_damage() {
    return Hit.Physical { amount: 7 };
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.reward"),
                r#"
pub struct Reward { count: int }
"#,
            ),
            ModuleSource::new(
                SourceId::new(3),
                ModulePath::from_dotted("game.damage"),
                r#"
pub enum Damage { Physical }
"#,
            ),
        ])
        .expect("compile imported cross-module type constructors");
        let mut reward_fields = BTreeMap::new();
        reward_fields.insert("count".into(), Value::Int(2));
        let mut damage_fields = BTreeMap::new();
        damage_fields.insert("amount".into(), Value::Int(7));

        assert_eq!(
            Vm::new().run_program(&program, "game.main.make_reward", &[]),
            Ok(Value::Record {
                type_name: "game.reward.Reward".into(),
                fields: ScriptFields::from_pairs("game.reward.Reward", reward_fields),
            })
        );
        assert_eq!(
            Vm::new().run_program(&program, "game.main.make_damage", &[]),
            Ok(Value::Enum {
                enum_name: "game.damage.Damage".into(),
                variant: "Physical".into(),
                fields: ScriptFields::from_pairs("game.damage.Damage.Physical", damage_fields),
            })
        );
    }

    #[test]
    fn runs_compiled_cross_module_imported_match_patterns() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.damage.Damage as Hit

fn main() {
    let damage = Hit.Physical { amount: 7 };
    match damage {
        Hit.Magical { amount } => { return amount + 100; },
        Hit.Physical { amount } => { return amount; },
        _ => { return 0; },
    }
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.damage"),
                r#"
pub enum Damage { Physical, Magical }
"#,
            ),
        ])
        .expect("compile imported cross-module match pattern");

        assert_eq!(
            Vm::new().run_program(&program, "game.main.main", &[]),
            Ok(Value::Int(7))
        );
    }

    #[test]
    fn runs_compiled_cross_module_qualified_function_and_const_paths() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
fn main() {
    return game.reward.grant() + game.config.BONUS;
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.reward"),
                r#"
pub fn grant() {
    return 4;
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(3),
                ModulePath::from_dotted("game.config"),
                r#"
pub const BONUS: int = 5;
"#,
            ),
        ])
        .expect("compile qualified cross-module paths");

        assert_eq!(
            Vm::new().run_program(&program, "game.main.main", &[]),
            Ok(Value::Int(9))
        );
    }

    #[test]
    fn heap_safe_point_gc_preserves_caller_roots_during_nested_calls() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn allocate_garbage() {
    let temporary = "temporary";
    return 1;
}

fn main() {
    let player = Player { name: "outer", level: 1 };
    let ignored = allocate_garbage();
    let after = "after";
    return player.name;
}
"#,
        )
        .expect("compile nested heap source");
        let mut heap = ScriptHeap::new();
        heap.set_gc_config(heap::GcConfig {
            max_pause_micros: 500,
            heap_growth_factor: 1.0,
        });
        let mut heap_execution =
            HeapExecution::new(&mut heap).with_safe_point_gc_budget(GcBudget::unlimited());
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = Vm::new()
            .run_program_with_heap_and_budget(
                &program,
                "main",
                &[],
                &mut heap_execution,
                &mut budget,
            )
            .expect("run nested heap source");

        let Value::HeapRef(result_ref) = result else {
            panic!("expected heap-backed field result");
        };
        assert_eq!(
            heap_execution.heap.get(result_ref),
            Some(&HeapValue::String("outer".into()))
        );
        assert_eq!(
            heap_execution
                .last_gc_step()
                .expect("safe-point GC should have run")
                .swept,
            1
        );
        assert_eq!(heap_execution.heap.live_object_count(), 3);
        assert_eq!(
            budget.memory_bytes_allocated(),
            heap_execution.heap.allocated_bytes()
        );
    }

    #[test]
    fn managed_heap_execution_materializes_return_and_releases_budget() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    return Reward { item_id: "gold", count: 2 };
}
"#,
        )
        .expect("compile record return source");
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);
        let mut fields = BTreeMap::new();
        fields.insert("count".into(), Value::Int(2));
        fields.insert("item_id".into(), Value::String("gold".into()));

        let result = Vm::new()
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("run managed heap source");

        assert_eq!(
            result,
            Value::Record {
                type_name: "Reward".into(),
                fields: ScriptFields::from_pairs("Reward", fields),
            }
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_execution_releases_budget_after_errors() {
        let mut code = CodeObject::new("main", 2);
        let label = code.push_constant(Constant::String("allocated-before-error".into()));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: label,
        }));
        code.push_instruction(Instruction::new(InstructionKind::CallNative {
            dst: Some(Register(1)),
            name: "missing".into(),
            args: Vec::new(),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(0),
        }));
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let error = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect_err("missing native should fail");

        assert_eq!(
            error.kind,
            VmErrorKind::UnknownNative {
                name: "missing".into()
            }
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_host_execution_materializes_return_and_records_patch() {
        let host_ref = player_ref(3);
        let mut code = CodeObject::new("main", 2).with_params(vec!["player".into()]);
        let gold = code.push_constant(Constant::String("gold".into()));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: gold,
        }));
        code.push_instruction(Instruction::new(InstructionKind::SetHostField {
            root: Register(0),
            field: level_field(),
            src: Register(1),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::String("old".into()));
        let mut tx = PatchTx::new();
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new()
                .run_program_with_host_managed_heap_and_budget(
                    &program,
                    "main",
                    &[Value::HostRef(host_ref)],
                    &mut host,
                    &mut budget,
                )
                .expect("run managed host heap source")
        };

        assert_eq!(result, Value::String("gold".into()));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::Set(HostValue::String("gold".into()))
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_host_execution_converts_map_for_host_write_and_overlay_read() {
        let host_ref = player_ref(3);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.level = {"class": "mage", score: 3};
    return player.level.len();
}
"#,
            &CompilerOptions::new().with_host_field("level", level_field()),
        )
        .expect("compile host map write source");
        let mut adapter = host_adapter(host_ref, HostValue::Null);
        let mut tx = PatchTx::new();
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new()
                .run_program_with_host_managed_heap_and_budget(
                    &program,
                    "main",
                    &[Value::HostRef(host_ref)],
                    &mut host,
                    &mut budget,
                )
                .expect("run managed host map source")
        };

        let mut expected = BTreeMap::new();
        expected.insert("class".into(), HostValue::String("mage".into()));
        expected.insert("score".into(), HostValue::Int(3));
        assert_eq!(result, Value::Int(2));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Map(expected)));
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Null)
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_host_execution_converts_record_for_host_write_and_overlay_read() {
        let host_ref = player_ref(3);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
struct Reward {
    item_id
    count
}

fn main(player) {
    player.level = Reward { item_id: "gold", count: 2 };
    return player.level;
}
"#,
            &CompilerOptions::new().with_host_field("level", level_field()),
        )
        .expect("compile host record write source");
        let mut adapter = host_adapter(host_ref, HostValue::Null);
        let mut tx = PatchTx::new();
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new()
                .run_program_with_host_managed_heap_and_budget(
                    &program,
                    "main",
                    &[Value::HostRef(host_ref)],
                    &mut host,
                    &mut budget,
                )
                .expect("run managed host record source")
        };

        let mut expected_script_fields = BTreeMap::new();
        expected_script_fields.insert("count".into(), Value::Int(2));
        expected_script_fields.insert("item_id".into(), Value::String("gold".into()));
        let mut expected_host_fields = BTreeMap::new();
        expected_host_fields.insert("count".into(), HostValue::Int(2));
        expected_host_fields.insert("item_id".into(), HostValue::String("gold".into()));
        assert_eq!(
            result,
            Value::Record {
                type_name: "Reward".into(),
                fields: ScriptFields::from_pairs("Reward", expected_script_fields),
            }
        );
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::Set(HostValue::Record {
                type_name: "Reward".into(),
                fields: expected_host_fields,
            })
        );
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Null)
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_host_execution_converts_enum_for_host_write_and_overlay_read() {
        let host_ref = player_ref(3);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.level = Damage.Physical { amount: 7 };
    return player.level;
}
"#,
            &CompilerOptions::new().with_host_field("level", level_field()),
        )
        .expect("compile host enum write source");
        let mut adapter = host_adapter(host_ref, HostValue::Null);
        let mut tx = PatchTx::new();
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new()
                .run_program_with_host_managed_heap_and_budget(
                    &program,
                    "main",
                    &[Value::HostRef(host_ref)],
                    &mut host,
                    &mut budget,
                )
                .expect("run managed host enum source")
        };

        let mut expected_script_fields = BTreeMap::new();
        expected_script_fields.insert("amount".into(), Value::Int(7));
        let mut expected_host_fields = BTreeMap::new();
        expected_host_fields.insert("amount".into(), HostValue::Int(7));
        assert_eq!(
            result,
            Value::Enum {
                enum_name: "Damage".into(),
                variant: "Physical".into(),
                fields: ScriptFields::from_pairs("Damage.Physical", expected_script_fields),
            }
        );
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::Set(HostValue::Enum {
                enum_name: "Damage".into(),
                variant: "Physical".into(),
                fields: expected_host_fields,
            })
        );
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Null)
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn managed_heap_host_execution_converts_host_ref_for_host_write_and_overlay_read() {
        let host_ref = player_ref(3);
        let target_ref = HostRef::new(HostTypeId::new(2), HostObjectId::new(11), 4);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player, target) {
    player.level = target;
    return player.level;
}
"#,
            &CompilerOptions::new().with_host_field("level", level_field()),
        )
        .expect("compile host ref write source");
        let mut adapter = host_adapter(host_ref, HostValue::Null);
        let mut tx = PatchTx::new();
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new()
                .run_program_with_host_managed_heap_and_budget(
                    &program,
                    "main",
                    &[Value::HostRef(host_ref), Value::HostRef(target_ref)],
                    &mut host,
                    &mut budget,
                )
                .expect("run managed host ref source")
        };

        assert_eq!(result, Value::HostRef(target_ref));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::Set(HostValue::HostRef(target_ref))
        );
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Null)
        );
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn passes_arguments_to_program_entry() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn double(value) {
    return value * 2;
}
"#,
        )
        .expect("compile program source");

        assert_eq!(
            Vm::new().run_program(&program, "double", &[Value::Int(9)]),
            Ok(Value::Int(18))
        );
    }

    #[test]
    fn runs_compiled_array_literal_source() {
        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { return [1, 2 + 3, \"gold\"]; }",
            "main",
        )
        .expect("compile array literal source");

        assert_eq!(
            Vm::new().run(&code),
            Ok(Value::Array(vec![
                Value::Int(1),
                Value::Int(5),
                Value::String("gold".into())
            ]))
        );
    }

    #[test]
    fn heap_execution_allocates_array_and_string_literals() {
        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { return [1, 2 + 3, \"gold\"]; }",
            "main",
        )
        .expect("compile array literal source");
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = Vm::new()
            .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
            .expect("run heap-backed array source");

        let Value::HeapRef(array_ref) = result else {
            panic!("expected heap array");
        };
        let Some(HeapValue::Array(values)) = heap.get(array_ref) else {
            panic!("expected heap array object");
        };
        assert_eq!(values[0], HeapSlot::Int(1));
        assert_eq!(values[1], HeapSlot::Int(5));
        let HeapSlot::Ref(string_ref) = values[2] else {
            panic!("expected heap string ref");
        };
        assert_eq!(
            heap.get(string_ref),
            Some(&HeapValue::String("gold".into()))
        );
        assert_eq!(budget.memory_bytes_allocated(), heap.allocated_bytes());
    }

    #[test]
    fn runs_compiled_map_literal_source() {
        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { return {\"level\": 2, exp: 10 + 5}; }",
            "main",
        )
        .expect("compile map literal source");
        let mut expected = BTreeMap::new();
        expected.insert("level".into(), Value::Int(2));
        expected.insert("exp".into(), Value::Int(15));

        assert_eq!(Vm::new().run(&code), Ok(Value::Map(expected)));
    }

    #[test]
    fn runs_record_constructor_and_field_reads() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let level = 3;
    let player = Player { level, exp: 7 };
    return player.level + player.exp;
}
"#,
            "main",
        )
        .expect("compile record source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(10)));
    }

    #[test]
    fn heap_execution_reads_record_fields_from_heap_records() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let level = 3;
    let player = Player { level, exp: 7 };
    return player.level + player.exp;
}
"#,
            "main",
        )
        .expect("compile record source");
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = Vm::new()
            .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
            .expect("run heap-backed record source");

        assert_eq!(result, Value::Int(10));
        assert_eq!(heap.live_object_count(), 1);
    }

    #[test]
    fn returns_first_class_record_values() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return Reward { item_id: "gold", count: 2 };
}
"#,
            "main",
        )
        .expect("compile record source");
        let mut fields = BTreeMap::new();
        fields.insert("count".into(), Value::Int(2));
        fields.insert("item_id".into(), Value::String("gold".into()));

        assert_eq!(
            Vm::new().run(&code),
            Ok(Value::Record {
                type_name: "Reward".into(),
                fields: ScriptFields::from_pairs("Reward", fields),
            })
        );
    }

    #[test]
    fn record_constructors_use_stable_slot_shapes() {
        let first = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return Reward { count: 2, item_id: "gold" };
}
"#,
            "main",
        )
        .expect("compile first record source");
        let second = compile_function_source(
            SourceId::new(2),
            r#"
fn main() {
    return Reward { item_id: "gold", count: 2 };
}
"#,
            "main",
        )
        .expect("compile second record source");

        let Ok(Value::Record {
            fields: first_fields,
            ..
        }) = Vm::new().run(&first)
        else {
            panic!("first record");
        };
        let Ok(Value::Record {
            fields: second_fields,
            ..
        }) = Vm::new().run(&second)
        else {
            panic!("second record");
        };

        assert_eq!(first_fields.shape_id(), second_fields.shape_id());
        assert_eq!(
            first_fields
                .iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            ["count", "item_id"]
        );
    }

    #[test]
    fn runs_compiled_immediate_slot_field_reads() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return Reward { item_id: "gold", count: 2 }.count
        + Damage.Physical { amount: 7 }.amount;
}
"#,
            "main",
        )
        .expect("compile immediate slot field reads");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(9)));
    }

    #[test]
    fn runs_compiled_typed_record_slot_field_reads() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
struct Reward {
    item_id: string,
    count: int,
}

fn make_reward() {
    return Reward { item_id: "gold", count: 2 };
}

fn main() {
    let reward: Reward = make_reward();
    return reward.count;
}
"#,
        )
        .expect("compile typed record slot field read");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(2))
        );
    }

    #[test]
    fn runs_compiled_typed_record_slot_field_writes() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
struct Reward {
    item_id: string,
    count: int,
}

fn make_reward() {
    return Reward { item_id: "gold", count: 2 };
}

fn main() {
    let reward: Reward = make_reward();
    reward.count += 3;
    reward.item_id = "xp";
    return reward.count + reward.item_id.len();
}
"#,
        )
        .expect("compile typed record slot field writes");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(7))
        );
    }

    #[test]
    fn runs_compiled_typed_enum_variant_slot_field_reads() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
enum Damage {
    Physical { amount: int, element: string },
    Magical { amount: int },
}

fn main() {
    let damage = Damage.Physical { amount: 7, element: "slash" };
    return damage.amount + damage.element.len();
}
"#,
        )
        .expect("compile typed enum variant slot field read");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(12))
        );
    }

    #[test]
    fn returns_first_class_enum_values() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return Damage.Physical { amount: 7 };
}
"#,
            "main",
        )
        .expect("compile enum source");
        let mut fields = BTreeMap::new();
        fields.insert("amount".into(), Value::Int(7));

        assert_eq!(
            Vm::new().run(&code),
            Ok(Value::Enum {
                enum_name: "Damage".into(),
                variant: "Physical".into(),
                fields: ScriptFields::from_pairs("Damage.Physical", fields),
            })
        );
    }

    #[test]
    fn matches_enum_tag_and_binds_variant_fields() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    match damage {
        Damage.Magical { amount } => { return amount + 100; },
        Damage.Physical { amount } => { return amount + 1; },
        _ => { return 0; },
    }
}
"#,
            "main",
        )
        .expect("compile enum match source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
    }

    #[test]
    fn heap_execution_matches_enum_tags_and_reads_fields() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    match damage {
        Damage.Magical { amount } => { return amount + 100; },
        Damage.Physical { amount } => { return amount + 1; },
        _ => { return 0; },
    }
}
"#,
            "main",
        )
        .expect("compile enum match source");
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = Vm::new()
            .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
            .expect("run heap-backed enum source");

        assert_eq!(result, Value::Int(8));
        assert_eq!(heap.live_object_count(), 1);
    }

    #[test]
    fn heap_execution_enforces_memory_budget_for_bytecode_allocations() {
        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { return \"this string is too large\"; }",
            "main",
        )
        .expect("compile string source");
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 8, usize::MAX, usize::MAX);

        let error = Vm::new()
            .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
            .expect_err("string allocation should exceed memory budget");

        assert_eq!(
            error.kind,
            VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::MemoryBytes,
                limit: 8,
            }
        );
        assert_eq!(heap.live_object_count(), 0);
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn runs_compiled_if_then_branch_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    if 2 < 3 {
        return 10;
    } else {
        return 20;
    }
}
"#,
            "main",
        )
        .expect("compile if source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(10)));
    }

    #[test]
    fn runs_compiled_if_else_branch_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    if 3 < 2 {
        return 10;
    } else {
        return 20;
    }
}
"#,
            "main",
        )
        .expect("compile if source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
    }

    #[test]
    fn runs_compiled_comparison_and_remainder_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    if 10 % 4 == 2 {
        if 3 >= 3 {
            if 2 <= 5 {
                if 5 != 6 {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
            "main",
        )
        .expect("compile operator source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(1)));
    }

    #[test]
    fn reads_host_field_through_patch_transaction() {
        let (program, host_ref) = host_read_program();
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        let result = Vm::new().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
        );

        assert_eq!(result, Ok(Value::Int(9)));
    }

    #[test]
    fn set_host_field_records_patch_and_overlay_read() {
        let host_ref = player_ref(3);
        let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
        let ten = code.push_constant(Constant::Int(10));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: ten,
        }));
        code.push_instruction(Instruction::new(InstructionKind::SetHostField {
            root: Register(0),
            field: level_field(),
            src: Register(1),
        }));
        code.push_instruction(Instruction::new(InstructionKind::GetHostField {
            dst: Register(2),
            root: Register(0),
            field: level_field(),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(10)));
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(9))
        );
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
        tx.apply(&mut adapter).expect("apply patches");
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(10))
        );
    }

    #[test]
    fn heap_execution_converts_heap_string_for_host_field_write() {
        let host_ref = player_ref(3);
        let mut code = CodeObject::new("main", 2).with_params(vec!["player".into()]);
        let gold = code.push_constant(Constant::String("gold".into()));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: gold,
        }));
        code.push_instruction(Instruction::new(InstructionKind::SetHostField {
            root: Register(0),
            field: level_field(),
            src: Register(1),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::String("old".into()));
        let mut tx = PatchTx::new();
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host_heap_and_budget(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
                &mut heap_execution,
                &mut budget,
            )
        };

        assert!(matches!(result, Ok(Value::HeapRef(_))));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::Set(HostValue::String("gold".into()))
        );
    }

    #[test]
    fn patch_budget_stops_host_writes_before_recording_overflow_patch() {
        let host_ref = player_ref(3);
        let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
        let ten = code.push_constant(Constant::Int(10));
        let eleven = code.push_constant(Constant::Int(11));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: ten,
        }));
        code.push_instruction(Instruction::new(InstructionKind::SetHostField {
            root: Register(0),
            field: level_field(),
            src: Register(1),
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(2),
            constant: eleven,
        }));
        code.push_instruction(Instruction::new(InstructionKind::SetHostField {
            root: Register(0),
            field: level_field(),
            src: Register(2),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut budget = ExecutionBudget::new(100, usize::MAX, usize::MAX, 1);

        let error = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new()
                .run_program_with_host_and_budget(
                    &program,
                    "main",
                    &[Value::HostRef(host_ref)],
                    &mut host,
                    &mut budget,
                )
                .expect_err("second patch exceeds budget")
        };

        assert_eq!(
            error.kind,
            VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::Patches,
                limit: 1,
            }
        );
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(9))
        );
    }

    #[test]
    fn add_host_field_records_patch_and_overlay_read() {
        let host_ref = player_ref(3);
        let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
        let one = code.push_constant(Constant::Int(1));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: one,
        }));
        code.push_instruction(Instruction::new(InstructionKind::AddHostField {
            root: Register(0),
            field: level_field(),
            rhs: Register(1),
        }));
        code.push_instruction(Instruction::new(InstructionKind::GetHostField {
            dst: Register(2),
            root: Register(0),
            field: level_field(),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(10)));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
        tx.apply(&mut adapter).expect("apply patches");
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(10))
        );
    }

    #[test]
    fn host_field_read_rejects_stale_generation() {
        let (program, _host_ref) = host_read_program();
        let fresh_ref = player_ref(3);
        let stale_ref = player_ref(2);
        let mut adapter = host_adapter(fresh_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        let error = Vm::new()
            .run_program_with_host(&program, "main", &[Value::HostRef(stale_ref)], &mut host)
            .expect_err("stale host read");

        assert_eq!(
            error.kind,
            VmErrorKind::Host(vela_host::HostErrorKind::StaleGeneration {
                expected: 2,
                actual: 3
            })
        );
    }

    #[test]
    fn host_field_read_error_keeps_instruction_source_span() {
        let host_ref = player_ref(3);
        let span = Span::new(SourceId::new(7), 20, 32);
        let mut code = CodeObject::new("main", 2).with_params(vec!["player".into()]);
        code.push_instruction(
            Instruction::new(InstructionKind::GetHostField {
                dst: Register(1),
                root: Register(0),
                field: level_field(),
            })
            .with_span(span),
        );
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.deny_read(level_path(host_ref));
        let mut tx = PatchTx::new();
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        let error = Vm::new()
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
            .expect_err("denied host read");

        assert_eq!(error.source_span, Some(span));
        assert_eq!(
            error.kind,
            VmErrorKind::Host(HostErrorKind::PermissionDenied {
                path: level_path(host_ref),
                action: "read"
            })
        );
    }

    #[test]
    fn compiled_source_mutates_host_field_through_patch_tx() {
        let host_ref = player_ref(3);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.level = 10;
    player.level += 1;
    return player.level;
}
"#,
            &CompilerOptions::new().with_host_field("level", level_field()),
        )
        .expect("compile host field source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(11)));
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(9))
        );
        assert_eq!(tx.patches().len(), 2);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
        assert_eq!(tx.patches()[1].op, PatchOp::Add(HostValue::Int(1)));
        tx.apply(&mut adapter).expect("apply patches");
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(11))
        );
    }

    #[test]
    fn compiled_source_mutates_nested_host_field_through_patch_tx() {
        let host_ref = player_ref(3);
        let stats = FieldId::new(8);
        let level = FieldId::new(9);
        let stats_level = HostPath::new(host_ref).field(stats).field(level);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.stats.level += 2;
    return player.stats.level;
}
"#,
            &CompilerOptions::new()
                .with_host_field("stats", stats)
                .with_host_field("level", level),
        )
        .expect("compile nested host field source");
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(stats_level.clone(), HostValue::Int(9));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(11)));
        assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(9)));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].path, stats_level);
        assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(2)));
        tx.apply(&mut adapter).expect("apply nested host patch");
        assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(11)));
    }

    #[test]
    fn compiled_source_subtracts_nested_host_field_through_patch_tx() {
        let host_ref = player_ref(3);
        let stats = FieldId::new(8);
        let level = FieldId::new(9);
        let stats_level = HostPath::new(host_ref).field(stats).field(level);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.stats.level -= 2;
    return player.stats.level;
}
"#,
            &CompilerOptions::new()
                .with_host_field("stats", stats)
                .with_host_field("level", level),
        )
        .expect("compile nested host subtraction source");
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(stats_level.clone(), HostValue::Int(9));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(7)));
        assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(9)));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].path, stats_level);
        assert_eq!(tx.patches()[0].op, PatchOp::Sub(HostValue::Int(2)));
        tx.apply(&mut adapter).expect("apply nested host sub patch");
        assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(7)));
    }

    #[test]
    fn compiled_source_pushes_host_path_through_patch_tx() {
        let host_ref = player_ref(3);
        let inventory = FieldId::new(8);
        let rewards = FieldId::new(9);
        let reward_path = HostPath::new(host_ref).field(inventory).field(rewards);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.inventory.rewards.push("gold");
    return player.inventory.rewards.len();
}
"#,
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_field("rewards", rewards),
        )
        .expect("compile host path push source");
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(
            reward_path.clone(),
            HostValue::Array(vec![HostValue::String("xp".into())]),
        );
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(2)));
        assert_eq!(
            adapter.read_path(&reward_path),
            Ok(HostValue::Array(vec![HostValue::String("xp".into())]))
        );
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].path, reward_path);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::Push(HostValue::String("gold".into()))
        );
        tx.apply(&mut adapter).expect("apply host push patch");
        assert_eq!(
            adapter.read_path(&reward_path),
            Ok(HostValue::Array(vec![
                HostValue::String("xp".into()),
                HostValue::String("gold".into())
            ]))
        );
    }

    #[test]
    fn compiled_source_removes_host_path_through_patch_tx() {
        let host_ref = player_ref(3);
        let inventory = FieldId::new(8);
        let items = FieldId::new(9);
        let item_key = Symbol::new(NonZeroU32::new(1).expect("non-zero symbol"));
        let item_path = HostPath::new(host_ref)
            .field(inventory)
            .field(items)
            .key(item_key);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    let item_id = "gold";
    player.inventory.items[item_id].remove();
    return 1;
}
"#,
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_field("items", items),
        )
        .expect("compile host path remove source");
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(item_path.clone(), HostValue::String("gold".into()));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(1)));
        assert_eq!(
            adapter.read_path(&item_path),
            Ok(HostValue::String("gold".into()))
        );
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].path, item_path);
        assert_eq!(tx.patches()[0].op, PatchOp::Remove);
        tx.apply(&mut adapter).expect("apply host remove patch");
        assert!(matches!(
            adapter.read_path(&item_path),
            Err(error)
                if error.kind == (HostErrorKind::MissingPath {
                    path: item_path.clone()
                })
        ));
    }

    #[test]
    fn compiled_source_mutates_indexed_host_field_through_patch_tx() {
        let host_ref = player_ref(3);
        let inventory = FieldId::new(8);
        let items = FieldId::new(9);
        let count = FieldId::new(10);
        let item_key = Symbol::new(NonZeroU32::new(1).expect("non-zero symbol"));
        let item_count = HostPath::new(host_ref)
            .field(inventory)
            .field(items)
            .key(item_key)
            .field(count);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    let item_id = "gold";
    player.inventory.items[item_id].count += 1;
    return player.inventory.items[item_id].count;
}
"#,
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_field("items", items)
                .with_host_field("count", count),
        )
        .expect("compile indexed host field source");
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(item_count.clone(), HostValue::Int(4));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(5)));
        assert_eq!(adapter.read_path(&item_count), Ok(HostValue::Int(4)));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].path, item_count);
        assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
        tx.apply(&mut adapter).expect("apply indexed host patch");
        assert_eq!(adapter.read_path(&item_count), Ok(HostValue::Int(5)));
    }

    #[test]
    fn compiled_source_host_method_call_records_patch_tx() {
        let host_ref = player_ref(3);
        let method = HostMethodId::new(5);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.grant_exp(20);
    return 1;
}
"#,
            &CompilerOptions::new().with_host_method("grant_exp", method),
        )
        .expect("compile host method source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.insert_method_return(method, HostValue::Int(12));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(1)));
        assert!(adapter.method_calls().is_empty());
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method,
                args: vec![HostValue::Int(20)]
            }
        );
        tx.apply(&mut adapter).expect("apply host method patch");
        assert_eq!(
            adapter.method_calls(),
            &[(HostPath::new(host_ref), method, vec![HostValue::Int(20)])]
        );
    }

    #[test]
    fn compiled_source_host_field_method_call_records_path_patch_tx() {
        let host_ref = player_ref(3);
        let inventory = FieldId::new(8);
        let method = HostMethodId::new(9);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.inventory.add("gold", 100);
    return 1;
}
"#,
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_method("add", method),
        )
        .expect("compile host field method source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.insert_method_return(method, HostValue::Int(12));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(1)));
        assert!(adapter.method_calls().is_empty());
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method,
                args: vec![HostValue::String("gold".into()), HostValue::Int(100)]
            }
        );
        tx.apply(&mut adapter).expect("apply host method patch");
        assert_eq!(
            adapter.method_calls(),
            &[(
                HostPath::new(host_ref).field(inventory),
                method,
                vec![HostValue::String("gold".into()), HostValue::Int(100)]
            )]
        );
    }

    #[test]
    fn compiled_source_host_indexed_method_call_records_path_patch_tx() {
        let host_ref = player_ref(3);
        let inventory = FieldId::new(8);
        let items = FieldId::new(9);
        let method = HostMethodId::new(10);
        let item_key = Symbol::new(NonZeroU32::new(1).expect("non-zero symbol"));
        let item_path = HostPath::new(host_ref)
            .field(inventory)
            .field(items)
            .key(item_key);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    let item_id = "gold";
    player.inventory.items[item_id].grant(20);
    return 1;
}
"#,
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_field("items", items)
                .with_host_method("grant", method),
        )
        .expect("compile indexed host method source");
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(item_path.clone(), HostValue::Int(0));
        adapter.insert_method_return(method, HostValue::Null);
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(1)));
        assert!(adapter.method_calls().is_empty());
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].path, item_path);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method,
                args: vec![HostValue::Int(20)]
            }
        );
        tx.apply(&mut adapter)
            .expect("apply indexed host method patch");
        assert_eq!(
            adapter.method_calls(),
            &[(item_path, method, vec![HostValue::Int(20)])]
        );
    }

    #[test]
    fn compiled_source_context_time_and_emit_records_patch_tx() {
        let ctx_ref = HostRef::new(HostTypeId::new(9), HostObjectId::new(11), 1);
        let now_field = FieldId::new(6);
        let tick_field = FieldId::new(7);
        let emit_method = HostMethodId::new(8);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(ctx) {
    let stamp = ctx.now + ctx.tick;
    ctx.emit("player.level_checked", stamp);
    return stamp;
}
"#,
            &CompilerOptions::new()
                .with_host_field("now", now_field)
                .with_host_field("tick", tick_field)
                .with_host_method("emit", emit_method),
        )
        .expect("compile context source");
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(
            HostPath::new(ctx_ref).field(now_field),
            HostValue::Int(1000),
        );
        adapter.insert_value(HostPath::new(ctx_ref).field(tick_field), HostValue::Int(42));
        adapter.insert_method_return(emit_method, HostValue::Null);
        let mut tx = PatchTx::new();
        let mut budget = ExecutionBudget::new(10_000, 1024 * 1024, 64, 1024);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host_managed_heap_and_budget(
                &program,
                "main",
                &[Value::HostRef(ctx_ref)],
                &mut host,
                &mut budget,
            )
        };

        assert_eq!(result, Ok(Value::Int(1042)));
        assert!(adapter.method_calls().is_empty());
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method: emit_method,
                args: vec![
                    HostValue::String("player.level_checked".into()),
                    HostValue::Int(1042)
                ]
            }
        );
        tx.apply(&mut adapter).expect("apply context emit patch");
        assert_eq!(
            adapter.method_calls(),
            &[(
                HostPath::new(ctx_ref),
                emit_method,
                vec![
                    HostValue::String("player.level_checked".into()),
                    HostValue::Int(1042)
                ]
            )]
        );
    }

    #[test]
    fn compiled_source_uses_reflection_natives_for_host_state() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    if reflect.type_of(player) == "Player" {
        if reflect.implements(player, "Damageable") {
            reflect.set(player, "level", 10);
            return reflect.get(player, "level");
        }
    }
    return 0;
}
"#,
        )
        .expect("compile reflection source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        };

        assert_eq!(result, Ok(Value::Int(10)));
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(9))
        );
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
        tx.apply(&mut adapter).expect("apply reflection patch");
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(10))
        );
    }

    #[test]
    fn reflection_permissions_deny_writes_before_patches() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    reflect.set(player, "level", 10);
    return 1;
}
"#,
        )
        .expect("compile denied reflection write source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives_with_permissions(
            Arc::new(reflection_registry()),
            reflect::ReflectPermissionSet::read_only(),
        );
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert!(matches!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
                permission: reflect::ReflectPermission::WriteValueFields
            })
        ));
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn reflection_permissions_deny_calls_before_patches() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    reflect.call(player, "grant_exp", 10);
    return 1;
}
"#,
        )
        .expect("compile denied reflection call source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.insert_method_return(HostMethodId::new(5), HostValue::Null);
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives_with_permissions(
            Arc::new(reflection_registry()),
            reflect::ReflectPermissionSet::read_only(),
        );
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert!(matches!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
                permission: reflect::ReflectPermission::CallMethods
            })
        ));
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn reflection_permissions_deny_host_ref_metadata_without_inspection() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return reflect.type_of(player);
}
"#,
        )
        .expect("compile denied host-ref metadata source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives_with_permissions(
            Arc::new(reflection_registry()),
            reflect::ReflectPermissionSet::new().with(reflect::ReflectPermission::ReadTypeInfo),
        );
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert!(matches!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
                permission: reflect::ReflectPermission::InspectHostPath
            })
        ));
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn reflection_permissions_allow_script_metadata_without_host_inspection() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    return reflect.name(player);
}
"#,
        )
        .expect("compile script metadata source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives_with_permissions(
            Arc::new(script_reflection_registry()),
            reflect::ReflectPermissionSet::new().with(reflect::ReflectPermission::ReadTypeInfo),
        );
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Ok(Value::String("Player".into()))
        );
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn reflection_permissions_deny_function_metadata_without_function_permission() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    reflect.function("game.admin");
    return 1;
}
"#,
        )
        .expect("compile function metadata permission source");
        let mut registry = TypeRegistry::new();
        registry.register_function(
            FunctionDesc::new(FunctionId::new(9), "game.admin")
                .access(FunctionAccess::new().require_permission("game.admin")),
        );
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives_with_policy(
            Arc::new(registry),
            reflect::ReflectPolicy::new(
                reflect::ReflectPermissionSet::new().with(reflect::ReflectPermission::ReadTypeInfo),
            ),
        );
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        let error = vm
            .run_program_with_host(&program, "main", &[], &mut host)
            .expect_err("function metadata permission should be denied");
        assert_eq!(
            error.kind,
            VmErrorKind::Reflect(ReflectErrorKind::FunctionPermissionDenied {
                function: "game.admin".to_owned(),
                permission: "game.admin".to_owned(),
            })
        );
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn reflection_field_access_denies_hidden_host_field_reads() {
        let host_ref = player_ref(3);
        let secret_field = FieldId::new(77);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return reflect.get(player, "secret");
}
"#,
        )
        .expect("compile hidden field reflection source");
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .field(
                    FieldDesc::new(secret_field, "secret")
                        .access(FieldAccess::new().reflect_readable(false)),
                ),
        );
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(
            HostPath::new(host_ref).field(secret_field),
            HostValue::Int(99),
        );
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(registry));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        let error = vm
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
            .expect_err("hidden field read should be denied");
        assert_eq!(
            error.kind,
            VmErrorKind::Reflect(ReflectErrorKind::FieldNotReflectReadable {
                type_name: "Player".to_owned(),
                field: "secret".to_owned(),
            })
        );
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn reflection_lookup_budget_stops_after_limit() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    reflect.name(player);
    reflect.kind(player);
    return 1;
}
"#,
        )
        .expect("compile budgeted reflection source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives_with_policy(
            Arc::new(reflection_registry()),
            reflect::ReflectPolicy::all().with_lookup_limit(1),
        );
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert!(matches!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::LookupBudgetExceeded {
                limit: 1
            })
        ));
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn heap_execution_uses_reflection_natives_for_host_state() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    if reflect.type_of(player) == "Player" {
        if reflect.implements(player, "Damageable") {
            reflect.set(player, "level", 10);
            return reflect.get(player, "level");
        }
    }
    return 0;
}
"#,
        )
        .expect("compile reflection source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            vm.run_program_with_host_heap_and_budget(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
                &mut heap_execution,
                &mut budget,
            )
        };

        assert_eq!(result, Ok(Value::Int(10)));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    }

    #[test]
    fn compiled_source_reflection_fields_returns_metadata() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return reflect.fields(player);
}
"#,
        )
        .expect("compile reflection fields source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        let result =
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host);

        assert_eq!(
            result,
            Ok(Value::Array(vec![
                Value::String("id".into()),
                Value::String("level".into())
            ]))
        );
    }

    #[test]
    fn compiled_source_reflects_name_kind_and_field_metadata() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    let field = reflect.field(player, "level");
    if reflect.name(player) == "Player"
        && reflect.kind(player) == "host"
        && reflect.docs(player) == "A player host object."
        && reflect.attrs(player).get("domain") == "gameplay"
        && reflect.has_field(player, "level")
        && !reflect.has_field(player, "mana")
        && field.name == "level"
        && field.docs == "Current player level."
        && field.attrs.get("unit") == "level"
        && field.writable {
        return 1;
    }
    return 0;
}
"#,
        )
        .expect("compile field reflection source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Ok(Value::Int(1))
        );
    }

    #[test]
    fn compiled_source_reflect_fields_respect_field_access() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    let fields = reflect.fields(player);
    if reflect.has_field(player, "level")
        && !reflect.has_field(player, "secret")
        && reflect.field(player, "level").name == "level" {
        return fields.len();
    }
    return 0;
}
"#,
        )
        .expect("compile policy fields reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        let policy = reflect::ReflectPolicy::new(
            reflect::ReflectPermissionSet::new()
                .with(reflect::ReflectPermission::ReadTypeInfo)
                .with(reflect::ReflectPermission::InspectHostPath),
        );
        vm.register_reflection_natives_with_policy(
            Arc::new(policy_field_reflection_registry()),
            policy,
        );
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Ok(Value::Int(1))
        );
    }

    #[test]
    fn compiled_source_reflects_modules_functions_and_exports() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let module = reflect.module("game.reward");
    let exports = reflect.exports("game.reward");
    let function = reflect.function("game.reward.grant");
    if module.get("name") == "game.reward" && exports.len() == 1 && function.get("return") == "bool" {
        return function.get("params").len();
    }
    return 0;
}
"#,
        )
        .expect("compile module reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(script_module_reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Ok(Value::Int(2))
        );
    }

    #[test]
    fn compiled_source_reflect_exports_respect_function_policy() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let module = reflect.module("game.reward");
    let exports = reflect.exports("game.reward");
    return module.get("exports").len() * 10 + exports.len();
}
"#,
        )
        .expect("compile policy exports reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives_with_policy(
            Arc::new(policy_module_reflection_registry()),
            reflect::ReflectPolicy::read_only(),
        );
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Ok(Value::Int(11))
        );
    }

    #[test]
    fn compiled_source_reflects_methods_traits_and_variants() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    let methods = reflect.methods(player);
    let traits = reflect.traits(player);
    let quest = QuestProgress.Active { count: 1 };
    let variants = reflect.variants(quest);
    if reflect.has_method(player, "grant_exp")
        && methods.len() == 1
        && traits.len() == 1
        && variants.len() == 2
        && reflect.variant(quest) == "Active"
        && reflect.variant_is(quest, "Active") {
        return variants[0].fields.len();
    }
    return 0;
}
"#,
        )
        .expect("compile member reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(member_reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(player_ref(3))],
                &mut host
            ),
            Ok(Value::Int(1))
        );
    }

    #[test]
    fn compiled_source_reflects_registered_trait_metadata() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let trait_info = reflect.trait_info("Damageable");
    if trait_info.name == "Damageable" && trait_info.methods[0].name == "damage" {
        return trait_info.methods.len();
    }
    return 0;
}
"#,
        )
        .expect("compile trait metadata reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Ok(Value::Int(1))
        );
    }

    #[test]
    fn compiled_source_reflects_registered_type_metadata() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let names = reflect.types();
    let player = reflect.type_info("Player");
    if names.len() == 1
        && names[0] == "Player"
        && player.kind == "host"
        && player.field_count == 2
        && player.method_count == 1
        && player.trait_count == 1 {
        return player.name;
    }
    return "missing";
}
"#,
        )
        .expect("compile type metadata reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Ok(Value::String("Player".to_owned()))
        );
    }

    #[test]
    fn compiled_source_reflect_type_reports_unknown_type_candidates() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    return reflect.type_info("Plyer");
}
"#,
        )
        .expect("compile unknown type metadata source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert!(matches!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::UnknownTypeName {
                type_name: "Plyer".to_owned(),
                candidates: vec!["Player".to_owned()]
            })
        ));
    }

    #[test]
    fn compiled_source_reflect_trait_reports_unknown_trait_candidates() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    return reflect.trait_info("Damagable");
}
"#,
        )
        .expect("compile unknown trait metadata source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert!(matches!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::UnknownTrait {
                trait_name: "Damagable".to_owned(),
                candidates: vec!["Damageable".to_owned()]
            })
        ));
    }

    #[test]
    fn compiled_source_reflect_variants_respect_field_access() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let quest = QuestProgress.Active { count: 1 };
    let variants = reflect.variants(quest);
    if variants[0].fields.len() == 1 && variants[0].fields[0].name == "count" {
        return variants.len();
    }
    return 0;
}
"#,
        )
        .expect("compile policy variant reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(member_reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Ok(Value::Int(2))
        );
    }

    #[test]
    fn compiled_source_reflect_methods_respect_method_policy() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    let methods = reflect.methods(player);
    if reflect.has_method(player, "visible")
        && !reflect.has_method(player, "hidden")
        && !reflect.has_method(player, "private")
        && !reflect.has_method(player, "admin") {
        return methods.len();
    }
    return 0;
}
"#,
        )
        .expect("compile policy methods reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        let policy = reflect::ReflectPolicy::new(
            reflect::ReflectPermissionSet::new()
                .with(reflect::ReflectPermission::ReadTypeInfo)
                .with(reflect::ReflectPermission::InspectHostPath),
        );
        vm.register_reflection_natives_with_policy(
            Arc::new(policy_method_reflection_registry()),
            policy,
        );
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Ok(Value::Int(1))
        );
    }

    #[test]
    fn compiled_source_reflect_variant_is_reports_unknown_variant_candidates() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let quest = QuestProgress.Active { count: 1 };
    return reflect.variant_is(quest, "Actve");
}
"#,
        )
        .expect("compile unknown variant reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(member_reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert!(matches!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::UnknownVariant {
                type_name: "QuestProgress".to_owned(),
                variant: "Actve".to_owned(),
                candidates: vec!["Active".to_owned(), "Finished".to_owned()]
            })
        ));
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn compiled_source_reflect_implements_reports_unknown_trait_candidates() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return reflect.implements(player, "Damagable");
}
"#,
        )
        .expect("compile unknown trait reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert!(matches!(
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
            Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::UnknownTrait {
                trait_name: "Damagable".to_owned(),
                candidates: vec!["Damageable".to_owned()]
            })
        ));
        assert!(tx.patches().is_empty());
    }

    #[test]
    fn compiled_source_reflects_script_record_implements() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    if reflect.type_of(player) == "Player" && reflect.implements(player, "Damageable") {
        return reflect.get(player, "level") + reflect.fields(player).len();
    }
    return 0;
}
"#,
        )
        .expect("compile script record reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(script_reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "main", &[], &mut host),
            Ok(Value::Int(8))
        );
    }

    #[test]
    fn heap_execution_reflection_fields_returns_heap_metadata_array() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return reflect.fields(player);
}
"#,
        )
        .expect("compile reflection fields source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            vm.run_program_with_host_heap_and_budget(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
                &mut heap_execution,
                &mut budget,
            )
        }
        .expect("run heap reflection fields");

        let Value::HeapRef(fields_ref) = result else {
            panic!("expected heap metadata array");
        };
        let Some(HeapValue::Array(fields)) = heap_execution.heap.get(fields_ref).cloned() else {
            panic!("expected heap metadata array object");
        };
        let field_names = fields
            .iter()
            .map(|slot| materialize_heap_slot(slot, Some(&heap_execution)))
            .collect::<VmResult<Vec<_>>>()
            .expect("materialize field names");

        assert_eq!(
            field_names,
            vec![Value::String("id".into()), Value::String("level".into())]
        );
    }

    #[test]
    fn heap_execution_reflects_script_record_implements() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    if reflect.type_of(player) == "Player" && reflect.implements(player, "Damageable") {
        return reflect.get(player, "level") + reflect.fields(player).len();
    }
    return 0;
}
"#,
        )
        .expect("compile heap script record reflection source");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(script_reflection_registry()));
        let mut budget = ExecutionBudget::unbounded();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            vm.run_program_with_host_managed_heap_and_budget(
                &program,
                "main",
                &[],
                &mut host,
                &mut budget,
            )
        };

        assert_eq!(result, Ok(Value::Int(8)));
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn compiled_module_reflects_registered_script_trait_impls() {
        let sources = [ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game"),
            r#"
trait Damageable {
    fn damage(self) -> int { return self.level; }
}
struct Player { level: int }

impl Damageable for Player {}

pub fn main() {
    let player = Player { level: 7 };
    if reflect.type_of(player) == "game.Player" && reflect.implements(player, "game.Damageable") {
        return player.damage() + reflect.fields(player).len();
    }
    return 0;
}
"#,
        )];
        let mut graph = ModuleGraph::new();
        for source in &sources {
            graph.add_source(source.clone());
        }
        graph.resolve_imports();
        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let mut registry = TypeRegistry::new();
        registry.register_script_types(&graph);
        let program = compile_module_sources(&sources).expect("compile script trait module");
        let mut adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(registry));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        assert_eq!(
            vm.run_program_with_host(&program, "game.main", &[], &mut host),
            Ok(Value::Int(8))
        );
    }

    #[test]
    fn compiled_source_reflect_call_records_host_method_patch() {
        let host_ref = player_ref(3);
        let method = HostMethodId::new(5);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    reflect.call(player, "grant_exp", 20);
    return 1;
}
"#,
        )
        .expect("compile reflection call source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.insert_method_return(method, HostValue::Int(12));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        };

        assert_eq!(result, Ok(Value::Int(1)));
        assert!(adapter.method_calls().is_empty());
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method,
                args: vec![HostValue::Int(20)]
            }
        );
        tx.apply(&mut adapter).expect("apply reflection call");
        assert_eq!(
            adapter.method_calls(),
            &[(HostPath::new(host_ref), method, vec![HostValue::Int(20)])]
        );
    }

    #[test]
    fn call_host_method_records_patch_and_applies_later() {
        let host_ref = player_ref(3);
        let method = HostMethodId::new(8);
        let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
        let gold = code.push_constant(Constant::String("gold".into()));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: gold,
        }));
        code.push_instruction(Instruction::new(InstructionKind::CallHostMethod {
            dst: Some(Register(2)),
            root: Register(0),
            segments: Vec::new(),
            method,
            args: vec![Register(1)],
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.insert_method_return(method, HostValue::Int(12));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(12)));
        assert!(adapter.method_calls().is_empty());
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method,
                args: vec![HostValue::String("gold".into())]
            }
        );
        tx.apply(&mut adapter).expect("apply method call");
        assert_eq!(
            adapter.method_calls(),
            &[(
                HostPath::new(host_ref),
                method,
                vec![HostValue::String("gold".into())]
            )]
        );
    }

    #[test]
    fn heap_execution_converts_heap_string_for_host_method_call() {
        let host_ref = player_ref(3);
        let method = HostMethodId::new(8);
        let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
        let gold = code.push_constant(Constant::String("gold".into()));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: gold,
        }));
        code.push_instruction(Instruction::new(InstructionKind::CallHostMethod {
            dst: Some(Register(2)),
            root: Register(0),
            segments: Vec::new(),
            method,
            args: vec![Register(1)],
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.insert_method_return(method, HostValue::Null);
        let mut tx = PatchTx::new();
        let mut heap = ScriptHeap::new();
        let mut heap_execution = HeapExecution::new(&mut heap);
        let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host_heap_and_budget(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
                &mut heap_execution,
                &mut budget,
            )
        };

        assert_eq!(result, Ok(Value::Null));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method,
                args: vec![HostValue::String("gold".into())]
            }
        );
    }

    #[test]
    fn compiled_source_host_method_call_returns_copied_preview_value() {
        let host_ref = player_ref(3);
        let method = HostMethodId::new(5);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    return player.grant_exp(20);
}
"#,
            &CompilerOptions::new().with_host_method("grant_exp", method),
        )
        .expect("compile host method return source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.insert_method_return(method, HostValue::String("accepted".into()));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::String("accepted".into())));
        assert!(adapter.method_calls().is_empty());
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method,
                args: vec![HostValue::Int(20)]
            }
        );
    }

    fn host_read_program() -> (Program, HostRef) {
        let host_ref = player_ref(3);
        let mut code = CodeObject::new("main", 2).with_params(vec!["player".into()]);
        code.push_instruction(Instruction::new(InstructionKind::GetHostField {
            dst: Register(1),
            root: Register(0),
            field: level_field(),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        (program, host_ref)
    }

    fn host_adapter(host_ref: HostRef, value: HostValue) -> MockStateAdapter {
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(level_path(host_ref), value);
        adapter
    }

    fn reflection_registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register_trait(
            TraitDesc::new("Damageable")
                .method(TraitMethodDesc::new(MethodId::new(1), "damage").defaulted(true)),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .docs("A player host object.")
                .attr("domain", "gameplay")
                .field(FieldDesc::new(FieldId::new(1), "id"))
                .field(
                    FieldDesc::new(level_field(), "level")
                        .writable(true)
                        .docs("Current player level.")
                        .attr("unit", "level"),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(5), "grant_exp")
                        .docs("Grant experience.")
                        .attr("effect", "write"),
                )
                .trait_impl(TraitDesc::new("Damageable")),
        );
        registry
    }

    fn script_reflection_registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(200), "Player"))
                .kind(TypeKind::ScriptStruct)
                .field(FieldDesc::new(FieldId::new(20), "level"))
                .trait_impl(TraitDesc::new("Damageable")),
        );
        registry
    }

    fn script_module_reflection_registry() -> TypeRegistry {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.reward"),
            r#"
pub fn grant(player: Player, amount: int = 1) -> bool {
    return true;
}
"#,
        ));
        let mut registry = TypeRegistry::new();
        registry.register_script_modules(&graph);
        registry
    }

    fn policy_module_reflection_registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register_module(ModuleDesc::new("game.reward"));
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "game.reward.grant").module("game.reward"),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(2), "game.reward.hidden")
                .module("game.reward")
                .access(FunctionAccess::new().reflect_visible(false)),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(3), "game.reward.private")
                .module("game.reward")
                .access(FunctionAccess::new().public(false).reflect_visible(true)),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(4), "game.reward.admin")
                .module("game.reward")
                .access(FunctionAccess::new().require_permission("game.admin")),
        );
        registry
    }

    fn policy_method_reflection_registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(500), "Player"))
                .host_type(HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(1), "visible"))
                .method(
                    MethodDesc::new(HostMethodId::new(2), "hidden")
                        .access(MethodAccess::new().reflect_callable(false)),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(3), "private")
                        .access(MethodAccess::new().public(false).reflect_callable(true)),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(4), "admin")
                        .access(MethodAccess::new().require_permission("player.admin")),
                ),
        );
        registry
    }

    fn policy_field_reflection_registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(600), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(FieldId::new(1), "level"))
                .field(
                    FieldDesc::new(FieldId::new(2), "secret")
                        .access(FieldAccess::new().reflect_readable(false)),
                ),
        );
        registry
    }

    fn member_reflection_registry() -> TypeRegistry {
        let mut registry = reflection_registry();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(300), "QuestProgress"))
                .kind(TypeKind::ScriptEnum)
                .variant(
                    VariantDesc::new(VariantId::new(10), "Active")
                        .field(FieldDesc::new(FieldId::new(11), "count"))
                        .field(
                            FieldDesc::new(FieldId::new(13), "secret")
                                .access(FieldAccess::new().reflect_readable(false)),
                        ),
                )
                .variant(VariantDesc::new(VariantId::new(12), "Finished")),
        );
        registry
    }

    fn player_ref(generation: u32) -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), generation)
    }

    fn level_path(host_ref: HostRef) -> HostPath {
        HostPath::new(host_ref).field(level_field())
    }

    fn level_field() -> FieldId {
        FieldId::new(2)
    }
}
