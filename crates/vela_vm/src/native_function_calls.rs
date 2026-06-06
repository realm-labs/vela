use vela_bytecode::Register;
use vela_common::{FunctionId, Span};

use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, HostNativeFunction, NativeFunction,
    OwnedValue, SmallStorage, Vm, VmError, VmErrorKind, VmResult, owned_to_value, value_to_owned,
};

pub(crate) struct NativeFunctionCall<'a> {
    pub(crate) dst: Option<Register>,
    pub(crate) name: &'a str,
    pub(crate) native: Option<FunctionId>,
    pub(crate) args: &'a [Register],
    pub(crate) call_site: Option<Span>,
}

enum NativeCallTarget<'a> {
    Pure(&'a NativeFunction),
    Host(&'a HostNativeFunction),
}

pub(crate) fn dispatch_native_function_call(
    vm: &Vm,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: NativeFunctionCall<'_>,
) -> VmResult<()> {
    let values = native_call_args_from_registers(frame, call.args, heap.as_deref())?;
    let target = resolve_native_call_target(vm, call.name, call.native);
    let result = match target {
        Some(NativeCallTarget::Pure(native)) => native(values.as_slice())
            .map_err(|error| error.with_source_span_if_absent(call.call_site))?,
        Some(NativeCallTarget::Host(native)) => {
            let host = host.as_deref_mut().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host context",
                })
            })?;
            native(values.as_slice(), host, budget.as_deref_mut())
                .map_err(|error| error.with_source_span_if_absent(call.call_site))?
        }
        None => {
            return Err(VmError::new(VmErrorKind::UnknownNative {
                name: call.name.to_owned(),
            })
            .with_source_span_if_absent(call.call_site));
        }
    };
    if let Some(dst) = call.dst {
        let result = owned_to_value(
            result,
            heap.as_deref_mut().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "native heap",
                })
            })?,
            budget.as_deref_mut(),
        )?;
        frame.write(dst, result)?;
    }
    Ok(())
}

fn resolve_native_call_target<'a>(
    vm: &'a Vm,
    name: &str,
    native: Option<FunctionId>,
) -> Option<NativeCallTarget<'a>> {
    native
        .and_then(|id| {
            vm.native_ids
                .get(&id)
                .map(NativeCallTarget::Pure)
                .or_else(|| vm.host_native_ids.get(&id).map(NativeCallTarget::Host))
        })
        .or_else(|| vm.natives.get(name).map(NativeCallTarget::Pure))
        .or_else(|| vm.host_natives.get(name).map(NativeCallTarget::Host))
}

#[inline]
fn native_call_args_from_registers(
    frame: &CallFrame,
    registers: &[Register],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<SmallStorage<OwnedValue>> {
    SmallStorage::try_from_slice_map(registers, 4, |register| {
        value_to_owned(frame.read(*register)?, heap)
    })
}
