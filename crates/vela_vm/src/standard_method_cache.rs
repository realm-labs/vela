use crate::std_method_ids::std_method_ids;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodInlineCacheEntry,
    StandardMethodInlineCacheTarget, StandardMethodReceiver, Value, VmResult, array_methods,
    bytes_methods, map_methods, option_result_methods, script_builtin_methods, set_methods,
};
use vela_def::MethodId;

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
        (StandardMethodReceiver::String, id) if id == ids.string_char_at => {
            StandardMethodInlineCacheTarget::CharAt
        }
        (StandardMethodReceiver::String, id) if id == ids.string_split => {
            StandardMethodInlineCacheTarget::Split
        }
        (StandardMethodReceiver::String, id) if id == ids.string_split_once => {
            StandardMethodInlineCacheTarget::SplitOnce
        }
        (StandardMethodReceiver::String, id) if id == ids.string_split_lines => {
            StandardMethodInlineCacheTarget::SplitLines
        }
        (StandardMethodReceiver::String, id) if id == ids.string_split_whitespace => {
            StandardMethodInlineCacheTarget::SplitWhitespace
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
        (StandardMethodReceiver::String, id) if id == ids.string_slice => {
            StandardMethodInlineCacheTarget::Slice
        }
        (StandardMethodReceiver::String, id) if id == ids.string_repeat => {
            StandardMethodInlineCacheTarget::Repeat
        }
        (StandardMethodReceiver::String, id) if id == ids.string_replace => {
            StandardMethodInlineCacheTarget::Replace
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
        (StandardMethodReceiver::Array, id) if id == ids.array_first => {
            StandardMethodInlineCacheTarget::First
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_last => {
            StandardMethodInlineCacheTarget::Last
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_index_of => {
            StandardMethodInlineCacheTarget::IndexOf
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_slice => {
            StandardMethodInlineCacheTarget::Slice
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_reverse => {
            StandardMethodInlineCacheTarget::Reverse
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_distinct => {
            StandardMethodInlineCacheTarget::Distinct
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_join => {
            StandardMethodInlineCacheTarget::Join
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_sort => {
            StandardMethodInlineCacheTarget::Sort
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_min => {
            StandardMethodInlineCacheTarget::Min
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_max => {
            StandardMethodInlineCacheTarget::Max
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
        (StandardMethodReceiver::Map, id) if id == ids.map_get => {
            StandardMethodInlineCacheTarget::Get
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_get_or => {
            StandardMethodInlineCacheTarget::GetOr
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_keys => {
            StandardMethodInlineCacheTarget::Keys
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_values => {
            StandardMethodInlineCacheTarget::Values
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_entries => {
            StandardMethodInlineCacheTarget::Entries
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_merge => {
            StandardMethodInlineCacheTarget::Merge
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
        (StandardMethodReceiver::Set, id) if id == ids.set_values => {
            StandardMethodInlineCacheTarget::Values
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_union => {
            StandardMethodInlineCacheTarget::Union
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_intersection => {
            StandardMethodInlineCacheTarget::Intersection
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_difference => {
            StandardMethodInlineCacheTarget::Difference
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_symmetric_difference => {
            StandardMethodInlineCacheTarget::SymmetricDifference
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
        (StandardMethodReceiver::Option, id) if id == ids.option_ok_or => {
            StandardMethodInlineCacheTarget::OkOr
        }
        (StandardMethodReceiver::Option, id) if id == ids.option_flatten => {
            StandardMethodInlineCacheTarget::Flatten
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
        (StandardMethodReceiver::Result, id) if id == ids.result_to_option => {
            StandardMethodInlineCacheTarget::ToOption
        }
        (StandardMethodReceiver::Result, id) if id == ids.result_to_error_option => {
            StandardMethodInlineCacheTarget::ToErrorOption
        }
        (StandardMethodReceiver::Result, id) if id == ids.result_flatten => {
            StandardMethodInlineCacheTarget::Flatten
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
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::CharAt) => {
            crate::string_methods::char_at(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Split) => {
            crate::string_methods::split(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::SplitOnce) => {
            crate::string_methods::split_once(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::SplitLines) => {
            crate::string_methods::split_lines(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::SplitWhitespace) => {
            crate::string_methods::split_whitespace(receiver, args, heap, budget)
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
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Slice) => {
            crate::string_methods::slice(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Repeat) => {
            crate::string_methods::repeat(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Replace) => {
            crate::string_methods::replace(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Slice) => {
            bytes_methods::slice(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::ToHex) => {
            bytes_methods::to_hex(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::First) => {
            array_methods::first(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Last) => {
            array_methods::last(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::IndexOf) => {
            array_methods::index_of(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Slice) => {
            array_methods::slice(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Reverse) => {
            array_methods::reverse(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Distinct) => {
            array_methods::distinct(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Join) => {
            array_methods::join(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Sort) => {
            array_methods::sort(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Min) => {
            array_methods::min(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Max) => {
            array_methods::max(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Get) => {
            map_methods::get(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Keys) => {
            map_methods::keys(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Values) => {
            map_methods::values(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Entries) => {
            map_methods::entries(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Merge) => {
            map_methods::merge(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Values) => {
            set_methods::values(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Union) => {
            set_methods::union(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Intersection) => {
            set_methods::intersection(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Difference) => {
            set_methods::difference(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::SymmetricDifference) => {
            set_methods::symmetric_difference(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Option, StandardMethodInlineCacheTarget::OkOr) => {
            option_result_methods::ok_or(receiver, args, heap, budget)
        }
        (
            StandardMethodReceiver::Option | StandardMethodReceiver::Result,
            StandardMethodInlineCacheTarget::Flatten,
        ) => option_result_methods::flatten(receiver, args, heap, budget),
        (StandardMethodReceiver::Result, StandardMethodInlineCacheTarget::ToOption) => {
            option_result_methods::to_option(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Result, StandardMethodInlineCacheTarget::ToErrorOption) => {
            option_result_methods::to_error_option(receiver, args, heap, budget)
        }
        _ => return None,
    };
    Some(result)
}

fn call_readonly_cached(
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
            script_builtin_methods::expect_no_args("len", args)
                .and_then(|()| script_builtin_methods::len(receiver, heap).map(Value::i64))
        }
        (_, StandardMethodInlineCacheTarget::IsEmpty) => {
            script_builtin_methods::expect_no_args("is_empty", args)
                .and_then(|()| script_builtin_methods::is_empty(receiver, heap).map(Value::Bool))
        }
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
        ) => script_builtin_methods::has(receiver, args, heap).map(Value::Bool),
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
