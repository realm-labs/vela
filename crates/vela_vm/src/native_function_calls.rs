use vela_bytecode::{CacheSiteId, DebugNameId, LinkedProgram, NativeHandle, Register};
use vela_common::Span;
use vela_def::FunctionId;

use crate::{
    BorrowedNativeFunction, CallFrame, ExecutionBudget, HeapExecution, HostExecution,
    HostNativeFunction, NativeFunction, NativeInlineCacheEntry, OwnedValue, SmallStorage, Vm,
    VmError, VmErrorKind, VmInlineCaches, VmResult, owned_to_value, value::Value, value_to_owned,
};

pub(crate) struct NativeFunctionCall<'a> {
    pub(crate) dst: Option<Register>,
    pub(crate) name: &'a str,
    pub(crate) native: FunctionId,
    pub(crate) args: &'a [Register],
    pub(crate) call_site: Option<Span>,
}

pub(crate) struct LinkedNativeFunctionCall<'a> {
    pub(crate) dst: Option<Register>,
    pub(crate) program: &'a LinkedProgram,
    pub(crate) native: NativeHandle,
    pub(crate) debug_name: DebugNameId,
    pub(crate) cache_site: Option<CacheSiteId>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) args: &'a [Register],
    pub(crate) call_site: Option<Span>,
}

#[derive(Clone)]
pub(crate) enum NativeCallTarget {
    Pure(NativeFunction),
    BorrowedPure(BorrowedNativeFunction),
    Host(HostNativeFunction),
    BorrowedHost(crate::BorrowedHostNativeFunction),
}

impl NativeCallTarget {
    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::Pure(_) => "pure",
            Self::BorrowedPure(_) => "borrowed_pure",
            Self::Host(_) => "host",
            Self::BorrowedHost(_) => "borrowed_host",
        }
    }
}

pub(crate) fn dispatch_native_function_call(
    vm: &Vm,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: NativeFunctionCall<'_>,
) -> VmResult<()> {
    let Some(target) = resolve_native_call_target_by_id(vm, call.native) else {
        return Err(VmError::new(VmErrorKind::UnknownNative {
            name: call.name.to_owned(),
        })
        .with_source_span_if_absent(call.call_site));
    };
    dispatch_resolved_native_function_call(host, heap, budget, frame, &call, target)
}

pub(crate) fn dispatch_linked_native_function_call(
    vm: &Vm,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: LinkedNativeFunctionCall<'_>,
) -> VmResult<()> {
    let target = call.program.native_function(call.native).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownNative {
            name: call.program.debug_name(call.debug_name).to_owned(),
        })
        .with_source_span_if_absent(call.call_site)
    })?;
    let cache_site = call.cache_site;
    let inline_caches = call.inline_caches;
    let call = NativeFunctionCall {
        dst: call.dst,
        name: call.program.debug_name(target.debug_name),
        native: target.id,
        args: call.args,
        call_site: call.call_site,
    };
    let Some(target) =
        resolve_cached_native_call_target(vm, call.native, cache_site, inline_caches)
    else {
        return Err(VmError::new(VmErrorKind::UnknownNative {
            name: call.name.to_owned(),
        })
        .with_source_span_if_absent(call.call_site));
    };
    dispatch_resolved_native_function_call(host, heap, budget, frame, &call, target)
}

fn dispatch_resolved_native_function_call(
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: &NativeFunctionCall<'_>,
    target: NativeCallTarget,
) -> VmResult<()> {
    let result = match target {
        NativeCallTarget::Pure(native) => {
            let values = native_call_args_from_registers(frame, call.args, heap.as_deref())?;
            native(values.as_slice())
                .map_err(|error| error.with_source_span_if_absent(call.call_site))?
        }
        NativeCallTarget::BorrowedPure(native) => {
            let values = native_borrowed_call_args_from_registers(frame, call.args)?;
            let heap = heap.as_deref().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "native heap",
                })
                .with_source_span_if_absent(call.call_site)
            })?;
            native(values.as_slice(), heap, budget.as_deref_mut())
                .map_err(|error| error.with_source_span_if_absent(call.call_site))?
        }
        NativeCallTarget::Host(native) => {
            let values = native_call_args_from_registers(frame, call.args, heap.as_deref())?;
            let host = host.as_deref_mut().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host context",
                })
            })?;
            native(values.as_slice(), host, budget.as_deref_mut())
                .map_err(|error| error.with_source_span_if_absent(call.call_site))?
        }
        NativeCallTarget::BorrowedHost(native) => {
            let values = native_borrowed_call_args_from_registers(frame, call.args)?;
            let heap = heap.as_deref().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "native heap",
                })
                .with_source_span_if_absent(call.call_site)
            })?;
            let host = host.as_deref_mut().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "host context",
                })
                .with_source_span_if_absent(call.call_site)
            })?;
            native(values.as_slice(), heap, host, budget.as_deref_mut())
                .map_err(|error| error.with_source_span_if_absent(call.call_site))?
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

fn resolve_cached_native_call_target(
    vm: &Vm,
    native: FunctionId,
    cache_site: Option<CacheSiteId>,
    inline_caches: Option<&dyn VmInlineCaches>,
) -> Option<NativeCallTarget> {
    let cache = cache_site.zip(inline_caches);
    if let Some((site, caches)) = cache
        && let Some(entry) = caches.native_call(site)
        && entry.matches(native)
    {
        return Some(entry.target());
    }
    let target = resolve_native_call_target_by_id(vm, native)?;
    if let Some((site, caches)) = cache {
        caches.set_native_call(site, NativeInlineCacheEntry::new(native, target.clone()));
    }
    Some(target)
}

fn resolve_native_call_target_by_id(vm: &Vm, native: FunctionId) -> Option<NativeCallTarget> {
    vm.borrowed_native_ids
        .get(&native)
        .cloned()
        .map(NativeCallTarget::BorrowedPure)
        .or_else(|| {
            vm.borrowed_host_native_ids
                .get(&native)
                .cloned()
                .map(NativeCallTarget::BorrowedHost)
        })
        .or_else(|| {
            vm.native_ids
                .get(&native)
                .cloned()
                .map(NativeCallTarget::Pure)
        })
        .or_else(|| {
            vm.host_native_ids
                .get(&native)
                .cloned()
                .map(NativeCallTarget::Host)
        })
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

#[inline]
fn native_borrowed_call_args_from_registers(
    frame: &CallFrame,
    registers: &[Register],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(registers, 4, |register| Ok(*frame.read(*register)?))
}
