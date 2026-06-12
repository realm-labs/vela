use crate::callback_method_dispatch::CallbackMethodDispatch;
use crate::heap::HeapValue;
use crate::option_result::{StdEnumVariant, option_value, result_value, std_enum_tag};
use crate::{
    CallbackMethodInlineCacheTarget, HeapExecution, StandardMethodReceiver, Value, VmError,
    VmErrorKind, VmResult, stored_runtime_value,
};

pub(super) fn call_cached(
    receiver: &Value,
    receiver_kind: StandardMethodReceiver,
    target: CallbackMethodInlineCacheTarget,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> Option<VmResult<Value>> {
    if let Err(error) = crate::runtime_checks::expect_arity(method_name(target)?, args, 1) {
        return Some(Err(error));
    }
    let enum_value = match cached_enum_value(receiver, dispatch.heap_ref(), operation_name(target)?)
    {
        Ok(enum_value) => enum_value,
        Err(error) => return Some(Err(error)),
    };
    let payload = match (receiver_kind, target, enum_value.variant) {
        (
            StandardMethodReceiver::Option,
            CallbackMethodInlineCacheTarget::Map
            | CallbackMethodInlineCacheTarget::AndThen
            | CallbackMethodInlineCacheTarget::Filter,
            StdEnumVariant::None,
        ) => return Some(cached_option_result(None, dispatch, "method option")),
        (
            StandardMethodReceiver::Option,
            CallbackMethodInlineCacheTarget::OrElse,
            StdEnumVariant::Some,
        ) => enum_value.payload,
        (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::Map | CallbackMethodInlineCacheTarget::AndThen,
            StdEnumVariant::Err,
        ) => enum_value.payload,
        (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::MapErr | CallbackMethodInlineCacheTarget::OrElse,
            StdEnumVariant::Ok,
        ) => enum_value.payload,
        _ => return None,
    };
    let Some(payload) = payload else {
        return Some(type_error(operation_name(target)?));
    };
    let result_variant = match (receiver_kind, target, enum_value.variant) {
        (
            StandardMethodReceiver::Option,
            CallbackMethodInlineCacheTarget::OrElse,
            StdEnumVariant::Some,
        ) => {
            return Some(cached_option_result(
                Some(payload),
                dispatch,
                "method option",
            ));
        }
        (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::Map | CallbackMethodInlineCacheTarget::AndThen,
            StdEnumVariant::Err,
        ) => StdEnumVariant::Err,
        (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::MapErr | CallbackMethodInlineCacheTarget::OrElse,
            StdEnumVariant::Ok,
        ) => StdEnumVariant::Ok,
        _ => return None,
    };
    Some(cached_result_result(
        result_variant,
        payload,
        dispatch,
        "method result",
    ))
}

struct CachedEnumValue {
    variant: StdEnumVariant,
    payload: Option<Value>,
}

fn cached_enum_value(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<CachedEnumValue> {
    let Value::HeapRef(reference) = receiver else {
        return type_error(operation);
    };
    let Some(HeapValue::Enum {
        identity: Some(identity),
        fields,
        ..
    }) = heap.and_then(|heap| heap.heap.get(*reference))
    else {
        return type_error(operation);
    };
    let Some((_, variant)) = std_enum_tag(*identity) else {
        return type_error(operation);
    };
    let payload = if variant.has_payload() {
        Some(
            fields
                .get_slot(0, "0")
                .map(stored_runtime_value)
                .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?,
        )
    } else {
        None
    };
    Ok(CachedEnumValue { variant, payload })
}

fn cached_option_result(
    payload: Option<Value>,
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = dispatch.heap.as_deref_mut() else {
        return type_error(operation);
    };
    option_value(payload, heap, dispatch.budget.as_deref_mut())
}

fn cached_result_result(
    variant: StdEnumVariant,
    payload: Value,
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = dispatch.heap.as_deref_mut() else {
        return type_error(operation);
    };
    result_value(variant, payload, heap, dispatch.budget.as_deref_mut())
}

fn method_name(target: CallbackMethodInlineCacheTarget) -> Option<&'static str> {
    match target {
        CallbackMethodInlineCacheTarget::Map => Some("map"),
        CallbackMethodInlineCacheTarget::MapErr => Some("map_err"),
        CallbackMethodInlineCacheTarget::AndThen => Some("and_then"),
        CallbackMethodInlineCacheTarget::OrElse => Some("or_else"),
        CallbackMethodInlineCacheTarget::Filter => Some("filter"),
        _ => None,
    }
}

fn operation_name(target: CallbackMethodInlineCacheTarget) -> Option<&'static str> {
    match target {
        CallbackMethodInlineCacheTarget::Map => Some("method map"),
        CallbackMethodInlineCacheTarget::MapErr => Some("method map_err"),
        CallbackMethodInlineCacheTarget::AndThen => Some("method and_then"),
        CallbackMethodInlineCacheTarget::OrElse => Some("method or_else"),
        CallbackMethodInlineCacheTarget::Filter => Some("method filter"),
        _ => None,
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
