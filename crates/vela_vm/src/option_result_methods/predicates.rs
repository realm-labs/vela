use crate::{HeapExecution, Value};

use super::access::enum_tag;

pub(crate) fn is_option_or_result(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    enum_tag(receiver, heap).is_some_and(|tag| tag.is_option() || tag.is_result())
}

pub(crate) fn is_option(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    enum_tag(receiver, heap).is_some_and(|tag| tag.is_option())
}

pub(crate) fn is_result(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    enum_tag(receiver, heap).is_some_and(|tag| tag.is_result())
}
