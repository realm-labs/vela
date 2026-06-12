mod materializing_cache;
mod readonly_cache;

use materializing_cache::{
    call_cached_array_lookup_option, call_cached_array_materialization,
    call_cached_bytes_materialization, call_cached_map_get_option, call_cached_string_array,
    call_cached_string_option, call_cached_string_parse_option, call_cached_string_transform,
};
use readonly_cache::{
    call_cached_array_contains, call_cached_bytes_accessor, call_cached_collection_has,
    call_cached_is_empty, call_cached_len, call_cached_map_get_or,
    call_cached_option_result_predicate, call_cached_option_result_unwrap_or,
    call_cached_set_relation, call_cached_string_predicate,
};

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
    let target = standard_method_target(receiver, method_id)?;
    Some(StandardMethodInlineCacheEntry { receiver, target })
}

pub(crate) fn standard_cache_entry_matches_method_id(
    method_id: MethodId,
    cache: StandardMethodInlineCacheEntry,
) -> bool {
    standard_method_target(cache.receiver, method_id) == Some(cache.target)
}

fn standard_method_target(
    receiver: StandardMethodReceiver,
    method_id: MethodId,
) -> Option<StandardMethodInlineCacheTarget> {
    let ids = std_method_ids();
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
        (StandardMethodReceiver::Array, id) if id == ids.array_push => {
            StandardMethodInlineCacheTarget::Push
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_pop => {
            StandardMethodInlineCacheTarget::Pop
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_insert => {
            StandardMethodInlineCacheTarget::Insert
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_remove_at => {
            StandardMethodInlineCacheTarget::RemoveAt
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_clear => {
            StandardMethodInlineCacheTarget::Clear
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_extend => {
            StandardMethodInlineCacheTarget::Extend
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
        (StandardMethodReceiver::Map, id) if id == ids.map_set => {
            StandardMethodInlineCacheTarget::Set
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_remove => {
            StandardMethodInlineCacheTarget::Remove
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_clear => {
            StandardMethodInlineCacheTarget::Clear
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_extend => {
            StandardMethodInlineCacheTarget::Extend
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
        (StandardMethodReceiver::Set, id) if id == ids.set_add => {
            StandardMethodInlineCacheTarget::Add
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_remove => {
            StandardMethodInlineCacheTarget::Remove
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_clear => {
            StandardMethodInlineCacheTarget::Clear
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_extend => {
            StandardMethodInlineCacheTarget::Extend
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
    Some(target)
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
    match cache.target {
        StandardMethodInlineCacheTarget::First
        | StandardMethodInlineCacheTarget::Last
        | StandardMethodInlineCacheTarget::IndexOf
            if cache.receiver == StandardMethodReceiver::Array =>
        {
            return call_cached_array_lookup_option(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Get if cache.receiver == StandardMethodReceiver::Map => {
            return call_cached_map_get_option(receiver, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::ParseInt
        | StandardMethodInlineCacheTarget::ParseFloat
        | StandardMethodInlineCacheTarget::ParseBool
            if cache.receiver == StandardMethodReceiver::String =>
        {
            return call_cached_string_parse_option(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Find
        | StandardMethodInlineCacheTarget::CharAt
        | StandardMethodInlineCacheTarget::SplitOnce
        | StandardMethodInlineCacheTarget::StripPrefix
        | StandardMethodInlineCacheTarget::StripSuffix
            if cache.receiver == StandardMethodReceiver::String =>
        {
            return call_cached_string_option(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Split
        | StandardMethodInlineCacheTarget::SplitLines
        | StandardMethodInlineCacheTarget::SplitWhitespace
            if cache.receiver == StandardMethodReceiver::String =>
        {
            return call_cached_string_array(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::ToUpper
        | StandardMethodInlineCacheTarget::ToLower
        | StandardMethodInlineCacheTarget::Trim
        | StandardMethodInlineCacheTarget::TrimStart
        | StandardMethodInlineCacheTarget::TrimEnd
        | StandardMethodInlineCacheTarget::Repeat
        | StandardMethodInlineCacheTarget::Replace
        | StandardMethodInlineCacheTarget::Slice
            if cache.receiver == StandardMethodReceiver::String =>
        {
            return call_cached_string_transform(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Slice | StandardMethodInlineCacheTarget::ToHex
            if cache.receiver == StandardMethodReceiver::Bytes =>
        {
            return call_cached_bytes_materialization(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Slice
        | StandardMethodInlineCacheTarget::Reverse
        | StandardMethodInlineCacheTarget::Distinct
            if cache.receiver == StandardMethodReceiver::Array =>
        {
            return call_cached_array_materialization(receiver, cache.target, args, heap, budget);
        }
        _ => {}
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
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Push) => {
            let mut receiver = *receiver;
            array_methods::push(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Pop) => {
            let mut receiver = *receiver;
            array_methods::pop(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Insert) => {
            let mut receiver = *receiver;
            array_methods::insert(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::RemoveAt) => {
            let mut receiver = *receiver;
            array_methods::remove_at(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Clear) => {
            let mut receiver = *receiver;
            array_methods::clear(&mut receiver, args, heap.as_deref_mut())
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Extend) => {
            let mut receiver = *receiver;
            array_methods::extend(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
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
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Set) => {
            let mut receiver = *receiver;
            map_methods::set(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Remove) => {
            let mut receiver = *receiver;
            map_methods::remove(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Clear) => {
            let mut receiver = *receiver;
            map_methods::clear(&mut receiver, args, heap.as_deref_mut())
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Extend) => {
            let mut receiver = *receiver;
            map_methods::extend(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
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
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Add) => {
            let mut receiver = *receiver;
            set_methods::add(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Remove) => {
            let mut receiver = *receiver;
            set_methods::remove(&mut receiver, args, heap.as_deref_mut())
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Clear) => {
            let mut receiver = *receiver;
            set_methods::clear(&mut receiver, args, heap.as_deref_mut())
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Extend) => {
            let mut receiver = *receiver;
            set_methods::extend(
                &mut receiver,
                args,
                heap.as_deref_mut(),
                budget.as_deref_mut(),
            )
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
    match cache.target {
        StandardMethodInlineCacheTarget::Len => {
            return call_cached_len(receiver, cache.receiver, args, heap);
        }
        StandardMethodInlineCacheTarget::IsEmpty => {
            return call_cached_is_empty(receiver, cache.receiver, args, heap);
        }
        StandardMethodInlineCacheTarget::IsSome
        | StandardMethodInlineCacheTarget::IsNone
        | StandardMethodInlineCacheTarget::IsOk
        | StandardMethodInlineCacheTarget::IsErr => {
            return call_cached_option_result_predicate(
                receiver,
                cache.receiver,
                cache.target,
                args,
                heap,
            );
        }
        StandardMethodInlineCacheTarget::UnwrapOr => {
            return call_cached_option_result_unwrap_or(receiver, cache.receiver, args, heap);
        }
        StandardMethodInlineCacheTarget::GetOr => {
            return call_cached_map_get_or(receiver, cache.receiver, args, heap);
        }
        StandardMethodInlineCacheTarget::Has => {
            return call_cached_collection_has(receiver, cache.receiver, args, heap);
        }
        StandardMethodInlineCacheTarget::IsSubset
        | StandardMethodInlineCacheTarget::IsSuperset
        | StandardMethodInlineCacheTarget::IsDisjoint => {
            return call_cached_set_relation(receiver, cache.receiver, cache.target, args, heap);
        }
        StandardMethodInlineCacheTarget::Contains
        | StandardMethodInlineCacheTarget::StartsWith
        | StandardMethodInlineCacheTarget::EndsWith
            if cache.receiver == StandardMethodReceiver::String =>
        {
            return call_cached_string_predicate(receiver, cache.target, args, heap);
        }
        StandardMethodInlineCacheTarget::Contains
            if cache.receiver == StandardMethodReceiver::Array =>
        {
            return call_cached_array_contains(receiver, args, heap);
        }
        StandardMethodInlineCacheTarget::Get
        | StandardMethodInlineCacheTarget::ReadU32Le
        | StandardMethodInlineCacheTarget::ReadU32Be
            if cache.receiver == StandardMethodReceiver::Bytes =>
        {
            return call_cached_bytes_accessor(receiver, cache.target, args, heap);
        }
        _ => {}
    }
    if !receiver_matches_cache(receiver, cache.receiver, heap) {
        return None;
    }
    let result = match (cache.receiver, cache.target) {
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
