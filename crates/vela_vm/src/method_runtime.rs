use vela_bytecode::LinkedProgram;

use crate::runtime_checks::expect_closure_ref;
use crate::{ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) struct MethodRuntime<'a, 'heap> {
    pub(crate) program: &'a LinkedProgram,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
}

pub(crate) fn call_callback(
    runtime: &mut MethodRuntime<'_, '_>,
    operation: &'static str,
    callback: &Value,
    args: &[Value],
    protected_values: &[Value],
) -> VmResult<Value> {
    call_callback_with_protected_values(runtime, operation, callback, args, protected_values.iter())
}

pub(crate) fn callback_param_len(
    runtime: &MethodRuntime<'_, '_>,
    operation: &'static str,
    callback: &Value,
) -> VmResult<usize> {
    let closure = expect_closure_ref(callback, runtime.heap.as_deref(), operation)?;
    let code = closure.owner.function(closure.function).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: format!("<linked closure#{}>", closure.function.index()),
        })
    })?;
    Ok(code.params.len())
}

pub(crate) fn call_callback_with_protected_values<'value>(
    _runtime: &mut MethodRuntime<'_, '_>,
    _operation: &'static str,
    _callback: &Value,
    _args: &[Value],
    _protected_values: impl IntoIterator<Item = &'value Value>,
) -> VmResult<Value> {
    Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
        opcode: "callback escaped the execution session",
    }))
}
