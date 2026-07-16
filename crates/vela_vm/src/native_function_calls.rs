use std::sync::Arc;

use vela_bytecode::{CacheSiteId, DebugNameId, LinkedProgram, NativeHandle, Register};
use vela_common::Span;
use vela_def::FunctionId;

use crate::{
    AsyncHostNativeFunction, AsyncNativeFunction, BorrowedNativeFunction, CallFrame,
    ConditionalAsyncNativeFunction, ConditionalHostNativeFunction, ConditionalHostNativeOutcome,
    ExecutionBudget, HeapExecution, HostExecution, HostNativeFunction, NativeFunction,
    NativeInlineCacheEntry, OwnedValue, SmallStorage, Vm, VmError, VmErrorKind, VmInlineCaches,
    VmResult, owned_to_value, value::Value, value_to_owned,
};

struct NativeFunctionCall<'a> {
    dst: Option<Register>,
    name: &'a str,
    native: FunctionId,
    args: &'a [Register],
    call_site: Option<Span>,
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

pub(crate) enum LinkedNativeDispatch {
    Complete,
    Async(PreparedAsyncNativeCall),
}

pub(crate) struct PreparedAsyncNativeCall {
    pub(crate) native_id: FunctionId,
    pub(crate) function: PreparedAsyncNativeFunction,
    pub(crate) args: Vec<OwnedValue>,
    pub(crate) destination: Option<Register>,
    pub(crate) name: String,
    pub(crate) source_span: Option<Span>,
}

pub(crate) enum PreparedAsyncNativeFunction {
    Pure(AsyncNativeFunction),
    Host(AsyncHostNativeFunction),
    HostMethod {
        function: crate::AsyncHostMethodFunction,
        receiver: vela_host::path::HostPath,
    },
    DirectHostMethod {
        function: crate::AsyncDirectHostMethodFunction,
        receiver: vela_host::path::HostPath,
        lease_kind: vela_host::lease::HostLeaseKind,
    },
    DirectHostFunction {
        function: crate::AsyncDirectHostFunction,
        requests: Vec<(vela_host::path::HostRef, vela_host::lease::HostLeaseKind)>,
    },
}

#[derive(Clone)]
pub(crate) enum NativeCallTarget {
    Pure(NativeFunction),
    AsyncPure(AsyncNativeFunction),
    AsyncHost(AsyncHostNativeFunction),
    ConditionalHost(ConditionalHostNativeFunction),
    BorrowedPure(BorrowedNativeFunction),
    Host(HostNativeFunction),
    BorrowedHost(crate::BorrowedHostNativeFunction),
}

impl NativeCallTarget {
    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::Pure(_) => "pure",
            Self::AsyncPure(_) => "async_pure",
            Self::AsyncHost(_) => "async_host",
            Self::ConditionalHost(_) => "conditional_host",
            Self::BorrowedPure(_) => "borrowed_pure",
            Self::Host(_) => "host",
            Self::BorrowedHost(_) => "borrowed_host",
        }
    }
}

pub(crate) fn dispatch_linked_native_function_call(
    vm: &Vm,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: LinkedNativeFunctionCall<'_>,
) -> VmResult<LinkedNativeDispatch> {
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
    let async_function = match &target {
        NativeCallTarget::AsyncPure(function) => {
            Some(PreparedAsyncNativeFunction::Pure(Arc::clone(function)))
        }
        NativeCallTarget::AsyncHost(function) => {
            Some(PreparedAsyncNativeFunction::Host(Arc::clone(function)))
        }
        _ => None,
    };
    if let Some(function) = async_function {
        let args = native_call_args_from_registers(frame, call.args, heap.as_deref())?
            .as_slice()
            .to_vec();
        return Ok(LinkedNativeDispatch::Async(PreparedAsyncNativeCall {
            native_id: call.native,
            function,
            args,
            destination: call.dst,
            name: call.name.to_owned(),
            source_span: call.call_site,
        }));
    }
    if let NativeCallTarget::ConditionalHost(function) = target {
        let args = native_call_args_from_registers(frame, call.args, heap.as_deref())?
            .as_slice()
            .to_vec();
        let host = host.as_deref_mut().ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "host context",
            })
        })?;
        return match function(&args, host, budget.as_deref_mut())
            .map_err(|error| error.with_source_span_if_absent(call.call_site))?
        {
            ConditionalHostNativeOutcome::Complete(result) => {
                write_native_result(frame, heap, budget, call.dst, result)?;
                Ok(LinkedNativeDispatch::Complete)
            }
            ConditionalHostNativeOutcome::Async {
                function,
                args,
                diagnostic_name,
            } => Ok(LinkedNativeDispatch::Async(PreparedAsyncNativeCall {
                native_id: call.native,
                function: match function {
                    ConditionalAsyncNativeFunction::Pure(function) => {
                        PreparedAsyncNativeFunction::Pure(function)
                    }
                    ConditionalAsyncNativeFunction::Host(function) => {
                        PreparedAsyncNativeFunction::Host(function)
                    }
                    ConditionalAsyncNativeFunction::HostMethod { function, receiver } => {
                        PreparedAsyncNativeFunction::HostMethod { function, receiver }
                    }
                    ConditionalAsyncNativeFunction::DirectHostMethod {
                        function,
                        receiver,
                        lease_kind,
                    } => PreparedAsyncNativeFunction::DirectHostMethod {
                        function,
                        receiver,
                        lease_kind,
                    },
                    ConditionalAsyncNativeFunction::DirectHostFunction { function, requests } => {
                        PreparedAsyncNativeFunction::DirectHostFunction { function, requests }
                    }
                },
                args,
                destination: call.dst,
                name: diagnostic_name,
                source_span: call.call_site,
            })),
        };
    }
    dispatch_resolved_native_function_call(host, heap, budget, frame, &call, target)?;
    Ok(LinkedNativeDispatch::Complete)
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
        NativeCallTarget::AsyncPure(_)
        | NativeCallTarget::AsyncHost(_)
        | NativeCallTarget::ConditionalHost(_) => {
            unreachable!("async targets are prepared before dispatch")
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
    write_native_result(frame, heap, budget, call.dst, result)
}

pub(crate) fn write_native_result(
    frame: &mut CallFrame,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    destination: Option<Register>,
    result: OwnedValue,
) -> VmResult<()> {
    let Some(destination) = destination else {
        return Ok(());
    };
    let result = owned_to_value(
        result,
        heap.as_deref_mut().ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "native heap",
            })
        })?,
        budget.as_deref_mut(),
    )?;
    frame.write(destination, result)
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
            vm.conditional_host_native_ids
                .get(&native)
                .cloned()
                .map(NativeCallTarget::ConditionalHost)
        })
        .or_else(|| {
            vm.borrowed_host_native_ids
                .get(&native)
                .cloned()
                .map(NativeCallTarget::BorrowedHost)
        })
        .or_else(|| {
            vm.async_native_ids
                .get(&native)
                .cloned()
                .map(NativeCallTarget::AsyncPure)
        })
        .or_else(|| {
            vm.async_host_native_ids
                .get(&native)
                .cloned()
                .map(NativeCallTarget::AsyncHost)
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
        value_to_owned(&frame.read(*register)?, heap)
    })
}

#[inline]
fn native_borrowed_call_args_from_registers(
    frame: &CallFrame,
    registers: &[Register],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(registers, 4, |register| frame.read(*register))
}
