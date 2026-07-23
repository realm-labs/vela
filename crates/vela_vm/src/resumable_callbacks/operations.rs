use crate::option_result::{StdEnumVariant, StdEnumVariant::Err, StdEnumVariant::None};
use crate::{CallbackMethodInlineCacheTarget, StandardMethodReceiver};

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
            None,
        ) | (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::Map | CallbackMethodInlineCacheTarget::AndThen,
            StdEnumVariant::Ok,
        ) | (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::MapErr | CallbackMethodInlineCacheTarget::OrElse,
            Err,
        )
    )
}
