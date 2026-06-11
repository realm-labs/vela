use std::sync::OnceLock;

use crate::heap::HeapValue;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, array_methods,
    bytes_methods, map_methods, option_result_methods, set_methods,
};
use vela_def::MethodId;

#[derive(Clone, Copy)]
struct StdMethodIds {
    string_len: MethodId,
    string_is_empty: MethodId,
    string_contains: MethodId,
    string_find: MethodId,
    string_starts_with: MethodId,
    string_ends_with: MethodId,
    string_strip_prefix: MethodId,
    string_strip_suffix: MethodId,
    string_to_upper: MethodId,
    string_to_lower: MethodId,
    string_trim: MethodId,
    string_trim_start: MethodId,
    string_trim_end: MethodId,
    string_replace: MethodId,
    string_repeat: MethodId,
    string_slice: MethodId,
    string_split: MethodId,
    string_split_once: MethodId,
    string_split_lines: MethodId,
    string_split_whitespace: MethodId,
    string_char_at: MethodId,
    string_parse_int: MethodId,
    string_parse_float: MethodId,
    string_parse_bool: MethodId,
    bytes_len: MethodId,
    bytes_is_empty: MethodId,
    bytes_slice: MethodId,
    bytes_get: MethodId,
    bytes_read_u32_le: MethodId,
    bytes_read_u32_be: MethodId,
    bytes_to_hex: MethodId,
    array_len: MethodId,
    array_is_empty: MethodId,
    array_push: MethodId,
    array_pop: MethodId,
    array_insert: MethodId,
    array_extend: MethodId,
    array_clear: MethodId,
    array_first: MethodId,
    array_last: MethodId,
    array_remove_at: MethodId,
    array_join: MethodId,
    array_contains: MethodId,
    array_index_of: MethodId,
    array_distinct: MethodId,
    array_reverse: MethodId,
    array_slice: MethodId,
    array_sort: MethodId,
    array_min: MethodId,
    array_max: MethodId,
    map_len: MethodId,
    map_is_empty: MethodId,
    map_has: MethodId,
    map_get: MethodId,
    map_get_or: MethodId,
    map_set: MethodId,
    map_remove: MethodId,
    map_extend: MethodId,
    map_clear: MethodId,
    map_keys: MethodId,
    map_values: MethodId,
    map_entries: MethodId,
    map_merge: MethodId,
    set_len: MethodId,
    set_is_empty: MethodId,
    set_has: MethodId,
    set_add: MethodId,
    set_remove: MethodId,
    set_extend: MethodId,
    set_clear: MethodId,
    set_values: MethodId,
    set_union: MethodId,
    set_intersection: MethodId,
    set_difference: MethodId,
    set_symmetric_difference: MethodId,
    set_is_subset: MethodId,
    set_is_superset: MethodId,
    set_is_disjoint: MethodId,
    option_is_some: MethodId,
    option_is_none: MethodId,
    option_unwrap_or: MethodId,
    option_ok_or: MethodId,
    option_flatten: MethodId,
    result_is_ok: MethodId,
    result_is_err: MethodId,
    result_unwrap_or: MethodId,
    result_to_option: MethodId,
    result_to_error_option: MethodId,
    result_flatten: MethodId,
    range_len: MethodId,
    range_is_empty: MethodId,
}

impl StdMethodIds {
    fn new() -> Self {
        Self {
            string_len: standard_method_id("String", "len"),
            string_is_empty: standard_method_id("String", "is_empty"),
            string_contains: standard_method_id("String", "contains"),
            string_find: standard_method_id("String", "find"),
            string_starts_with: standard_method_id("String", "starts_with"),
            string_ends_with: standard_method_id("String", "ends_with"),
            string_strip_prefix: standard_method_id("String", "strip_prefix"),
            string_strip_suffix: standard_method_id("String", "strip_suffix"),
            string_to_upper: standard_method_id("String", "to_upper"),
            string_to_lower: standard_method_id("String", "to_lower"),
            string_trim: standard_method_id("String", "trim"),
            string_trim_start: standard_method_id("String", "trim_start"),
            string_trim_end: standard_method_id("String", "trim_end"),
            string_replace: standard_method_id("String", "replace"),
            string_repeat: standard_method_id("String", "repeat"),
            string_slice: standard_method_id("String", "slice"),
            string_split: standard_method_id("String", "split"),
            string_split_once: standard_method_id("String", "split_once"),
            string_split_lines: standard_method_id("String", "split_lines"),
            string_split_whitespace: standard_method_id("String", "split_whitespace"),
            string_char_at: standard_method_id("String", "char_at"),
            string_parse_int: standard_method_id("String", "parse_int"),
            string_parse_float: standard_method_id("String", "parse_float"),
            string_parse_bool: standard_method_id("String", "parse_bool"),
            bytes_len: standard_method_id("Bytes", "len"),
            bytes_is_empty: standard_method_id("Bytes", "is_empty"),
            bytes_slice: standard_method_id("Bytes", "slice"),
            bytes_get: standard_method_id("Bytes", "get"),
            bytes_read_u32_le: standard_method_id("Bytes", "read_u32_le"),
            bytes_read_u32_be: standard_method_id("Bytes", "read_u32_be"),
            bytes_to_hex: standard_method_id("Bytes", "to_hex"),
            array_len: standard_method_id("Array", "len"),
            array_is_empty: standard_method_id("Array", "is_empty"),
            array_push: standard_method_id("Array", "push"),
            array_pop: standard_method_id("Array", "pop"),
            array_insert: standard_method_id("Array", "insert"),
            array_extend: standard_method_id("Array", "extend"),
            array_clear: standard_method_id("Array", "clear"),
            array_first: standard_method_id("Array", "first"),
            array_last: standard_method_id("Array", "last"),
            array_remove_at: standard_method_id("Array", "remove_at"),
            array_join: standard_method_id("Array", "join"),
            array_contains: standard_method_id("Array", "contains"),
            array_index_of: standard_method_id("Array", "index_of"),
            array_distinct: standard_method_id("Array", "distinct"),
            array_reverse: standard_method_id("Array", "reverse"),
            array_slice: standard_method_id("Array", "slice"),
            array_sort: standard_method_id("Array", "sort"),
            array_min: standard_method_id("Array", "min"),
            array_max: standard_method_id("Array", "max"),
            map_len: standard_method_id("Map", "len"),
            map_is_empty: standard_method_id("Map", "is_empty"),
            map_has: standard_method_id("Map", "has"),
            map_get: standard_method_id("Map", "get"),
            map_get_or: standard_method_id("Map", "get_or"),
            map_set: standard_method_id("Map", "set"),
            map_remove: standard_method_id("Map", "remove"),
            map_extend: standard_method_id("Map", "extend"),
            map_clear: standard_method_id("Map", "clear"),
            map_keys: standard_method_id("Map", "keys"),
            map_values: standard_method_id("Map", "values"),
            map_entries: standard_method_id("Map", "entries"),
            map_merge: standard_method_id("Map", "merge"),
            set_len: standard_method_id("Set", "len"),
            set_is_empty: standard_method_id("Set", "is_empty"),
            set_has: standard_method_id("Set", "has"),
            set_add: standard_method_id("Set", "add"),
            set_remove: standard_method_id("Set", "remove"),
            set_extend: standard_method_id("Set", "extend"),
            set_clear: standard_method_id("Set", "clear"),
            set_values: standard_method_id("Set", "values"),
            set_union: standard_method_id("Set", "union"),
            set_intersection: standard_method_id("Set", "intersection"),
            set_difference: standard_method_id("Set", "difference"),
            set_symmetric_difference: standard_method_id("Set", "symmetric_difference"),
            set_is_subset: standard_method_id("Set", "is_subset"),
            set_is_superset: standard_method_id("Set", "is_superset"),
            set_is_disjoint: standard_method_id("Set", "is_disjoint"),
            option_is_some: standard_method_id("Option", "is_some"),
            option_is_none: standard_method_id("Option", "is_none"),
            option_unwrap_or: standard_method_id("Option", "unwrap_or"),
            option_ok_or: standard_method_id("Option", "ok_or"),
            option_flatten: standard_method_id("Option", "flatten"),
            result_is_ok: standard_method_id("Result", "is_ok"),
            result_is_err: standard_method_id("Result", "is_err"),
            result_unwrap_or: standard_method_id("Result", "unwrap_or"),
            result_to_option: standard_method_id("Result", "to_option"),
            result_to_error_option: standard_method_id("Result", "to_error_option"),
            result_flatten: standard_method_id("Result", "flatten"),
            range_len: standard_method_id("Range", "len"),
            range_is_empty: standard_method_id("Range", "is_empty"),
        }
    }
}

fn std_method_ids() -> &'static StdMethodIds {
    static IDS: OnceLock<StdMethodIds> = OnceLock::new();
    IDS.get_or_init(StdMethodIds::new)
}

fn standard_method_id(owner: &str, name: &str) -> MethodId {
    let Some(id) = vela_stdlib::std_method_id(owner, name) else {
        panic!("missing standard method identity for {owner}::{name}");
    };
    id
}

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
