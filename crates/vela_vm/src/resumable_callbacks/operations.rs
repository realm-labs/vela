use crate::heap::HeapValue;
use crate::option_result::{StdEnumVariant, std_enum_tag};
use crate::{
    CallbackMethodInlineCacheTarget, HeapExecution, StandardMethodReceiver, Value, VmError,
    VmErrorKind, VmResult,
};

pub(super) fn callback_operation(
    receiver: StandardMethodReceiver,
    target: CallbackMethodInlineCacheTarget,
) -> Option<&'static str> {
    let supported = match receiver {
        StandardMethodReceiver::Array | StandardMethodReceiver::Set => matches!(
            target,
            CallbackMethodInlineCacheTarget::Map
                | CallbackMethodInlineCacheTarget::Filter
                | CallbackMethodInlineCacheTarget::Retain
                | CallbackMethodInlineCacheTarget::Find
                | CallbackMethodInlineCacheTarget::Any
                | CallbackMethodInlineCacheTarget::All
                | CallbackMethodInlineCacheTarget::Count
                | CallbackMethodInlineCacheTarget::Sum
                | CallbackMethodInlineCacheTarget::GroupBy
                | CallbackMethodInlineCacheTarget::SortBy
        ),
        StandardMethodReceiver::Map => matches!(
            target,
            CallbackMethodInlineCacheTarget::MapValues
                | CallbackMethodInlineCacheTarget::Filter
                | CallbackMethodInlineCacheTarget::GroupBy
                | CallbackMethodInlineCacheTarget::Retain
                | CallbackMethodInlineCacheTarget::Find
                | CallbackMethodInlineCacheTarget::Any
                | CallbackMethodInlineCacheTarget::All
                | CallbackMethodInlineCacheTarget::Count
        ),
        StandardMethodReceiver::Option => matches!(
            target,
            CallbackMethodInlineCacheTarget::Map
                | CallbackMethodInlineCacheTarget::AndThen
                | CallbackMethodInlineCacheTarget::OrElse
                | CallbackMethodInlineCacheTarget::Filter
        ),
        StandardMethodReceiver::Result => matches!(
            target,
            CallbackMethodInlineCacheTarget::Map
                | CallbackMethodInlineCacheTarget::MapErr
                | CallbackMethodInlineCacheTarget::AndThen
                | CallbackMethodInlineCacheTarget::OrElse
        ),
        _ => false,
    };
    supported.then(|| match target {
        CallbackMethodInlineCacheTarget::Map => "method map",
        CallbackMethodInlineCacheTarget::MapValues => "method map_values",
        CallbackMethodInlineCacheTarget::MapErr => "method map_err",
        CallbackMethodInlineCacheTarget::AndThen => "method and_then",
        CallbackMethodInlineCacheTarget::OrElse => "method or_else",
        CallbackMethodInlineCacheTarget::Filter => "method filter",
        CallbackMethodInlineCacheTarget::Retain => "method retain",
        CallbackMethodInlineCacheTarget::Find => "method find",
        CallbackMethodInlineCacheTarget::Any => "method any",
        CallbackMethodInlineCacheTarget::All => "method all",
        CallbackMethodInlineCacheTarget::Count => "method count",
        CallbackMethodInlineCacheTarget::Sum => "method sum",
        CallbackMethodInlineCacheTarget::GroupBy => "method group_by",
        CallbackMethodInlineCacheTarget::SortBy => "method sort_by",
        _ => unreachable!(),
    })
}

pub(super) fn enum_callback_is_active(
    receiver: StandardMethodReceiver,
    target: CallbackMethodInlineCacheTarget,
    variant: StdEnumVariant,
) -> bool {
    matches!(
        (receiver, target, variant),
        (
            StandardMethodReceiver::Option,
            CallbackMethodInlineCacheTarget::Map
                | CallbackMethodInlineCacheTarget::AndThen
                | CallbackMethodInlineCacheTarget::Filter,
            StdEnumVariant::Some,
        ) | (
            StandardMethodReceiver::Option,
            CallbackMethodInlineCacheTarget::OrElse,
            StdEnumVariant::None,
        ) | (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::Map | CallbackMethodInlineCacheTarget::AndThen,
            StdEnumVariant::Ok,
        ) | (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::MapErr | CallbackMethodInlineCacheTarget::OrElse,
            StdEnumVariant::Err,
        )
    )
}

pub(super) fn enum_value(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<(StdEnumVariant, Option<Value>)> {
    let Value::HeapRef(reference) = receiver else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let Some(HeapValue::Enum {
        identity: Some(identity),
        fields,
        ..
    }) = heap.and_then(|heap| heap.heap.get(*reference))
    else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let Some((_, variant)) = std_enum_tag(*identity) else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let payload = if variant.has_payload() {
        Some(
            fields
                .get_slot(0, "0")
                .map(crate::stored_runtime_value)
                .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?,
        )
    } else {
        None
    };
    Ok((variant, payload))
}
