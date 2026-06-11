use vela_bytecode::{LinkedProgram, UnlinkedProgramCode};

use crate::heap::GcRef;
use crate::linked_execution::LinkedExecutionCall;
use crate::runtime_checks::expect_closure_ref;
use crate::value::ClosureCode;
use crate::{
    ExecutionBudget, ExecutionCall, HeapExecution, HostExecution, Value, Vm, VmBytecodeProfiler,
    VmError, VmErrorKind, VmInlineCaches, VmResult,
};

pub(crate) struct MethodRuntime<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: Option<&'a dyn UnlinkedProgramCode>,
    pub(crate) linked_program: Option<&'a LinkedProgram>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) caller_roots: &'a [GcRef],
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
}

pub(crate) fn call_callback(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    args: &[Value],
    protected_values: &[Value],
) -> VmResult<Value> {
    call_callback_with_protected_values(runtime, operation, callback, args, protected_values.iter())
}

pub(crate) fn callback_param_len(
    runtime: &MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
) -> VmResult<usize> {
    let closure = expect_closure_ref(callback, runtime.heap.as_deref(), operation)?;
    match &closure.code {
        ClosureCode::Unlinked(code) => Ok(code.params.len()),
        ClosureCode::Linked(function) => {
            let program = runtime
                .linked_program
                .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            let code = program.function(*function).ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownFunction {
                    name: format!("<linked closure#{}>", function.index()),
                })
            })?;
            Ok(code.params.len())
        }
    }
}

pub(crate) fn call_callback_with_protected_values<'value>(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    args: &[Value],
    protected_values: impl IntoIterator<Item = &'value Value>,
) -> VmResult<Value> {
    let (code, captures) = {
        let closure = expect_closure_ref(callback, runtime.heap.as_deref(), operation)?;
        let captures = closure.captures.clone();
        (closure.code.clone(), captures)
    };
    let protected_root_len = runtime.heap.as_deref_mut().map(|heap| {
        let protected_root_len = heap.push_protected_roots(runtime.caller_roots);
        heap.protect_values(args);
        heap.protect_value_refs(protected_values);
        protected_root_len
    });
    let result = match code {
        ClosureCode::Unlinked(code) => runtime.vm.execute_call(
            ExecutionCall {
                code: &code,
                program: runtime.program,
                captures: captures.as_slice(),
                args,
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: runtime.inline_caches,
            },
            runtime.host.as_deref_mut(),
            runtime.heap.as_deref_mut(),
            runtime.budget.as_deref_mut(),
        ),
        ClosureCode::Linked(function) => {
            let program = runtime
                .linked_program
                .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            let code = program.function(function).ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownFunction {
                    name: format!("<linked closure#{}>", function.index()),
                })
            })?;
            runtime.vm.execute_linked_call(
                LinkedExecutionCall {
                    code,
                    program,
                    captures: captures.as_slice(),
                    args,
                    check_param_guards: true,
                    call_site: None,
                    call_site_offset: None,
                    inline_caches: runtime.inline_caches,
                    bytecode_profiler: runtime.bytecode_profiler,
                },
                runtime.host.as_deref_mut(),
                runtime.heap.as_deref_mut(),
                runtime.budget.as_deref_mut(),
            )
        }
    };
    if let (Some(heap), Some(protected_root_len)) =
        (runtime.heap.as_deref_mut(), protected_root_len)
    {
        heap.truncate_protected_roots(protected_root_len);
    }
    result
}
