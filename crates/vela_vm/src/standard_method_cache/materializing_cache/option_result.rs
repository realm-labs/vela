use crate::heap::HeapValue;
use crate::option_result::{
    StdEnumKind, StdEnumVariant, option_value, result_value, std_enum_identity, std_enum_tag,
};
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheTarget, StandardMethodReceiver, Value,
    VmError, VmErrorKind, VmResult,
};

pub(in crate::standard_method_cache) fn call_cached_option_result_materialization(
    receiver: &Value,
    cached: StandardMethodReceiver,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let (kind, variant) = cached_standard_enum_tag(receiver, heap.as_deref())?;
    match (cached, kind, target) {
        (
            StandardMethodReceiver::Option,
            StdEnumKind::Option,
            StandardMethodInlineCacheTarget::OkOr,
        ) => Some(call_cached_option_ok_or(
            receiver, variant, args, heap, budget,
        )),
        (
            StandardMethodReceiver::Option,
            StdEnumKind::Option,
            StandardMethodInlineCacheTarget::Flatten,
        ) => Some(call_cached_option_flatten(
            receiver, variant, args, heap, budget,
        )),
        (
            StandardMethodReceiver::Result,
            StdEnumKind::Result,
            StandardMethodInlineCacheTarget::ToOption,
        ) => Some(call_cached_result_to_option(
            receiver, variant, args, heap, budget,
        )),
        (
            StandardMethodReceiver::Result,
            StdEnumKind::Result,
            StandardMethodInlineCacheTarget::ToErrorOption,
        ) => Some(call_cached_result_to_error_option(
            receiver, variant, args, heap, budget,
        )),
        (
            StandardMethodReceiver::Result,
            StdEnumKind::Result,
            StandardMethodInlineCacheTarget::Flatten,
        ) => Some(call_cached_result_flatten(
            receiver, variant, args, heap, budget,
        )),
        _ => None,
    }
}

fn call_cached_option_ok_or(
    receiver: &Value,
    variant: StdEnumVariant,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("ok_or", args, 1)?;
    match variant {
        StdEnumVariant::Some => {
            let payload = cached_standard_enum_payload(
                receiver,
                heap.as_deref(),
                StdEnumVariant::Some,
                "method ok_or",
            )?;
            make_result(StdEnumVariant::Ok, payload, heap, budget, "method ok_or")
        }
        StdEnumVariant::None => {
            make_result(StdEnumVariant::Err, args[0], heap, budget, "method ok_or")
        }
        _ => type_error("method ok_or"),
    }
}

fn call_cached_option_flatten(
    receiver: &Value,
    variant: StdEnumVariant,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("flatten", args, 0)?;
    match variant {
        StdEnumVariant::Some => {
            let payload = cached_standard_enum_payload(
                receiver,
                heap.as_deref(),
                StdEnumVariant::Some,
                "method flatten",
            )?;
            expect_standard_enum_kind(payload, heap.as_deref(), StdEnumKind::Option)
        }
        StdEnumVariant::None => make_option(None, heap, budget, "method flatten"),
        _ => type_error("method flatten"),
    }
}

fn call_cached_result_to_option(
    receiver: &Value,
    variant: StdEnumVariant,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("to_option", args, 0)?;
    match variant {
        StdEnumVariant::Ok => {
            let payload = cached_standard_enum_payload(
                receiver,
                heap.as_deref(),
                StdEnumVariant::Ok,
                "method to_option",
            )?;
            make_option(Some(payload), heap, budget, "method to_option")
        }
        StdEnumVariant::Err => make_option(None, heap, budget, "method to_option"),
        _ => type_error("method to_option"),
    }
}

fn call_cached_result_to_error_option(
    receiver: &Value,
    variant: StdEnumVariant,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("to_error_option", args, 0)?;
    match variant {
        StdEnumVariant::Ok => make_option(None, heap, budget, "method to_error_option"),
        StdEnumVariant::Err => {
            let payload = cached_standard_enum_payload(
                receiver,
                heap.as_deref(),
                StdEnumVariant::Err,
                "method to_error_option",
            )?;
            make_option(Some(payload), heap, budget, "method to_error_option")
        }
        _ => type_error("method to_error_option"),
    }
}

fn call_cached_result_flatten(
    receiver: &Value,
    variant: StdEnumVariant,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    crate::runtime_checks::expect_arity("flatten", args, 0)?;
    match variant {
        StdEnumVariant::Ok => {
            let payload = cached_standard_enum_payload(
                receiver,
                heap.as_deref(),
                StdEnumVariant::Ok,
                "method flatten",
            )?;
            expect_standard_enum_kind(payload, heap.as_deref(), StdEnumKind::Result)
        }
        StdEnumVariant::Err => {
            let payload = cached_standard_enum_payload(
                receiver,
                heap.as_deref(),
                StdEnumVariant::Err,
                "method flatten",
            )?;
            make_result(StdEnumVariant::Err, payload, heap, budget, "method flatten")
        }
        _ => type_error("method flatten"),
    }
}

fn cached_standard_enum_tag(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<(StdEnumKind, StdEnumVariant)> {
    let HeapValue::Enum {
        identity: Some(identity),
        ..
    } = cached_heap_value(receiver, heap)?
    else {
        return None;
    };
    std_enum_tag(*identity)
}

fn cached_standard_enum_payload(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    variant: StdEnumVariant,
    operation: &'static str,
) -> VmResult<Value> {
    let HeapValue::Enum {
        identity: Some(identity),
        fields,
        ..
    } = cached_heap_value(receiver, heap)
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?
    else {
        return type_error(operation);
    };
    if !variant.has_payload()
        || identity.payload_field_id != std_enum_identity(variant).payload_field_id
    {
        return type_error(operation);
    }
    fields
        .get_slot(0, "0")
        .copied()
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn expect_standard_enum_kind(
    value: Value,
    heap: Option<&HeapExecution<'_>>,
    expected: StdEnumKind,
) -> VmResult<Value> {
    match cached_standard_enum_tag(&value, heap) {
        Some((kind, _)) if kind == expected => Ok(value),
        _ => type_error("method flatten"),
    }
}

fn cached_heap_value<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<&'a HeapValue> {
    let Value::HeapRef(reference) = receiver else {
        return None;
    };
    heap.and_then(|heap| heap.heap.get(*reference))
}

fn make_option(
    payload: Option<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    option_value(payload, heap, budget.as_deref_mut())
}

fn make_result(
    variant: StdEnumVariant,
    payload: Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    result_value(variant, payload, heap, budget.as_deref_mut())
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
