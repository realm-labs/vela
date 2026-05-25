//! Register VM for Vela bytecode.

#![allow(clippy::result_large_err)]

mod array_methods;
mod budget;
mod error;
pub mod heap;
mod heap_execution;
mod host_values;
mod indexing;
mod iteration;
mod map_methods;
mod math_stdlib;
mod method_runtime;
mod option_result;
mod ranges;
mod record_fields;
mod reflection;
mod script_methods;
mod script_object;
mod set_methods;
mod stdlib;
mod string_methods;
mod try_propagation;
mod value;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub use error::{VmError, VmErrorKind, VmResult, VmStackFrame};
use heap::{GcRef, HeapSlot, HeapValue, ScriptHeap};
pub use heap_execution::HeapExecution;
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
use vela_host::{HostPath, HostRef, PatchTx, ScriptStateAdapter};
use vela_reflect::{self as reflect, TypeRegistry};

pub use budget::{ExecutionBudget, ExecutionBudgetKind};
pub use value::{ClosureValue, Value};

struct ExecutionCall<'a> {
    code: &'a CodeObject,
    program: Option<&'a Program>,
    captures: &'a [Value],
    args: &'a [Value],
    call_site: Option<Span>,
}

impl ExecutionCall<'_> {
    fn stack_frame(&self) -> VmStackFrame {
        VmStackFrame::new(self.code.name.clone(), self.call_site)
    }
}

pub type NativeFunction = Arc<dyn Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static>;
pub type HostNativeFunction = Arc<
    dyn for<'host, 'budget> Fn(
            &[Value],
            &mut HostExecution<'host>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<Value>
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
        self.host_natives.insert(
            name.into(),
            Arc::new(move |args, host, _budget| function(args, host)),
        );
    }

    pub fn register_budgeted_host_native(
        &mut self,
        name: impl Into<String>,
        function: impl for<'host, 'budget> Fn(
            &[Value],
            &mut HostExecution<'host>,
            Option<&'budget mut ExecutionBudget>,
        ) -> VmResult<Value>
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
                call_site: None,
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
                call_site: None,
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
                        call_stack: Default::default(),
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
                        native(&values, host, budget.as_deref_mut())?
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
                    let result = self.execute_call(
                        ExecutionCall {
                            code: function,
                            program: Some(program),
                            captures: &[],
                            args: &values,
                            call_site: instruction.span,
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
                            call_site: instruction.span,
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
                call_stack: Default::default(),
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
        Value::Range(_)
        | Value::Closure(_)
        | Value::Iterator(_)
        | Value::PathProxy(_)
        | Value::Missing => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "heap slot",
        })),
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
        | Value::HostRef(_)
        | Value::PathProxy(_) => Ok(value.clone()),
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

pub(crate) fn values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
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
        | Value::PathProxy(_)
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
        Value::Array(_)
        | Value::Set(_)
        | Value::Range(_)
        | Value::Closure(_)
        | Value::PathProxy(_)
        | Value::Missing => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
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
        | Value::HostRef(_)
        | Value::PathProxy(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
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
mod tests;
