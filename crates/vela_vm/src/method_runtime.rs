use vela_bytecode::LinkedProgram;
use vela_host::resolved::ResolvedHostAccess;
use vela_host::target::{HostPathArg, HostTargetInstance, HostTargetPlan};

use crate::runtime_checks::expect_closure_ref;
use crate::{ExecutionBudget, HeapExecution, HostExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) trait HostIteratorAccess {
    fn read_index(
        &mut self,
        root: vela_host::path::HostRef,
        target: &HostTargetPlan,
        access: ResolvedHostAccess,
        index: u32,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value>;
}

impl HostIteratorAccess for HostExecution<'_> {
    fn read_index(
        &mut self,
        root: vela_host::path::HostRef,
        target: &HostTargetPlan,
        access: ResolvedHostAccess,
        index: u32,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        let args = [HostPathArg::Index(index)];
        let instance = HostTargetInstance::new(root, target, &args);
        let value = self
            .access
            .read_resolved(self.adapter, access, instance, None)?;
        crate::host_access::runtime_value_from_host(value, heap, budget, self)
    }
}

pub(crate) struct MethodRuntime<'a, 'heap, 'host> {
    pub(crate) program: &'a LinkedProgram,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) host: Option<&'a mut (dyn HostIteratorAccess + 'host)>,
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
    let code = closure.owner.function(closure.function).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: format!("<linked closure#{}>", closure.function.index()),
        })
    })?;
    Ok(code.params.len())
}

pub(crate) fn call_callback_with_protected_values<'value>(
    _runtime: &mut MethodRuntime<'_, '_, '_>,
    _operation: &'static str,
    _callback: &Value,
    _args: &[Value],
    _protected_values: impl IntoIterator<Item = &'value Value>,
) -> VmResult<Value> {
    Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
        opcode: "callback escaped the execution session",
    }))
}
