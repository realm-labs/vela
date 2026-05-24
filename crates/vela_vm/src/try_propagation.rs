use crate::{
    HeapExecution, HeapValue, Value, VmError, VmErrorKind, VmResult, get_enum_field_value,
};

pub(crate) enum TryPropagation {
    Continue(Value),
    Return(Value),
}

pub(crate) fn try_propagate_value(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<TryPropagation> {
    let Some((enum_name, variant)) = enum_tag(value, heap) else {
        return type_error();
    };

    if is_builtin_enum(enum_name, "Option") {
        match variant {
            "Some" => get_enum_field_value(value, tuple_variant_field_name(0).as_str(), heap)
                .map(TryPropagation::Continue),
            "None" => Ok(TryPropagation::Return(value.clone())),
            _ => type_error(),
        }
    } else if is_builtin_enum(enum_name, "Result") {
        match variant {
            "Ok" => get_enum_field_value(value, tuple_variant_field_name(0).as_str(), heap)
                .map(TryPropagation::Continue),
            "Err" => Ok(TryPropagation::Return(value.clone())),
            _ => type_error(),
        }
    } else {
        type_error()
    }
}

fn enum_tag<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<(&'a str, &'a str)> {
    match value {
        Value::Enum {
            enum_name, variant, ..
        } => Some((enum_name.as_str(), variant.as_str())),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Enum {
                enum_name, variant, ..
            }) => Some((enum_name.as_str(), variant.as_str())),
            _ => None,
        },
        _ => None,
    }
}

fn is_builtin_enum(enum_name: &str, expected: &str) -> bool {
    enum_name == expected || enum_name.rsplit('.').next() == Some(expected)
}

fn tuple_variant_field_name(index: usize) -> String {
    index.to_string()
}

fn type_error<T>() -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch {
        operation: "try propagation",
    }))
}
