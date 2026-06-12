use crate::heap::HeapValue;
use crate::std_method_ids::std_method_ids;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheEntry,
    StandardMethodInlineCacheTarget, StandardMethodReceiver, Value, VmError, VmErrorKind, VmResult,
    array_methods, bytes_methods, map_methods, option_result_methods, set_methods,
};
use vela_def::MethodId;

pub(crate) fn call(
    receiver: &mut Value,
    method: &str,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    if bytes_methods::is_bytes(receiver, heap.as_deref())
        && let Some(result) = call_bytes_by_name(receiver, method, args, heap, budget)
    {
        return Some(result);
    }
    let result = match method {
        "len" => expect_no_args(method, args)
            .and_then(|()| len(receiver, heap.as_deref()).map(Value::i64)),
        "is_empty" => expect_no_args(method, args)
            .and_then(|()| is_empty(receiver, heap.as_deref()).map(Value::Bool)),
        "contains" => array_methods::contains(receiver, args, heap.as_deref()).map(Value::Bool),
        "slice" => array_methods::slice(receiver, args, heap, budget),
        "push" => array_methods::push(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "pop" => array_methods::pop(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "insert" => {
            array_methods::insert(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
        }
        "extend" => extend(receiver, args, heap, budget),
        "first" => array_methods::first(receiver, args, heap, budget),
        "last" => array_methods::last(receiver, args, heap, budget),
        "remove_at" => {
            array_methods::remove_at(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
        }
        "join" => array_methods::join(receiver, args, heap, budget),
        "index_of" => array_methods::index_of(receiver, args, heap, budget),
        "distinct" => array_methods::distinct(receiver, args, heap, budget),
        "reverse" => array_methods::reverse(receiver, args, heap, budget),
        "sort" => array_methods::sort(receiver, args, heap, budget),
        "min" => array_methods::min(receiver, args, heap, budget),
        "max" => array_methods::max(receiver, args, heap, budget),
        "is_some" => option_result_methods::is_some(receiver, args, heap.as_deref()),
        "is_none" => option_result_methods::is_none(receiver, args, heap.as_deref()),
        "is_ok" => option_result_methods::is_ok(receiver, args, heap.as_deref()),
        "is_err" => option_result_methods::is_err(receiver, args, heap.as_deref()),
        "unwrap_or" => option_result_methods::unwrap_or(receiver, args, heap.as_deref()),
        "ok_or" => option_result_methods::ok_or(receiver, args, heap, budget),
        "to_option" => option_result_methods::to_option(receiver, args, heap, budget),
        "to_error_option" => option_result_methods::to_error_option(receiver, args, heap, budget),
        "flatten" => flatten(receiver, args, heap, budget),
        "merge" => map_methods::merge(receiver, args, heap, budget),
        "has" => has(receiver, args, heap.as_deref()).map(Value::Bool),
        "get" => map_methods::get(receiver, args, heap, budget),
        "get_or" => map_methods::get_or(receiver, args, heap.as_deref()),
        "add" => set_methods::add(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "set" => map_methods::set(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "remove" => remove(receiver, args, heap, budget),
        "clear" => clear(receiver, args, heap),
        "keys" => map_methods::keys(receiver, args, heap, budget),
        "values" => values(receiver, args, heap, budget),
        "union" => set_methods::union(receiver, args, heap, budget),
        "intersection" => set_methods::intersection(receiver, args, heap, budget),
        "difference" => set_methods::difference(receiver, args, heap, budget),
        "symmetric_difference" => set_methods::symmetric_difference(receiver, args, heap, budget),
        "is_subset" => set_methods::is_subset(receiver, args, heap.as_deref()).map(Value::Bool),
        "is_superset" => set_methods::is_superset(receiver, args, heap.as_deref()).map(Value::Bool),
        "is_disjoint" => set_methods::is_disjoint(receiver, args, heap.as_deref()).map(Value::Bool),
        "entries" => map_methods::entries(receiver, args, heap, budget),
        _ => return None,
    };
    Some(result)
}

pub(crate) fn call_by_id(
    receiver: &mut Value,
    method_id: MethodId,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let ids = std_method_ids();
    if let Some(result) = call_readonly_by_id(receiver, method_id, args, heap.as_deref()) {
        return Some(result);
    }
    if method_id == ids.array_push && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::push(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.array_pop && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::pop(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.array_insert && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::insert(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.array_extend && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::extend(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.array_clear && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::clear(receiver, args, heap.as_deref_mut()));
    }
    if method_id == ids.array_first && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::first(receiver, args, heap, budget));
    }
    if method_id == ids.array_last && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::last(receiver, args, heap, budget));
    }
    if method_id == ids.array_remove_at && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::remove_at(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.array_index_of && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::index_of(receiver, args, heap, budget));
    }
    if method_id == ids.array_join && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::join(receiver, args, heap, budget));
    }
    if method_id == ids.array_distinct && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::distinct(receiver, args, heap, budget));
    }
    if method_id == ids.array_reverse && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::reverse(receiver, args, heap, budget));
    }
    if method_id == ids.array_slice && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::slice(receiver, args, heap, budget));
    }
    if method_id == ids.array_sort && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::sort(receiver, args, heap, budget));
    }
    if method_id == ids.array_min && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::min(receiver, args, heap, budget));
    }
    if method_id == ids.array_max && array_methods::is_array(receiver, heap.as_deref()) {
        return Some(array_methods::max(receiver, args, heap, budget));
    }
    if method_id == ids.bytes_slice && bytes_methods::is_bytes(receiver, heap.as_deref()) {
        return Some(bytes_methods::slice(receiver, args, heap, budget));
    }
    if method_id == ids.bytes_to_hex && bytes_methods::is_bytes(receiver, heap.as_deref()) {
        return Some(bytes_methods::to_hex(receiver, args, heap, budget));
    }
    if method_id == ids.map_get && map_methods::is_map(receiver, heap.as_deref()) {
        return Some(map_methods::get(receiver, args, heap, budget));
    }
    if method_id == ids.map_set && map_methods::is_map(receiver, heap.as_deref()) {
        return Some(map_methods::set(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.map_remove && map_methods::is_map(receiver, heap.as_deref()) {
        return Some(map_methods::remove(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.map_extend && map_methods::is_map(receiver, heap.as_deref()) {
        return Some(map_methods::extend(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.map_clear && map_methods::is_map(receiver, heap.as_deref()) {
        return Some(map_methods::clear(receiver, args, heap.as_deref_mut()));
    }
    if method_id == ids.map_keys && map_methods::is_map(receiver, heap.as_deref()) {
        return Some(map_methods::keys(receiver, args, heap, budget));
    }
    if method_id == ids.map_values && map_methods::is_map(receiver, heap.as_deref()) {
        return Some(map_methods::values(receiver, args, heap, budget));
    }
    if method_id == ids.map_entries && map_methods::is_map(receiver, heap.as_deref()) {
        return Some(map_methods::entries(receiver, args, heap, budget));
    }
    if method_id == ids.map_merge && map_methods::is_map(receiver, heap.as_deref()) {
        return Some(map_methods::merge(receiver, args, heap, budget));
    }
    if method_id == ids.set_add && set_methods::is_set(receiver, heap.as_deref()) {
        return Some(set_methods::add(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.set_remove && set_methods::is_set(receiver, heap.as_deref()) {
        return Some(set_methods::remove(receiver, args, heap.as_deref_mut()));
    }
    if method_id == ids.set_extend && set_methods::is_set(receiver, heap.as_deref()) {
        return Some(set_methods::extend(
            receiver,
            args,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        ));
    }
    if method_id == ids.set_clear && set_methods::is_set(receiver, heap.as_deref()) {
        return Some(set_methods::clear(receiver, args, heap.as_deref_mut()));
    }
    if method_id == ids.set_values && set_methods::is_set(receiver, heap.as_deref()) {
        return Some(set_methods::values(receiver, args, heap, budget));
    }
    if method_id == ids.set_union && set_methods::is_set(receiver, heap.as_deref()) {
        return Some(set_methods::union(receiver, args, heap, budget));
    }
    if method_id == ids.set_intersection && set_methods::is_set(receiver, heap.as_deref()) {
        return Some(set_methods::intersection(receiver, args, heap, budget));
    }
    if method_id == ids.set_difference && set_methods::is_set(receiver, heap.as_deref()) {
        return Some(set_methods::difference(receiver, args, heap, budget));
    }
    if method_id == ids.set_symmetric_difference && set_methods::is_set(receiver, heap.as_deref()) {
        return Some(set_methods::symmetric_difference(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.option_ok_or && option_result_methods::is_option(receiver, heap.as_deref())
    {
        return Some(option_result_methods::ok_or(receiver, args, heap, budget));
    }
    if method_id == ids.option_flatten
        && option_result_methods::is_option(receiver, heap.as_deref())
    {
        return Some(flatten(receiver, args, heap, budget));
    }
    if method_id == ids.result_to_option
        && option_result_methods::is_result(receiver, heap.as_deref())
    {
        return Some(option_result_methods::to_option(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.result_to_error_option
        && option_result_methods::is_result(receiver, heap.as_deref())
    {
        return Some(option_result_methods::to_error_option(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.result_flatten
        && option_result_methods::is_result(receiver, heap.as_deref())
    {
        return Some(flatten(receiver, args, heap, budget));
    }
    if method_id == ids.string_to_upper
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::to_upper(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_to_lower
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::to_lower(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_trim && crate::string_methods::is_string(receiver, heap.as_deref()) {
        return Some(crate::string_methods::trim(receiver, args, heap, budget));
    }
    if method_id == ids.string_trim_start
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::trim_start(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_trim_end
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::trim_end(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_replace
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::replace(receiver, args, heap, budget));
    }
    if method_id == ids.string_repeat && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::repeat(receiver, args, heap, budget));
    }
    if method_id == ids.string_slice && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::slice(receiver, args, heap, budget));
    }
    if method_id == ids.string_find && crate::string_methods::is_string(receiver, heap.as_deref()) {
        return Some(crate::string_methods::find(receiver, args, heap, budget));
    }
    if method_id == ids.string_strip_prefix
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::strip_prefix(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_strip_suffix
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::strip_suffix(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_char_at
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::char_at(receiver, args, heap, budget));
    }
    if method_id == ids.string_split && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::split(receiver, args, heap, budget));
    }
    if method_id == ids.string_split_once
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::split_once(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_split_lines
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::split_lines(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_split_whitespace
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::split_whitespace(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_parse_int
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::parse_int(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_parse_float
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::parse_float(
            receiver, args, heap, budget,
        ));
    }
    if method_id == ids.string_parse_bool
        && crate::string_methods::is_string(receiver, heap.as_deref())
    {
        return Some(crate::string_methods::parse_bool(
            receiver, args, heap, budget,
        ));
    }
    None
}

pub(crate) fn call_readonly(
    receiver: &Value,
    method: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    if bytes_methods::is_bytes(receiver, heap)
        && let Some(result) = call_readonly_bytes_by_name(receiver, method, args, heap)
    {
        return Some(result);
    }
    let result = match method {
        "len" => expect_no_args(method, args).and_then(|()| len(receiver, heap).map(Value::i64)),
        "is_empty" => {
            expect_no_args(method, args).and_then(|()| is_empty(receiver, heap).map(Value::Bool))
        }
        "contains" => array_methods::contains(receiver, args, heap).map(Value::Bool),
        "is_some" => option_result_methods::is_some(receiver, args, heap),
        "is_none" => option_result_methods::is_none(receiver, args, heap),
        "is_ok" => option_result_methods::is_ok(receiver, args, heap),
        "is_err" => option_result_methods::is_err(receiver, args, heap),
        "unwrap_or" => option_result_methods::unwrap_or(receiver, args, heap),
        "has" => has(receiver, args, heap).map(Value::Bool),
        "get_or" => map_methods::get_or(receiver, args, heap),
        "is_subset" => set_methods::is_subset(receiver, args, heap).map(Value::Bool),
        "is_superset" => set_methods::is_superset(receiver, args, heap).map(Value::Bool),
        "is_disjoint" => set_methods::is_disjoint(receiver, args, heap).map(Value::Bool),
        _ => return None,
    };
    Some(result)
}

pub(crate) fn call_readonly_by_id(
    receiver: &Value,
    method_id: MethodId,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let ids = std_method_ids();
    if method_id == ids.string_len && crate::string_methods::is_string(receiver, heap) {
        return Some(
            expect_no_args("len", args).and_then(|()| len(receiver, heap).map(Value::i64)),
        );
    }
    if method_id == ids.string_is_empty && crate::string_methods::is_string(receiver, heap) {
        return Some(
            expect_no_args("is_empty", args)
                .and_then(|()| is_empty(receiver, heap).map(Value::Bool)),
        );
    }
    if method_id == ids.string_contains && crate::string_methods::is_string(receiver, heap) {
        return Some(crate::string_methods::contains(receiver, args, heap).map(Value::Bool));
    }
    if method_id == ids.string_starts_with && crate::string_methods::is_string(receiver, heap) {
        return Some(crate::string_methods::starts_with(receiver, args, heap).map(Value::Bool));
    }
    if method_id == ids.string_ends_with && crate::string_methods::is_string(receiver, heap) {
        return Some(crate::string_methods::ends_with(receiver, args, heap).map(Value::Bool));
    }
    if method_id == ids.bytes_len && bytes_methods::is_bytes(receiver, heap) {
        return Some(bytes_methods::len(receiver, args, heap));
    }
    if method_id == ids.bytes_is_empty && bytes_methods::is_bytes(receiver, heap) {
        return Some(bytes_methods::is_empty(receiver, args, heap));
    }
    if method_id == ids.bytes_get && bytes_methods::is_bytes(receiver, heap) {
        return Some(bytes_methods::get(receiver, args, heap));
    }
    if method_id == ids.bytes_read_u32_le && bytes_methods::is_bytes(receiver, heap) {
        return Some(bytes_methods::read_u32_le(receiver, args, heap));
    }
    if method_id == ids.bytes_read_u32_be && bytes_methods::is_bytes(receiver, heap) {
        return Some(bytes_methods::read_u32_be(receiver, args, heap));
    }
    if method_id == ids.range_len && matches!(receiver, Value::Range(_)) {
        return Some(
            expect_no_args("len", args).and_then(|()| len(receiver, heap).map(Value::i64)),
        );
    }
    if method_id == ids.range_is_empty && matches!(receiver, Value::Range(_)) {
        return Some(
            expect_no_args("is_empty", args)
                .and_then(|()| is_empty(receiver, heap).map(Value::Bool)),
        );
    }
    if method_id == ids.array_len && array_methods::is_array(receiver, heap) {
        return Some(
            expect_no_args("len", args).and_then(|()| len(receiver, heap).map(Value::i64)),
        );
    }
    if method_id == ids.array_is_empty && array_methods::is_array(receiver, heap) {
        return Some(
            expect_no_args("is_empty", args)
                .and_then(|()| is_empty(receiver, heap).map(Value::Bool)),
        );
    }
    if method_id == ids.array_contains && array_methods::is_array(receiver, heap) {
        return Some(array_methods::contains(receiver, args, heap).map(Value::Bool));
    }
    if method_id == ids.map_len && map_methods::is_map(receiver, heap) {
        return Some(
            expect_no_args("len", args).and_then(|()| len(receiver, heap).map(Value::i64)),
        );
    }
    if method_id == ids.map_is_empty && map_methods::is_map(receiver, heap) {
        return Some(
            expect_no_args("is_empty", args)
                .and_then(|()| is_empty(receiver, heap).map(Value::Bool)),
        );
    }
    if method_id == ids.map_has && map_methods::is_map(receiver, heap) {
        return Some(map_methods::has(receiver, args, heap).map(Value::Bool));
    }
    if method_id == ids.map_get_or && map_methods::is_map(receiver, heap) {
        return Some(map_methods::get_or(receiver, args, heap));
    }
    if method_id == ids.set_len && set_methods::is_set(receiver, heap) {
        return Some(
            expect_no_args("len", args).and_then(|()| len(receiver, heap).map(Value::i64)),
        );
    }
    if method_id == ids.set_is_empty && set_methods::is_set(receiver, heap) {
        return Some(
            expect_no_args("is_empty", args)
                .and_then(|()| is_empty(receiver, heap).map(Value::Bool)),
        );
    }
    if method_id == ids.set_has && set_methods::is_set(receiver, heap) {
        return Some(set_methods::has(receiver, args, heap).map(Value::Bool));
    }
    if method_id == ids.set_is_subset && set_methods::is_set(receiver, heap) {
        return Some(set_methods::is_subset(receiver, args, heap).map(Value::Bool));
    }
    if method_id == ids.set_is_superset && set_methods::is_set(receiver, heap) {
        return Some(set_methods::is_superset(receiver, args, heap).map(Value::Bool));
    }
    if method_id == ids.set_is_disjoint && set_methods::is_set(receiver, heap) {
        return Some(set_methods::is_disjoint(receiver, args, heap).map(Value::Bool));
    }
    if method_id == ids.option_is_some && option_result_methods::is_option(receiver, heap) {
        return Some(option_result_methods::is_some(receiver, args, heap));
    }
    if method_id == ids.option_is_none && option_result_methods::is_option(receiver, heap) {
        return Some(option_result_methods::is_none(receiver, args, heap));
    }
    if method_id == ids.option_unwrap_or && option_result_methods::is_option(receiver, heap) {
        return Some(option_result_methods::unwrap_or(receiver, args, heap));
    }
    if method_id == ids.result_is_ok && option_result_methods::is_result(receiver, heap) {
        return Some(option_result_methods::is_ok(receiver, args, heap));
    }
    if method_id == ids.result_is_err && option_result_methods::is_result(receiver, heap) {
        return Some(option_result_methods::is_err(receiver, args, heap));
    }
    if method_id == ids.result_unwrap_or && option_result_methods::is_result(receiver, heap) {
        return Some(option_result_methods::unwrap_or(receiver, args, heap));
    }
    None
}

pub(crate) fn standard_cache_entry(
    method_id: MethodId,
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<StandardMethodInlineCacheEntry> {
    let ids = std_method_ids();
    let receiver = if crate::string_methods::is_string(receiver, heap) {
        StandardMethodReceiver::String
    } else if bytes_methods::is_bytes(receiver, heap) {
        StandardMethodReceiver::Bytes
    } else if matches!(receiver, Value::Range(_)) {
        StandardMethodReceiver::Range
    } else if array_methods::is_array(receiver, heap) {
        StandardMethodReceiver::Array
    } else if map_methods::is_map(receiver, heap) {
        StandardMethodReceiver::Map
    } else if set_methods::is_set(receiver, heap) {
        StandardMethodReceiver::Set
    } else if option_result_methods::is_option(receiver, heap) {
        StandardMethodReceiver::Option
    } else if option_result_methods::is_result(receiver, heap) {
        StandardMethodReceiver::Result
    } else {
        return None;
    };
    let target = match (receiver, method_id) {
        (StandardMethodReceiver::String, id) if id == ids.string_len => {
            StandardMethodInlineCacheTarget::Len
        }
        (StandardMethodReceiver::String, id) if id == ids.string_is_empty => {
            StandardMethodInlineCacheTarget::IsEmpty
        }
        (StandardMethodReceiver::String, id) if id == ids.string_contains => {
            StandardMethodInlineCacheTarget::Contains
        }
        (StandardMethodReceiver::String, id) if id == ids.string_starts_with => {
            StandardMethodInlineCacheTarget::StartsWith
        }
        (StandardMethodReceiver::String, id) if id == ids.string_ends_with => {
            StandardMethodInlineCacheTarget::EndsWith
        }
        (StandardMethodReceiver::String, id) if id == ids.string_find => {
            StandardMethodInlineCacheTarget::Find
        }
        (StandardMethodReceiver::String, id) if id == ids.string_strip_prefix => {
            StandardMethodInlineCacheTarget::StripPrefix
        }
        (StandardMethodReceiver::String, id) if id == ids.string_strip_suffix => {
            StandardMethodInlineCacheTarget::StripSuffix
        }
        (StandardMethodReceiver::String, id) if id == ids.string_parse_int => {
            StandardMethodInlineCacheTarget::ParseInt
        }
        (StandardMethodReceiver::String, id) if id == ids.string_parse_float => {
            StandardMethodInlineCacheTarget::ParseFloat
        }
        (StandardMethodReceiver::String, id) if id == ids.string_parse_bool => {
            StandardMethodInlineCacheTarget::ParseBool
        }
        (StandardMethodReceiver::String, id) if id == ids.string_to_upper => {
            StandardMethodInlineCacheTarget::ToUpper
        }
        (StandardMethodReceiver::String, id) if id == ids.string_to_lower => {
            StandardMethodInlineCacheTarget::ToLower
        }
        (StandardMethodReceiver::String, id) if id == ids.string_trim => {
            StandardMethodInlineCacheTarget::Trim
        }
        (StandardMethodReceiver::String, id) if id == ids.string_trim_start => {
            StandardMethodInlineCacheTarget::TrimStart
        }
        (StandardMethodReceiver::String, id) if id == ids.string_trim_end => {
            StandardMethodInlineCacheTarget::TrimEnd
        }
        (StandardMethodReceiver::Bytes, id) if id == ids.bytes_len => {
            StandardMethodInlineCacheTarget::Len
        }
        (StandardMethodReceiver::Bytes, id) if id == ids.bytes_is_empty => {
            StandardMethodInlineCacheTarget::IsEmpty
        }
        (StandardMethodReceiver::Bytes, id) if id == ids.bytes_get => {
            StandardMethodInlineCacheTarget::Get
        }
        (StandardMethodReceiver::Bytes, id) if id == ids.bytes_slice => {
            StandardMethodInlineCacheTarget::Slice
        }
        (StandardMethodReceiver::Bytes, id) if id == ids.bytes_to_hex => {
            StandardMethodInlineCacheTarget::ToHex
        }
        (StandardMethodReceiver::Bytes, id) if id == ids.bytes_read_u32_le => {
            StandardMethodInlineCacheTarget::ReadU32Le
        }
        (StandardMethodReceiver::Bytes, id) if id == ids.bytes_read_u32_be => {
            StandardMethodInlineCacheTarget::ReadU32Be
        }
        (StandardMethodReceiver::Range, id) if id == ids.range_len => {
            StandardMethodInlineCacheTarget::Len
        }
        (StandardMethodReceiver::Range, id) if id == ids.range_is_empty => {
            StandardMethodInlineCacheTarget::IsEmpty
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_len => {
            StandardMethodInlineCacheTarget::Len
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_is_empty => {
            StandardMethodInlineCacheTarget::IsEmpty
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_contains => {
            StandardMethodInlineCacheTarget::Contains
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_len => {
            StandardMethodInlineCacheTarget::Len
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_is_empty => {
            StandardMethodInlineCacheTarget::IsEmpty
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_has => {
            StandardMethodInlineCacheTarget::Has
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_get_or => {
            StandardMethodInlineCacheTarget::GetOr
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_len => {
            StandardMethodInlineCacheTarget::Len
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_is_empty => {
            StandardMethodInlineCacheTarget::IsEmpty
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_has => {
            StandardMethodInlineCacheTarget::Has
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_is_subset => {
            StandardMethodInlineCacheTarget::IsSubset
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_is_superset => {
            StandardMethodInlineCacheTarget::IsSuperset
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_is_disjoint => {
            StandardMethodInlineCacheTarget::IsDisjoint
        }
        (StandardMethodReceiver::Option, id) if id == ids.option_is_some => {
            StandardMethodInlineCacheTarget::IsSome
        }
        (StandardMethodReceiver::Option, id) if id == ids.option_is_none => {
            StandardMethodInlineCacheTarget::IsNone
        }
        (StandardMethodReceiver::Option, id) if id == ids.option_unwrap_or => {
            StandardMethodInlineCacheTarget::UnwrapOr
        }
        (StandardMethodReceiver::Result, id) if id == ids.result_is_ok => {
            StandardMethodInlineCacheTarget::IsOk
        }
        (StandardMethodReceiver::Result, id) if id == ids.result_is_err => {
            StandardMethodInlineCacheTarget::IsErr
        }
        (StandardMethodReceiver::Result, id) if id == ids.result_unwrap_or => {
            StandardMethodInlineCacheTarget::UnwrapOr
        }
        _ => return None,
    };
    Some(StandardMethodInlineCacheEntry { receiver, target })
}

pub(crate) fn call_standard_cached(
    receiver: &Value,
    cache: StandardMethodInlineCacheEntry,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    if let Some(result) = call_readonly_cached(receiver, cache, args, heap.as_deref()) {
        return Some(result);
    }
    if !receiver_matches_cache(receiver, cache.receiver, heap.as_deref()) {
        return None;
    }
    let result = match (cache.receiver, cache.target) {
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::ToUpper) => {
            crate::string_methods::to_upper(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::ToLower) => {
            crate::string_methods::to_lower(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Trim) => {
            crate::string_methods::trim(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::TrimStart) => {
            crate::string_methods::trim_start(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::TrimEnd) => {
            crate::string_methods::trim_end(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Find) => {
            crate::string_methods::find(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::StripPrefix) => {
            crate::string_methods::strip_prefix(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::StripSuffix) => {
            crate::string_methods::strip_suffix(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::ParseInt) => {
            crate::string_methods::parse_int(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::ParseFloat) => {
            crate::string_methods::parse_float(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::ParseBool) => {
            crate::string_methods::parse_bool(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Slice) => {
            bytes_methods::slice(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::ToHex) => {
            bytes_methods::to_hex(receiver, args, heap, budget)
        }
        _ => return None,
    };
    Some(result)
}

pub(crate) fn call_readonly_cached(
    receiver: &Value,
    cache: StandardMethodInlineCacheEntry,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    if !receiver_matches_cache(receiver, cache.receiver, heap) {
        return None;
    }
    let result = match (cache.receiver, cache.target) {
        (_, StandardMethodInlineCacheTarget::Len) => {
            expect_no_args("len", args).and_then(|()| len(receiver, heap).map(Value::i64))
        }
        (_, StandardMethodInlineCacheTarget::IsEmpty) => expect_no_args("is_empty", args)
            .and_then(|()| is_empty(receiver, heap).map(Value::Bool)),
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Contains) => {
            crate::string_methods::contains(receiver, args, heap).map(Value::Bool)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Contains) => {
            array_methods::contains(receiver, args, heap).map(Value::Bool)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::StartsWith) => {
            crate::string_methods::starts_with(receiver, args, heap).map(Value::Bool)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::EndsWith) => {
            crate::string_methods::ends_with(receiver, args, heap).map(Value::Bool)
        }
        (
            StandardMethodReceiver::Map | StandardMethodReceiver::Set,
            StandardMethodInlineCacheTarget::Has,
        ) => has(receiver, args, heap).map(Value::Bool),
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::GetOr) => {
            map_methods::get_or(receiver, args, heap)
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::IsSubset) => {
            set_methods::is_subset(receiver, args, heap).map(Value::Bool)
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::IsSuperset) => {
            set_methods::is_superset(receiver, args, heap).map(Value::Bool)
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::IsDisjoint) => {
            set_methods::is_disjoint(receiver, args, heap).map(Value::Bool)
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Get) => {
            bytes_methods::get(receiver, args, heap)
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::ReadU32Le) => {
            bytes_methods::read_u32_le(receiver, args, heap)
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::ReadU32Be) => {
            bytes_methods::read_u32_be(receiver, args, heap)
        }
        (StandardMethodReceiver::Option, StandardMethodInlineCacheTarget::IsSome) => {
            option_result_methods::is_some(receiver, args, heap)
        }
        (StandardMethodReceiver::Option, StandardMethodInlineCacheTarget::IsNone) => {
            option_result_methods::is_none(receiver, args, heap)
        }
        (
            StandardMethodReceiver::Option | StandardMethodReceiver::Result,
            StandardMethodInlineCacheTarget::UnwrapOr,
        ) => option_result_methods::unwrap_or(receiver, args, heap),
        (StandardMethodReceiver::Result, StandardMethodInlineCacheTarget::IsOk) => {
            option_result_methods::is_ok(receiver, args, heap)
        }
        (StandardMethodReceiver::Result, StandardMethodInlineCacheTarget::IsErr) => {
            option_result_methods::is_err(receiver, args, heap)
        }
        _ => return None,
    };
    Some(result)
}

fn receiver_matches_cache(
    receiver: &Value,
    cached: StandardMethodReceiver,
    heap: Option<&HeapExecution<'_>>,
) -> bool {
    match cached {
        StandardMethodReceiver::String => crate::string_methods::is_string(receiver, heap),
        StandardMethodReceiver::Bytes => bytes_methods::is_bytes(receiver, heap),
        StandardMethodReceiver::Range => matches!(receiver, Value::Range(_)),
        StandardMethodReceiver::Array => array_methods::is_array(receiver, heap),
        StandardMethodReceiver::Map => map_methods::is_map(receiver, heap),
        StandardMethodReceiver::Set => set_methods::is_set(receiver, heap),
        StandardMethodReceiver::Option => option_result_methods::is_option(receiver, heap),
        StandardMethodReceiver::Result => option_result_methods::is_result(receiver, heap),
    }
}

fn call_bytes_by_name(
    receiver: &Value,
    method: &str,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match method {
        "len" | "is_empty" | "get" | "read_u32_le" | "read_u32_be" => {
            call_readonly_bytes_by_name(receiver, method, args, heap.as_deref())
        }
        "slice" => Some(bytes_methods::slice(receiver, args, heap, budget)),
        "to_hex" => Some(bytes_methods::to_hex(receiver, args, heap, budget)),
        _ => None,
    }
}

fn call_readonly_bytes_by_name(
    receiver: &Value,
    method: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    match method {
        "len" => Some(bytes_methods::len(receiver, args, heap)),
        "is_empty" => Some(bytes_methods::is_empty(receiver, args, heap)),
        "get" => Some(bytes_methods::get(receiver, args, heap)),
        "read_u32_le" => Some(bytes_methods::read_u32_le(receiver, args, heap)),
        "read_u32_be" => Some(bytes_methods::read_u32_be(receiver, args, heap)),
        _ => None,
    }
}

fn extend(
    receiver: &mut Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap.as_deref()) {
        set_methods::extend(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
    } else if map_methods::is_map(receiver, heap.as_deref()) {
        map_methods::extend(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
    } else {
        array_methods::extend(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
    }
}

fn flatten(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if option_result_methods::is_option_or_result(receiver, heap.as_deref()) {
        option_result_methods::flatten(receiver, args, heap, budget)
    } else {
        Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "method flatten",
        }))
    }
}

fn has(receiver: &Value, args: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
    if set_methods::is_set(receiver, heap) {
        set_methods::has(receiver, args, heap)
    } else {
        map_methods::has(receiver, args, heap)
    }
}

fn remove(
    receiver: &mut Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap.as_deref()) {
        set_methods::remove(receiver, args, heap.as_deref_mut())
    } else {
        map_methods::remove(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
    }
}

fn clear(
    receiver: &mut Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap.as_deref()) {
        set_methods::clear(receiver, args, heap.as_deref_mut())
    } else if map_methods::is_map(receiver, heap.as_deref()) {
        map_methods::clear(receiver, args, heap.as_deref_mut())
    } else {
        array_methods::clear(receiver, args, heap.as_deref_mut())
    }
}

fn values(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap.as_deref()) {
        set_methods::values(receiver, args, heap, budget)
    } else {
        map_methods::values(receiver, args, heap, budget)
    }
}

fn len(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<i64> {
    match receiver {
        Value::Range(range) => range.len().ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "method len",
            })
        }),
        Value::HeapRef(reference) => {
            let Some(value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return type_error("method len");
            };
            match value {
                HeapValue::String(value) => usize_to_i64(string_char_len(value), "method len"),
                HeapValue::Bytes(value) => usize_to_i64(value.len(), "method len"),
                HeapValue::Array(values) | HeapValue::Set(values) => {
                    usize_to_i64(values.len(), "method len")
                }
                HeapValue::Map(values) => usize_to_i64(values.len(), "method len"),
                HeapValue::Record { fields: values, .. }
                | HeapValue::Enum { fields: values, .. } => {
                    usize_to_i64(values.len(), "method len")
                }
                HeapValue::Closure(_) | HeapValue::Iterator(_) | HeapValue::PathProxy(_) => {
                    type_error("method len")
                }
            }
        }
        _ => type_error("method len"),
    }
}

fn string_char_len(value: &str) -> usize {
    if value.is_ascii() {
        value.len()
    } else {
        value.chars().count()
    }
}

fn is_empty(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
    match receiver {
        Value::Range(range) => Ok(range.is_empty()),
        Value::HeapRef(reference) => {
            let Some(value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return type_error("method is_empty");
            };
            match value {
                HeapValue::String(value) => Ok(value.is_empty()),
                HeapValue::Bytes(value) => Ok(value.is_empty()),
                HeapValue::Array(values) | HeapValue::Set(values) => Ok(values.is_empty()),
                HeapValue::Map(values) => Ok(values.is_empty()),
                HeapValue::Record { fields: values, .. }
                | HeapValue::Enum { fields: values, .. } => Ok(values.is_empty()),
                HeapValue::Closure(_) | HeapValue::Iterator(_) | HeapValue::PathProxy(_) => {
                    type_error("method is_empty")
                }
            }
        }
        _ => type_error("method is_empty"),
    }
}

fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
    expect_arity(method, args, 0)
}

fn expect_arity(method: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: method.to_owned(),
        expected,
        actual: args.len(),
    }))
}

fn usize_to_i64(value: usize, operation: &'static str) -> VmResult<i64> {
    i64::try_from(value).map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_function_source_with_registry;
    use vela_bytecode::{Linker, UnlinkedCodeObject, UnlinkedProgram};
    use vela_common::SourceId;

    use crate::{ExecutionBudget, OwnedValue, Vm, VmResult};

    fn compile_standard_function_source(
        source: SourceId,
        text: &str,
        function_name: &str,
    ) -> vela_bytecode::compiler::error::CompileResult<UnlinkedCodeObject> {
        let registry = vela_stdlib::standard_registry().expect("standard registry should build");
        compile_function_source_with_registry(source, text, function_name, registry.compile_view())
    }

    fn run_linked_builtin_test_code(
        code: UnlinkedCodeObject,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let entry = code.name.clone();
        let mut program = UnlinkedProgram::new();
        program.insert_function(code);
        let linked = Linker::new()
            .link_program(&program)
            .expect("builtin method test program should link");
        Vm::new().run_linked_program_with_budget(&linked, &entry, &[], budget)
    }

    #[test]
    fn string_len_counts_unicode_characters() {
        let source = r#"
fn main() {
    return "quest".len() * 100 + "é日".len();
}
"#;
        let code = compile_standard_function_source(SourceId::new(1), source, "main")
            .expect("string len source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result =
            run_linked_builtin_test_code(code, &mut budget).expect("string len should run");
        assert_eq!(
            result,
            OwnedValue::Scalar(vela_common::ScalarValue::I64(502))
        );
    }

    #[test]
    fn managed_heap_string_len_counts_unicode_characters() {
        let source = r#"
fn main() {
    let ascii = "quest";
    let unicode = "é日";
    return ascii.len() * 100 + unicode.len();
}
"#;
        let code = compile_standard_function_source(SourceId::new(1), source, "main")
            .expect("managed heap string len source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = run_linked_builtin_test_code(code, &mut budget)
            .expect("managed heap string len should run");
        assert_eq!(
            result,
            OwnedValue::Scalar(vela_common::ScalarValue::I64(502))
        );
    }
}
