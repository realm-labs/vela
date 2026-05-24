//! Register VM for Vela bytecode.

pub mod heap;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

use heap::{GcBudget, GcRef, GcStepStats, HeapSlot, HeapValue, ScriptHeap};
use vela_bytecode::{CodeObject, Constant, InstructionKind, Program, Register};
use vela_host::{
    HostError, HostErrorKind, HostPath, HostRef, HostValue, PatchTx, ScriptStateAdapter,
};
use vela_reflect::{self as reflect, ReflectError, ReflectErrorKind, TypeRegistry};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Record {
        type_name: String,
        fields: BTreeMap<String, Value>,
    },
    Enum {
        enum_name: String,
        variant: String,
        fields: BTreeMap<String, Value>,
    },
    HeapRef(GcRef),
    HostRef(HostRef),
}

impl Value {
    pub fn trace_heap_refs(&self, refs: &mut Vec<GcRef>) {
        match self {
            Self::HeapRef(reference) => refs.push(*reference),
            Self::Array(values) => values.iter().for_each(|value| value.trace_heap_refs(refs)),
            Self::Map(values) => values
                .values()
                .for_each(|value| value.trace_heap_refs(refs)),
            Self::Record { fields, .. } | Self::Enum { fields, .. } => {
                fields
                    .values()
                    .for_each(|value| value.trace_heap_refs(refs));
            }
            Self::Null
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Float(_)
            | Self::String(_)
            | Self::HostRef(_) => {}
        }
    }
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
}

impl VmError {
    fn new(kind: VmErrorKind) -> Self {
        Self { kind }
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
    UnknownFunction {
        name: String,
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
        Self::new(VmErrorKind::Host(value.kind))
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

    pub fn register_reflection_natives(&mut self, registry: Arc<TypeRegistry>) {
        let type_of_registry = Arc::clone(&registry);
        self.register_host_native("reflect.type_of", move |args, _host| {
            expect_arity("reflect.type_of", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.type_of")?;
            Ok(reflect::type_of(&type_of_registry, &target)
                .map_or(Value::Null, |desc| Value::String(desc.key.name.clone())))
        });

        let fields_registry = Arc::clone(&registry);
        self.register_host_native("reflect.fields", move |args, _host| {
            expect_arity("reflect.fields", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.fields")?;
            let Some(desc) = reflect::type_of(&fields_registry, &target) else {
                return Ok(Value::Null);
            };
            let fields = reflect::fields(&fields_registry, &desc.key)
                .unwrap_or(&[])
                .iter()
                .map(|field| Value::String(field.name.clone()))
                .collect();
            Ok(Value::Array(fields))
        });

        let get_registry = Arc::clone(&registry);
        self.register_host_native("reflect.get", move |args, host| {
            expect_arity("reflect.get", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.get")?;
            let field = expect_string(&args[1], "reflect.get")?;
            let adapter: &dyn ScriptStateAdapter = &*host.adapter;
            let mut ctx = reflect::ReflectContext {
                registry: &get_registry,
                adapter,
                tx: &mut *host.tx,
            };
            let value = reflect::get(&mut ctx, &target, field)?;
            value_from_reflect(value)
        });

        let set_registry = Arc::clone(&registry);
        self.register_host_native("reflect.set", move |args, host| {
            expect_arity("reflect.set", args, 3)?;
            let target = value_to_reflect(&args[0], "reflect.set")?;
            let field = expect_string(&args[1], "reflect.set")?;
            let value = value_to_reflect(&args[2], "reflect.set")?;
            let adapter: &dyn ScriptStateAdapter = &*host.adapter;
            let mut ctx = reflect::ReflectContext {
                registry: &set_registry,
                adapter,
                tx: &mut *host.tx,
            };
            reflect::set(&mut ctx, &target, field, value)?;
            Ok(Value::Null)
        });

        let call_registry = Arc::clone(&registry);
        self.register_host_native("reflect.call", move |args, host| {
            if args.len() < 2 {
                return Err(VmError::new(VmErrorKind::ArityMismatch {
                    name: "reflect.call".to_owned(),
                    expected: 2,
                    actual: args.len(),
                }));
            }
            let target = value_to_reflect(&args[0], "reflect.call")?;
            let method = expect_string(&args[1], "reflect.call")?;
            let call_args = args[2..]
                .iter()
                .map(|arg| value_to_reflect(arg, "reflect.call"))
                .collect::<VmResult<Vec<_>>>()?;
            let adapter: &dyn ScriptStateAdapter = &*host.adapter;
            let mut ctx = reflect::ReflectContext {
                registry: &call_registry,
                adapter,
                tx: &mut *host.tx,
            };
            let value = reflect::call(&mut ctx, &target, method, call_args)?;
            value_from_reflect(value)
        });

        self.register_host_native("reflect.implements", move |args, _host| {
            expect_arity("reflect.implements", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.implements")?;
            let trait_name = expect_string(&args[1], "reflect.implements")?;
            Ok(Value::Bool(reflect::implements(
                &registry, &target, trait_name,
            )?))
        });
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
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        if let Some(budget) = &mut budget {
            budget.enter_call()?;
        }
        let result = self.execute_body(code, program, args, host, heap, budget.as_deref_mut());
        if let Some(budget) = budget {
            budget.exit_call();
        }
        result
    }

    fn execute_body(
        &self,
        code: &CodeObject,
        program: Option<&Program>,
        args: &[Value],
        mut host: Option<&mut HostExecution<'_>>,
        mut heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        if code.params.len() != args.len() {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: code.name.clone(),
                expected: code.params.len(),
                actual: args.len(),
            }));
        }

        let mut frame = CallFrame::new(code.register_count);
        for (index, arg) in args.iter().enumerate() {
            frame.write(
                Register(u16::try_from(index).map_err(|_| {
                    VmError::new(VmErrorKind::RegisterOutOfBounds {
                        register: Register(u16::MAX),
                    })
                })?),
                arg.clone(),
            )?;
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
                        .map(|register| frame.read(*register).cloned())
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
                InstructionKind::MakeRecord {
                    dst,
                    type_name,
                    fields,
                } => {
                    let mut values = BTreeMap::new();
                    for (name, register) in fields {
                        values.insert(name.clone(), frame.read(*register)?.clone());
                    }
                    let value = if let Some(heap) = heap.as_deref_mut() {
                        let slots = values_to_heap_map(&values, heap, budget.as_deref_mut())?;
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
                    let mut values = BTreeMap::new();
                    for (name, register) in fields {
                        values.insert(name.clone(), frame.read(*register)?.clone());
                    }
                    let value = if let Some(heap) = heap.as_deref_mut() {
                        let slots = values_to_heap_map(&values, heap, budget.as_deref_mut())?;
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
                InstructionKind::GetEnumField { dst, value, field } => {
                    let value = get_enum_field_value(frame.read(*value)?, field, heap.as_deref())?;
                    frame.write(*dst, value)?;
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
                    let value = host.tx.read_path(host.adapter, &path)?;
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
                    let base_value = host.tx.read_path(host.adapter, &path)?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx
                        .add_path(path, value, base_value, instruction.span)?;
                }
                InstructionKind::CallHostMethod {
                    dst,
                    root,
                    method,
                    args,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "call_host_method")?;
                    let path = HostPath::new(root);
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
                    host.tx
                        .call_method(path, *method, values, instruction.span)?;
                    if let Some(dst) = dst {
                        frame.write(*dst, Value::Null)?;
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

fn value_to_heap_slot(
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
            let slots = values_to_heap_map(fields, heap, budget.as_deref_mut())?;
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
            let slots = values_to_heap_map(fields, heap, budget.as_deref_mut())?;
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
        Value::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), materialize_value(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(Value::Map),
        Value::Record { type_name, fields } => fields
            .iter()
            .map(|(key, value)| Ok((key.clone(), materialize_value(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(|fields| Value::Record {
                type_name: type_name.clone(),
                fields,
            }),
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.clone(), materialize_value(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(|fields| Value::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields,
            }),
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::HostRef(_) => Ok(value.clone()),
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
            .map(|(key, value)| Ok((key.clone(), materialize_heap_slot(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(|fields| Value::Record {
                type_name: type_name.clone(),
                fields,
            }),
        HeapValue::Enum {
            enum_name,
            variant,
            fields,
        } => fields
            .iter()
            .map(|(key, value)| Ok((key.clone(), materialize_heap_slot(value, heap)?)))
            .collect::<VmResult<BTreeMap<_, _>>>()
            .map(|fields| Value::Enum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields,
            }),
        HeapValue::Set(values) => values
            .iter()
            .map(|value| materialize_heap_slot(value, heap))
            .collect::<VmResult<Vec<_>>>()
            .map(Value::Array),
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
        | Value::HostRef(_) => Ok(value),
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

fn value_from_host(value: HostValue) -> Value {
    match value {
        HostValue::Null => Value::Null,
        HostValue::Bool(value) => Value::Bool(value),
        HostValue::Int(value) => Value::Int(value),
        HostValue::Float(value) => Value::Float(value),
        HostValue::String(value) => Value::String(value),
    }
}

fn value_to_host(
    value: &Value,
    operation: &'static str,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<HostValue> {
    match value {
        Value::Null => Ok(HostValue::Null),
        Value::Bool(value) => Ok(HostValue::Bool(*value)),
        Value::Int(value) => Ok(HostValue::Int(*value)),
        Value::Float(value) => Ok(HostValue::Float(*value)),
        Value::String(value) => Ok(HostValue::String(value.clone())),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(HostValue::String(value.clone())),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        },
        Value::Array(_)
        | Value::Map(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::HostRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn value_to_reflect(value: &Value, operation: &'static str) -> VmResult<reflect::ReflectValue> {
    match value {
        Value::HostRef(host_ref) => Ok(reflect::ReflectValue::HostRef(*host_ref)),
        Value::Map(values) | Value::Record { fields: values, .. } => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.clone(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::ReflectValue::Record(values))
        }
        Value::Array(_) | Value::Enum { .. } => {
            Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
        }
        Value::HeapRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
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
    }
}

fn expect_string<'a>(value: &'a Value, operation: &'static str) -> VmResult<&'a str> {
    match value {
        Value::String(value) => Ok(value),
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::Array(_)
        | Value::Map(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::HeapRef(_)
        | Value::HostRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
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
    !matches!(value, Value::Null | Value::Bool(false))
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
    use std::sync::Arc;
    use vela_bytecode::compiler::{
        CompilerOptions, compile_function_source, compile_module_sources, compile_program_source,
        compile_program_source_with_options,
    };
    use vela_bytecode::{ConstantId, Instruction, InstructionOffset};
    use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
    use vela_hir::{ModulePath, ModuleSource};
    use vela_host::{HostValue, MockStateAdapter, PatchOp};
    use vela_reflect::{FieldDesc, MethodDesc, TraitDesc, TypeDesc, TypeKey};

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
                    fields,
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
                fields,
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
                fields,
            })
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
                fields,
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
        adapter.insert_method_return(method, HostValue::Null);
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

        assert_eq!(result, Ok(Value::Null));
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
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(FieldId::new(1), "id"))
                .field(FieldDesc::new(level_field(), "level").writable(true))
                .method(MethodDesc::new(HostMethodId::new(5), "grant_exp"))
                .trait_impl(TraitDesc::new("Damageable")),
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
