use vela_common::{HostMethodId, HostTypeId};
use vela_def::MethodId;

use vela_bytecode::{
    LinkedMethodDispatchKind, LinkedProgram, MethodDispatchHandle, ScriptFunctionHandle,
};
use vela_reflect::registry::TypeRegistry;

use crate::heap::HeapValue;
use crate::std_method_ids::{StdMethodIds, std_method_ids};
use crate::{
    HeapExecution, HostExecution, Value, array_methods, bytes_methods, iteration, map_methods,
    option_result_methods, set_methods,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DynamicReceiverKind {
    String,
    Bytes,
    Array,
    Map,
    Set,
    Iterator,
    Option,
    Result,
    Range,
    ScriptRecord {
        type_name: String,
    },
    ScriptEnum {
        type_name: String,
    },
    Host {
        type_name: String,
        host_type_id: HostTypeId,
    },
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DynamicMethodTarget {
    Script {
        dispatch: MethodDispatchHandle,
        function: ScriptFunctionHandle,
    },
    Host {
        method_id: HostMethodId,
    },
    StandardValue {
        method_id: MethodId,
    },
}

pub(crate) fn resolve_linked_dynamic_method(
    receiver: &Value,
    method: &str,
    program: &LinkedProgram,
    heap: Option<&HeapExecution<'_>>,
    registry: Option<&TypeRegistry>,
) -> Option<DynamicMethodTarget> {
    resolve_script_dynamic_method(receiver, method, program, heap)
        .or_else(|| resolve_host_dynamic_method(receiver, method, registry))
        .or_else(|| resolve_standard_dynamic_method(receiver, method, heap))
}

pub(crate) fn classify_dynamic_receiver(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    host: Option<&HostExecution<'_>>,
) -> DynamicReceiverKind {
    if crate::string_methods::is_string(receiver, heap) {
        DynamicReceiverKind::String
    } else if bytes_methods::is_bytes(receiver, heap) {
        DynamicReceiverKind::Bytes
    } else if matches!(receiver, Value::Range(_)) {
        DynamicReceiverKind::Range
    } else if array_methods::is_array(receiver, heap) {
        DynamicReceiverKind::Array
    } else if map_methods::is_map(receiver, heap) {
        DynamicReceiverKind::Map
    } else if set_methods::is_set(receiver, heap) {
        DynamicReceiverKind::Set
    } else if iteration::is_iterator(receiver, heap) {
        DynamicReceiverKind::Iterator
    } else if option_result_methods::is_option(receiver, heap) {
        DynamicReceiverKind::Option
    } else if option_result_methods::is_result(receiver, heap) {
        DynamicReceiverKind::Result
    } else if let Value::HeapRef(reference) = receiver {
        match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Record { type_name, .. }) => DynamicReceiverKind::ScriptRecord {
                type_name: type_name.clone(),
            },
            Some(HeapValue::Enum { enum_name, .. }) => DynamicReceiverKind::ScriptEnum {
                type_name: enum_name.clone(),
            },
            _ => DynamicReceiverKind::Unsupported,
        }
    } else if let Value::HostRef(reference) = receiver {
        if host.is_some() {
            DynamicReceiverKind::Host {
                type_name: format!("{:?}", reference.type_id),
                host_type_id: reference.type_id,
            }
        } else {
            DynamicReceiverKind::Unsupported
        }
    } else {
        DynamicReceiverKind::Unsupported
    }
}

pub(crate) fn resolve_standard_dynamic_method(
    receiver: &Value,
    method: &str,
    heap: Option<&HeapExecution<'_>>,
) -> Option<DynamicMethodTarget> {
    let method_id =
        standard_method_id_for_receiver(classify_dynamic_receiver(receiver, heap, None), method)?;
    Some(DynamicMethodTarget::StandardValue { method_id })
}

fn resolve_script_dynamic_method(
    receiver: &Value,
    method: &str,
    program: &LinkedProgram,
    heap: Option<&HeapExecution<'_>>,
) -> Option<DynamicMethodTarget> {
    let type_name = match classify_dynamic_receiver(receiver, heap, None) {
        DynamicReceiverKind::ScriptRecord { type_name }
        | DynamicReceiverKind::ScriptEnum { type_name } => type_name,
        _ => return None,
    };
    program
        .script_method_dispatch(&type_name, method)
        .and_then(|dispatch| {
            let function = match program.method_dispatch(dispatch)?.kind {
                LinkedMethodDispatchKind::Script { function, .. } => function,
                _ => return None,
            };
            Some(DynamicMethodTarget::Script { dispatch, function })
        })
}

fn resolve_host_dynamic_method(
    receiver: &Value,
    method: &str,
    registry: Option<&TypeRegistry>,
) -> Option<DynamicMethodTarget> {
    let Value::HostRef(reference) = receiver else {
        return None;
    };
    let desc = registry?.type_of_host(*reference)?;
    desc.methods
        .iter()
        .find(|candidate| candidate.name == method)
        .map(|candidate| DynamicMethodTarget::Host {
            method_id: candidate.id,
        })
}

fn standard_method_id_for_receiver(
    receiver: DynamicReceiverKind,
    method: &str,
) -> Option<MethodId> {
    let ids = std_method_ids();
    match receiver {
        DynamicReceiverKind::String => string_method_id(ids, method),
        DynamicReceiverKind::Bytes => bytes_method_id(ids, method),
        DynamicReceiverKind::Array => array_method_id(ids, method),
        DynamicReceiverKind::Map => map_method_id(ids, method),
        DynamicReceiverKind::Set => set_method_id(ids, method),
        DynamicReceiverKind::Iterator => iterator_method_id(ids, method),
        DynamicReceiverKind::Option => option_method_id(ids, method),
        DynamicReceiverKind::Result => result_method_id(ids, method),
        DynamicReceiverKind::Range => range_method_id(ids, method),
        DynamicReceiverKind::ScriptRecord { .. }
        | DynamicReceiverKind::ScriptEnum { .. }
        | DynamicReceiverKind::Host { .. }
        | DynamicReceiverKind::Unsupported => None,
    }
}

fn string_method_id(ids: &StdMethodIds, method: &str) -> Option<MethodId> {
    Some(match method {
        "len" => ids.string_len,
        "is_empty" => ids.string_is_empty,
        "contains" => ids.string_contains,
        "find" => ids.string_find,
        "starts_with" => ids.string_starts_with,
        "ends_with" => ids.string_ends_with,
        "strip_prefix" => ids.string_strip_prefix,
        "strip_suffix" => ids.string_strip_suffix,
        "to_upper" => ids.string_to_upper,
        "to_lower" => ids.string_to_lower,
        "trim" => ids.string_trim,
        "trim_start" => ids.string_trim_start,
        "trim_end" => ids.string_trim_end,
        "replace" => ids.string_replace,
        "repeat" => ids.string_repeat,
        "slice" => ids.string_slice,
        "split" => ids.string_split,
        "split_once" => ids.string_split_once,
        "split_lines" => ids.string_split_lines,
        "split_whitespace" => ids.string_split_whitespace,
        "parse_int" => ids.string_parse_int,
        "parse_float" => ids.string_parse_float,
        "parse_bool" => ids.string_parse_bool,
        "chars" => ids.string_chars,
        "bytes" => ids.string_bytes,
        _ => return None,
    })
}

fn bytes_method_id(ids: &StdMethodIds, method: &str) -> Option<MethodId> {
    Some(match method {
        "len" => ids.bytes_len,
        "is_empty" => ids.bytes_is_empty,
        "slice" => ids.bytes_slice,
        "get" => ids.bytes_get,
        "read_u32_le" => ids.bytes_read_u32_le,
        "read_u32_be" => ids.bytes_read_u32_be,
        "to_hex" => ids.bytes_to_hex,
        _ => return None,
    })
}

fn array_method_id(ids: &StdMethodIds, method: &str) -> Option<MethodId> {
    Some(match method {
        "len" => ids.array_len,
        "is_empty" => ids.array_is_empty,
        "push" => ids.array_push,
        "pop" => ids.array_pop,
        "insert" => ids.array_insert,
        "extend" => ids.array_extend,
        "clear" => ids.array_clear,
        "first" => ids.array_first,
        "last" => ids.array_last,
        "remove_at" => ids.array_remove_at,
        "join" => ids.array_join,
        "contains" => ids.array_contains,
        "index_of" => ids.array_index_of,
        "distinct" => ids.array_distinct,
        "reverse" => ids.array_reverse,
        "slice" => ids.array_slice,
        "sort" => ids.array_sort,
        "min" => ids.array_min,
        "max" => ids.array_max,
        "sum" => ids.array_sum,
        "iter" => ids.array_iter,
        "values" => ids.array_values,
        _ => return None,
    })
}

fn map_method_id(ids: &StdMethodIds, method: &str) -> Option<MethodId> {
    Some(match method {
        "len" => ids.map_len,
        "is_empty" => ids.map_is_empty,
        "has" => ids.map_has,
        "get" => ids.map_get,
        "get_or" => ids.map_get_or,
        "set" => ids.map_set,
        "remove" => ids.map_remove,
        "extend" => ids.map_extend,
        "clear" => ids.map_clear,
        "keys" => ids.map_keys,
        "values" => ids.map_values,
        "entries" => ids.map_entries,
        "merge" => ids.map_merge,
        "iter" => ids.map_iter,
        _ => return None,
    })
}

fn set_method_id(ids: &StdMethodIds, method: &str) -> Option<MethodId> {
    Some(match method {
        "len" => ids.set_len,
        "is_empty" => ids.set_is_empty,
        "has" => ids.set_has,
        "add" => ids.set_add,
        "remove" => ids.set_remove,
        "extend" => ids.set_extend,
        "clear" => ids.set_clear,
        "values" => ids.set_values,
        "union" => ids.set_union,
        "intersection" => ids.set_intersection,
        "difference" => ids.set_difference,
        "symmetric_difference" => ids.set_symmetric_difference,
        "is_subset" => ids.set_is_subset,
        "is_superset" => ids.set_is_superset,
        "is_disjoint" => ids.set_is_disjoint,
        "iter" => ids.set_iter,
        _ => return None,
    })
}

fn option_method_id(ids: &StdMethodIds, method: &str) -> Option<MethodId> {
    Some(match method {
        "is_some" => ids.option_is_some,
        "is_none" => ids.option_is_none,
        "unwrap_or" => ids.option_unwrap_or,
        "ok_or" => ids.option_ok_or,
        "flatten" => ids.option_flatten,
        _ => return None,
    })
}

fn result_method_id(ids: &StdMethodIds, method: &str) -> Option<MethodId> {
    Some(match method {
        "is_ok" => ids.result_is_ok,
        "is_err" => ids.result_is_err,
        "unwrap_or" => ids.result_unwrap_or,
        "to_option" => ids.result_to_option,
        "to_error_option" => ids.result_to_error_option,
        "flatten" => ids.result_flatten,
        _ => return None,
    })
}

fn range_method_id(ids: &StdMethodIds, method: &str) -> Option<MethodId> {
    Some(match method {
        "len" => ids.range_len,
        "is_empty" => ids.range_is_empty,
        "iter" => ids.range_iter,
        _ => return None,
    })
}

fn iterator_method_id(ids: &StdMethodIds, method: &str) -> Option<MethodId> {
    Some(match method {
        "next" => ids.iterator_next,
        "count" => ids.iterator_count,
        "any" => ids.iterator_any,
        "all" => ids.iterator_all,
        "find" => ids.iterator_find,
        "map" => ids.iterator_map,
        "filter" => ids.iterator_filter,
        "take" => ids.iterator_take,
        "skip" => ids.iterator_skip,
        "collect_array" => ids.iterator_collect_array,
        _ => return None,
    })
}
