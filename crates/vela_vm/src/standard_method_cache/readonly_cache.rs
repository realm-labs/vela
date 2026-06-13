use crate::heap::HeapValue;
use crate::option_result::{StdEnumKind, StdEnumVariant, std_enum_identity, std_enum_tag};
use crate::{
    HeapExecution, StandardMethodInlineCacheTarget, StandardMethodReceiver, Value, VmError,
    VmErrorKind, VmResult, script_builtin_methods, set_methods,
};
use vela_common::ScalarValue;

pub(super) fn call_cached_len(
    receiver: &Value,
    cached: StandardMethodReceiver,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let len = match cached {
        StandardMethodReceiver::String => {
            let HeapValue::String(value) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            string_char_len(value)
        }
        StandardMethodReceiver::Bytes => {
            let HeapValue::Bytes(value) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            value.len()
        }
        StandardMethodReceiver::Range => {
            let Value::Range(range) = receiver else {
                return None;
            };
            if let Err(error) = script_builtin_methods::expect_no_args("len", args) {
                return Some(Err(error));
            }
            return Some(range.len().map(Value::i64).ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "method len",
                })
            }));
        }
        StandardMethodReceiver::Array => {
            let HeapValue::Array(values) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            values.len()
        }
        StandardMethodReceiver::Map => {
            let HeapValue::Map(values) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            values.len()
        }
        StandardMethodReceiver::Set => {
            let HeapValue::Set(values) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            values.len()
        }
        StandardMethodReceiver::Option | StandardMethodReceiver::Result => {
            return Some(type_error("method len"));
        }
    };
    Some(
        script_builtin_methods::expect_no_args("len", args)
            .and_then(|()| usize_to_i64(len, "method len").map(Value::i64)),
    )
}

pub(super) fn call_cached_is_empty(
    receiver: &Value,
    cached: StandardMethodReceiver,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let is_empty = match cached {
        StandardMethodReceiver::String => {
            let HeapValue::String(value) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            value.is_empty()
        }
        StandardMethodReceiver::Bytes => {
            let HeapValue::Bytes(value) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            value.is_empty()
        }
        StandardMethodReceiver::Range => {
            let Value::Range(range) = receiver else {
                return None;
            };
            if let Err(error) = script_builtin_methods::expect_no_args("is_empty", args) {
                return Some(Err(error));
            }
            return Some(Ok(Value::Bool(range.is_empty())));
        }
        StandardMethodReceiver::Array => {
            let HeapValue::Array(values) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            values.is_empty()
        }
        StandardMethodReceiver::Map => {
            let HeapValue::Map(values) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            values.is_empty()
        }
        StandardMethodReceiver::Set => {
            let HeapValue::Set(values) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            values.is_empty()
        }
        StandardMethodReceiver::Option | StandardMethodReceiver::Result => {
            return Some(type_error("method is_empty"));
        }
    };
    Some(script_builtin_methods::expect_no_args("is_empty", args).map(|()| Value::Bool(is_empty)))
}

pub(super) fn call_cached_option_result_predicate(
    receiver: &Value,
    cached: StandardMethodReceiver,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let (kind, variant) = cached_standard_enum_tag(receiver, heap)?;
    let (expected_receiver, expected_kind, method, result) = match target {
        StandardMethodInlineCacheTarget::IsSome => (
            StandardMethodReceiver::Option,
            StdEnumKind::Option,
            "is_some",
            variant == StdEnumVariant::Some,
        ),
        StandardMethodInlineCacheTarget::IsNone => (
            StandardMethodReceiver::Option,
            StdEnumKind::Option,
            "is_none",
            variant == StdEnumVariant::None,
        ),
        StandardMethodInlineCacheTarget::IsOk => (
            StandardMethodReceiver::Result,
            StdEnumKind::Result,
            "is_ok",
            variant == StdEnumVariant::Ok,
        ),
        StandardMethodInlineCacheTarget::IsErr => (
            StandardMethodReceiver::Result,
            StdEnumKind::Result,
            "is_err",
            variant == StdEnumVariant::Err,
        ),
        _ => return None,
    };
    if cached != expected_receiver || kind != expected_kind {
        return None;
    }
    Some(script_builtin_methods::expect_no_args(method, args).map(|()| Value::Bool(result)))
}

pub(super) fn call_cached_option_result_unwrap_or(
    receiver: &Value,
    cached: StandardMethodReceiver,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let (kind, variant) = cached_standard_enum_tag(receiver, heap)?;
    match (cached, kind, variant) {
        (StandardMethodReceiver::Option, StdEnumKind::Option, StdEnumVariant::Some)
        | (StandardMethodReceiver::Result, StdEnumKind::Result, StdEnumVariant::Ok) => Some(
            crate::runtime_checks::expect_arity("unwrap_or", args, 1).and_then(|()| {
                cached_standard_enum_payload(receiver, heap, variant, "method unwrap_or")
            }),
        ),
        (StandardMethodReceiver::Option, StdEnumKind::Option, StdEnumVariant::None)
        | (StandardMethodReceiver::Result, StdEnumKind::Result, StdEnumVariant::Err) => {
            Some(crate::runtime_checks::expect_arity("unwrap_or", args, 1).map(|()| args[0]))
        }
        _ => None,
    }
}

pub(super) fn call_cached_map_get_or(
    receiver: &Value,
    cached: StandardMethodReceiver,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    if cached != StandardMethodReceiver::Map {
        return None;
    }
    let HeapValue::Map(values) = cached_heap_value(receiver, heap)? else {
        return None;
    };
    Some(
        crate::runtime_checks::expect_arity("get_or", args, 2).and_then(|()| {
            let key = crate::string_methods::string_value(&args[0], heap, "map key")?;
            Ok(values.get(key).map_or(args[1], |value| *value))
        }),
    )
}

pub(super) fn call_cached_collection_has(
    receiver: &Value,
    cached: StandardMethodReceiver,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    match cached {
        StandardMethodReceiver::Map => {
            let HeapValue::Map(values) = cached_heap_value(receiver, heap)? else {
                return None;
            };
            Some(
                crate::runtime_checks::expect_arity("has", args, 1).and_then(|()| {
                    let key = crate::string_methods::string_value(&args[0], heap, "map key")?;
                    Ok(Value::Bool(values.contains_key(key)))
                }),
            )
        }
        StandardMethodReceiver::Set => {
            let heap = heap?;
            let HeapValue::Set(values) = cached_heap_value(receiver, Some(heap))? else {
                return None;
            };
            Some(
                crate::runtime_checks::expect_arity("has", args, 1).and_then(|()| {
                    if let Some(result) = cached_set_contains_immediate(values, &args[0]) {
                        return Ok(Value::Bool(result));
                    }
                    set_methods::contains_value(values, &args[0], heap, "method has")
                        .map(Value::Bool)
                }),
            )
        }
        _ => None,
    }
}

pub(super) fn call_cached_set_relation(
    receiver: &Value,
    cached: StandardMethodReceiver,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    if cached != StandardMethodReceiver::Set {
        return None;
    }
    let heap = heap?;
    let HeapValue::Set(values) = cached_heap_value(receiver, Some(heap))? else {
        return None;
    };
    let (name, operation, relation) = match target {
        StandardMethodInlineCacheTarget::IsSubset => (
            "is_subset",
            "method is_subset",
            set_methods::SetRelation::Subset,
        ),
        StandardMethodInlineCacheTarget::IsSuperset => (
            "is_superset",
            "method is_superset",
            set_methods::SetRelation::Superset,
        ),
        StandardMethodInlineCacheTarget::IsDisjoint => (
            "is_disjoint",
            "method is_disjoint",
            set_methods::SetRelation::Disjoint,
        ),
        _ => return None,
    };
    Some(
        crate::runtime_checks::expect_arity(name, args, 1).and_then(|()| {
            set_methods::relation_matches(values, &args[0], heap, relation, operation)
                .map(Value::Bool)
        }),
    )
}

pub(super) fn call_cached_string_predicate(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let HeapValue::String(value) = cached_heap_value(receiver, heap)? else {
        return None;
    };
    let (name, operation) = match target {
        StandardMethodInlineCacheTarget::Contains => ("contains", "method contains"),
        StandardMethodInlineCacheTarget::StartsWith => ("starts_with", "method starts_with"),
        StandardMethodInlineCacheTarget::EndsWith => ("ends_with", "method ends_with"),
        _ => return None,
    };
    Some(
        crate::runtime_checks::expect_arity(name, args, 1).and_then(|()| {
            let needle = crate::string_methods::string_value(&args[0], heap, operation)?;
            let result = match target {
                StandardMethodInlineCacheTarget::Contains => value.contains(needle),
                StandardMethodInlineCacheTarget::StartsWith => value.starts_with(needle),
                StandardMethodInlineCacheTarget::EndsWith => value.ends_with(needle),
                _ => unreachable!("string predicate target was validated above"),
            };
            Ok(Value::Bool(result))
        }),
    )
}

pub(super) fn call_cached_array_contains(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let HeapValue::Array(values) = cached_heap_value(receiver, heap)? else {
        return None;
    };
    Some(
        crate::runtime_checks::expect_arity("contains", args, 1).and_then(|()| {
            for value in values {
                if let Some(equal) = crate::heap_values::simple_values_equal(value, &args[0], heap)
                {
                    if equal {
                        return Ok(Value::Bool(true));
                    }
                    continue;
                }
                if crate::values_equal(value, &args[0], heap)? {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }),
    )
}

pub(super) fn call_cached_bytes_accessor(
    receiver: &Value,
    target: StandardMethodInlineCacheTarget,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let HeapValue::Bytes(bytes) = cached_heap_value(receiver, heap)? else {
        return None;
    };
    let (method, operation) = match target {
        StandardMethodInlineCacheTarget::Get => ("get", "method get"),
        StandardMethodInlineCacheTarget::ReadU32Le => ("read_u32_le", "method read_u32_le"),
        StandardMethodInlineCacheTarget::ReadU32Be => ("read_u32_be", "method read_u32_be"),
        _ => return None,
    };
    Some(
        crate::runtime_checks::expect_arity(method, args, 1).and_then(|()| {
            let index = byte_index(&args[0], bytes.len(), operation)?;
            match target {
                StandardMethodInlineCacheTarget::Get => {
                    let byte = bytes
                        .get(index)
                        .copied()
                        .ok_or_else(|| index_out_of_bounds(index, bytes.len()))?;
                    Ok(Value::Scalar(ScalarValue::U8(byte)))
                }
                StandardMethodInlineCacheTarget::ReadU32Le
                | StandardMethodInlineCacheTarget::ReadU32Be => {
                    let end = index
                        .checked_add(4)
                        .ok_or_else(|| index_out_of_bounds(index, bytes.len()))?;
                    if end > bytes.len() {
                        return Err(index_out_of_bounds(index, bytes.len()));
                    }
                    let word = <[u8; 4]>::try_from(&bytes[index..end])
                        .map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
                    let value = match target {
                        StandardMethodInlineCacheTarget::ReadU32Le => u32::from_le_bytes(word),
                        StandardMethodInlineCacheTarget::ReadU32Be => u32::from_be_bytes(word),
                        _ => unreachable!("bytes read target was validated above"),
                    };
                    Ok(Value::Scalar(ScalarValue::U32(value)))
                }
                _ => unreachable!("bytes accessor target was validated above"),
            }
        }),
    )
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

fn cached_heap_value<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> Option<&'a HeapValue> {
    let Value::HeapRef(reference) = receiver else {
        return None;
    };
    heap.and_then(|heap| heap.heap.get(*reference))
}

fn string_char_len(value: &str) -> usize {
    if value.is_ascii() {
        value.len()
    } else {
        value.chars().count()
    }
}

fn usize_to_i64(value: usize, operation: &'static str) -> VmResult<i64> {
    i64::try_from(value).map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn byte_index(value: &Value, len: usize, operation: &'static str) -> VmResult<usize> {
    match value {
        Value::Scalar(ScalarValue::I64(index)) if *index >= 0 => Ok(*index as usize),
        Value::Scalar(ScalarValue::I64(index)) => {
            Err(VmError::new(VmErrorKind::IndexOutOfBounds {
                index: *index,
                len,
            }))
        }
        _ => type_error(operation),
    }
}

fn index_out_of_bounds(index: usize, len: usize) -> VmError {
    VmError::new(VmErrorKind::IndexOutOfBounds {
        index: i64::try_from(index).unwrap_or(i64::MAX),
        len,
    })
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn cached_set_contains_immediate(values: &[Value], candidate: &Value) -> Option<bool> {
    for value in values {
        match (candidate, value) {
            (Value::Null, Value::Null) => return Some(true),
            (Value::Bool(lhs), Value::Bool(rhs)) if lhs == rhs => return Some(true),
            (Value::Scalar(ScalarValue::I64(lhs)), Value::Scalar(ScalarValue::I64(rhs)))
                if lhs == rhs =>
            {
                return Some(true);
            }
            (Value::Scalar(ScalarValue::F64(lhs)), Value::Scalar(ScalarValue::F64(rhs)))
                if lhs.is_finite() && rhs.is_finite() && lhs.to_bits() == rhs.to_bits() =>
            {
                return Some(true);
            }
            (
                Value::Null | Value::Bool(_) | Value::Scalar(ScalarValue::I64(_)),
                Value::Null | Value::Bool(_) | Value::Scalar(ScalarValue::I64(_)),
            ) => {}
            (
                Value::Null | Value::Bool(_) | Value::Scalar(ScalarValue::I64(_)),
                Value::Scalar(ScalarValue::F64(value)),
            ) if value.is_finite() => {}
            (
                Value::Scalar(ScalarValue::F64(candidate)),
                Value::Null | Value::Bool(_) | Value::Scalar(ScalarValue::I64(_)),
            ) if candidate.is_finite() => {}
            (
                Value::Scalar(ScalarValue::F64(candidate)),
                Value::Scalar(ScalarValue::F64(value)),
            ) if candidate.is_finite() && value.is_finite() => {}
            _ => return None,
        }
    }
    Some(false)
}
