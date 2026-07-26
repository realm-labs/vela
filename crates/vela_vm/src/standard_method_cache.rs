mod materializing_cache;
mod readonly_cache;
mod string_parse_cache;

use materializing_cache::{
    call_cached_array_lookup_option, call_cached_array_materialization, call_cached_array_mutation,
    call_cached_bytes_materialization, call_cached_map_get_option, call_cached_map_materialization,
    call_cached_map_mutation, call_cached_option_result_materialization,
    call_cached_set_materialization, call_cached_set_mutation, call_cached_string_array,
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

/// Classifies a receiver into its standard family with one heap access.
///
/// The former predicate chain asked each family in turn and every heap-backed
/// predicate dereferenced the receiver again, so a map receiver paid several
/// heap lookups per call before its method could even resolve.
fn classify_standard_receiver(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<StandardMethodReceiver> {
    match receiver {
        Value::Char(_) => Some(StandardMethodReceiver::Char),
        Value::HeapRef(reference) => match heap?.heap.get(*reference)? {
            crate::heap::HeapValue::Range(_) => Some(StandardMethodReceiver::Range),
            crate::heap::HeapValue::String(_) => Some(StandardMethodReceiver::String),
            crate::heap::HeapValue::Bytes(_) => Some(StandardMethodReceiver::Bytes),
            crate::heap::HeapValue::Array(_) => Some(StandardMethodReceiver::Array),
            crate::heap::HeapValue::Map(_) => Some(StandardMethodReceiver::Map),
            crate::heap::HeapValue::Set(_) => Some(StandardMethodReceiver::Set),
            crate::heap::HeapValue::Iterator(_) => Some(StandardMethodReceiver::Iterator),
            crate::heap::HeapValue::Enum { .. } => {
                if option_result_methods::is_option(receiver, heap) {
                    Some(StandardMethodReceiver::Option)
                } else if option_result_methods::is_result(receiver, heap) {
                    Some(StandardMethodReceiver::Result)
                } else {
                    None
                }
            }
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn standard_cache_entry(
    method_id: MethodId,
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<StandardMethodInlineCacheEntry> {
    let receiver = classify_standard_receiver(receiver, heap)?;
    let target = standard_method_target(receiver, method_id)?;
    Some(StandardMethodInlineCacheEntry { receiver, target })
}

pub(crate) fn standard_cache_entry_matches_method_id(
    method_id: MethodId,
    cache: StandardMethodInlineCacheEntry,
) -> bool {
    match (cache.receiver, cache.target) {
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Len) => {
            return method_id == std_method_ids().string_len;
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::ToUpper) => {
            return method_id == std_method_ids().string_to_upper;
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::ToLower) => {
            return method_id == std_method_ids().string_to_lower;
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Chars) => {
            return method_id == std_method_ids().string_chars;
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Bytes) => {
            return method_id == std_method_ids().string_bytes;
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::Trim) => {
            return method_id == std_method_ids().string_trim;
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::TrimStart) => {
            return method_id == std_method_ids().string_trim_start;
        }
        (StandardMethodReceiver::String, StandardMethodInlineCacheTarget::TrimEnd) => {
            return method_id == std_method_ids().string_trim_end;
        }
        (StandardMethodReceiver::String, target)
            if string_parse_cache::target_matches_method_id(target, method_id) =>
        {
            return true;
        }
        (StandardMethodReceiver::Range, StandardMethodInlineCacheTarget::Len) => {
            return method_id == std_method_ids().range_len;
        }
        (StandardMethodReceiver::Range, StandardMethodInlineCacheTarget::IsEmpty) => {
            return method_id == std_method_ids().range_is_empty;
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Len) => {
            return method_id == std_method_ids().bytes_len;
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::IsEmpty) => {
            return method_id == std_method_ids().bytes_is_empty;
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Get) => {
            return method_id == std_method_ids().bytes_get;
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::ReadU32Le) => {
            return method_id == std_method_ids().bytes_read_u32_le;
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::ReadU32Be) => {
            return method_id == std_method_ids().bytes_read_u32_be;
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Slice) => {
            return method_id == std_method_ids().bytes_slice;
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::ToHex) => {
            return method_id == std_method_ids().bytes_to_hex;
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Iter) => {
            return method_id == std_method_ids().bytes_iter;
        }
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Values) => {
            return method_id == std_method_ids().bytes_values;
        }
        (StandardMethodReceiver::Char, StandardMethodInlineCacheTarget::ToString) => {
            return method_id == std_method_ids().char_to_string;
        }
        (StandardMethodReceiver::Char, StandardMethodInlineCacheTarget::IsWhitespace) => {
            return method_id == std_method_ids().char_is_whitespace;
        }
        (StandardMethodReceiver::Char, StandardMethodInlineCacheTarget::IsAscii) => {
            return method_id == std_method_ids().char_is_ascii;
        }
        (StandardMethodReceiver::Char, StandardMethodInlineCacheTarget::IsAsciiDigit) => {
            return method_id == std_method_ids().char_is_ascii_digit;
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Len) => {
            return method_id == std_method_ids().array_len;
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Contains) => {
            return method_id == std_method_ids().array_contains;
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::IndexOf) => {
            return method_id == std_method_ids().array_index_of;
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Join) => {
            return method_id == std_method_ids().array_join;
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Sort) => {
            return method_id == std_method_ids().array_sort;
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Sum) => {
            return method_id == std_method_ids().array_sum;
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Iter) => {
            return method_id == std_method_ids().array_iter;
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Values) => {
            return method_id == std_method_ids().array_values;
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Keys) => {
            return method_id == std_method_ids().map_keys;
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Values) => {
            return method_id == std_method_ids().map_values;
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Entries) => {
            return method_id == std_method_ids().map_entries;
        }
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::Iter) => {
            return method_id == std_method_ids().map_iter;
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Values) => {
            return method_id == std_method_ids().set_values;
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Iter) => {
            return method_id == std_method_ids().set_iter;
        }
        (StandardMethodReceiver::Range, StandardMethodInlineCacheTarget::Iter) => {
            return method_id == std_method_ids().range_iter;
        }
        (StandardMethodReceiver::Iterator, StandardMethodInlineCacheTarget::Take) => {
            return method_id == std_method_ids().iterator_take;
        }
        (StandardMethodReceiver::Iterator, StandardMethodInlineCacheTarget::Skip) => {
            return method_id == std_method_ids().iterator_skip;
        }
        _ => {}
    }
    standard_method_target(cache.receiver, method_id) == Some(cache.target)
}

/// Resolves one standard method by receiver family first.
///
/// The former single guard chain compared `method_id` against every family's
/// identifiers in declaration order, so a map or set method paid dozens of
/// unrelated `MethodId` comparisons per call. Dispatching on the receiver
/// keeps each lookup inside its own family.
fn standard_method_target(
    receiver: StandardMethodReceiver,
    method_id: MethodId,
) -> Option<StandardMethodInlineCacheTarget> {
    let ids = std_method_ids();
    match receiver {
        StandardMethodReceiver::String => string_family_target(method_id, ids),
        StandardMethodReceiver::Bytes => bytes_family_target(method_id, ids),
        StandardMethodReceiver::Char => char_family_target(method_id, ids),
        StandardMethodReceiver::Range => range_family_target(method_id, ids),
        StandardMethodReceiver::Array => array_family_target(method_id, ids),
        StandardMethodReceiver::Map => map_family_target(method_id, ids),
        StandardMethodReceiver::Set => set_family_target(method_id, ids),
        StandardMethodReceiver::Iterator => iterator_family_target(method_id, ids),
        StandardMethodReceiver::Option => option_family_target(method_id, ids),
        StandardMethodReceiver::Result => result_family_target(method_id, ids),
    }
}

fn string_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if let Some(target) = string_parse_cache::target_for_method_id(method_id, ids) {
        return Some(target);
    }
    if method_id == ids.string_len {
        Some(StandardMethodInlineCacheTarget::Len)
    } else if method_id == ids.string_is_empty {
        Some(StandardMethodInlineCacheTarget::IsEmpty)
    } else if method_id == ids.string_contains {
        Some(StandardMethodInlineCacheTarget::Contains)
    } else if method_id == ids.string_starts_with {
        Some(StandardMethodInlineCacheTarget::StartsWith)
    } else if method_id == ids.string_ends_with {
        Some(StandardMethodInlineCacheTarget::EndsWith)
    } else if method_id == ids.string_find {
        Some(StandardMethodInlineCacheTarget::Find)
    } else if method_id == ids.string_strip_prefix {
        Some(StandardMethodInlineCacheTarget::StripPrefix)
    } else if method_id == ids.string_strip_suffix {
        Some(StandardMethodInlineCacheTarget::StripSuffix)
    } else if method_id == ids.string_split {
        Some(StandardMethodInlineCacheTarget::Split)
    } else if method_id == ids.string_split_once {
        Some(StandardMethodInlineCacheTarget::SplitOnce)
    } else if method_id == ids.string_split_lines {
        Some(StandardMethodInlineCacheTarget::SplitLines)
    } else if method_id == ids.string_split_whitespace {
        Some(StandardMethodInlineCacheTarget::SplitWhitespace)
    } else if method_id == ids.string_chars {
        Some(StandardMethodInlineCacheTarget::Chars)
    } else if method_id == ids.string_bytes {
        Some(StandardMethodInlineCacheTarget::Bytes)
    } else if method_id == ids.string_to_upper {
        Some(StandardMethodInlineCacheTarget::ToUpper)
    } else if method_id == ids.string_to_lower {
        Some(StandardMethodInlineCacheTarget::ToLower)
    } else if method_id == ids.string_trim {
        Some(StandardMethodInlineCacheTarget::Trim)
    } else if method_id == ids.string_trim_start {
        Some(StandardMethodInlineCacheTarget::TrimStart)
    } else if method_id == ids.string_trim_end {
        Some(StandardMethodInlineCacheTarget::TrimEnd)
    } else if method_id == ids.string_slice {
        Some(StandardMethodInlineCacheTarget::Slice)
    } else if method_id == ids.string_repeat {
        Some(StandardMethodInlineCacheTarget::Repeat)
    } else if method_id == ids.string_replace {
        Some(StandardMethodInlineCacheTarget::Replace)
    } else {
        None
    }
}

fn bytes_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if method_id == ids.bytes_len {
        Some(StandardMethodInlineCacheTarget::Len)
    } else if method_id == ids.bytes_is_empty {
        Some(StandardMethodInlineCacheTarget::IsEmpty)
    } else if method_id == ids.bytes_get {
        Some(StandardMethodInlineCacheTarget::Get)
    } else if method_id == ids.bytes_slice {
        Some(StandardMethodInlineCacheTarget::Slice)
    } else if method_id == ids.bytes_to_hex {
        Some(StandardMethodInlineCacheTarget::ToHex)
    } else if method_id == ids.bytes_read_u32_le {
        Some(StandardMethodInlineCacheTarget::ReadU32Le)
    } else if method_id == ids.bytes_read_u32_be {
        Some(StandardMethodInlineCacheTarget::ReadU32Be)
    } else if method_id == ids.bytes_iter {
        Some(StandardMethodInlineCacheTarget::Iter)
    } else if method_id == ids.bytes_values {
        Some(StandardMethodInlineCacheTarget::Values)
    } else {
        None
    }
}

fn char_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if method_id == ids.char_to_string {
        Some(StandardMethodInlineCacheTarget::ToString)
    } else if method_id == ids.char_is_whitespace {
        Some(StandardMethodInlineCacheTarget::IsWhitespace)
    } else if method_id == ids.char_is_ascii {
        Some(StandardMethodInlineCacheTarget::IsAscii)
    } else if method_id == ids.char_is_ascii_digit {
        Some(StandardMethodInlineCacheTarget::IsAsciiDigit)
    } else {
        None
    }
}

fn range_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if method_id == ids.range_len {
        Some(StandardMethodInlineCacheTarget::Len)
    } else if method_id == ids.range_is_empty {
        Some(StandardMethodInlineCacheTarget::IsEmpty)
    } else if method_id == ids.range_iter {
        Some(StandardMethodInlineCacheTarget::Iter)
    } else {
        None
    }
}

fn array_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if method_id == ids.array_len {
        Some(StandardMethodInlineCacheTarget::Len)
    } else if method_id == ids.array_is_empty {
        Some(StandardMethodInlineCacheTarget::IsEmpty)
    } else if method_id == ids.array_get {
        Some(StandardMethodInlineCacheTarget::Get)
    } else if method_id == ids.array_contains {
        Some(StandardMethodInlineCacheTarget::Contains)
    } else if method_id == ids.array_first {
        Some(StandardMethodInlineCacheTarget::First)
    } else if method_id == ids.array_last {
        Some(StandardMethodInlineCacheTarget::Last)
    } else if method_id == ids.array_index_of {
        Some(StandardMethodInlineCacheTarget::IndexOf)
    } else if method_id == ids.array_slice {
        Some(StandardMethodInlineCacheTarget::Slice)
    } else if method_id == ids.array_push {
        Some(StandardMethodInlineCacheTarget::Push)
    } else if method_id == ids.array_pop {
        Some(StandardMethodInlineCacheTarget::Pop)
    } else if method_id == ids.array_insert {
        Some(StandardMethodInlineCacheTarget::Insert)
    } else if method_id == ids.array_remove_at {
        Some(StandardMethodInlineCacheTarget::RemoveAt)
    } else if method_id == ids.array_clear {
        Some(StandardMethodInlineCacheTarget::Clear)
    } else if method_id == ids.array_extend {
        Some(StandardMethodInlineCacheTarget::Extend)
    } else if method_id == ids.array_reverse {
        Some(StandardMethodInlineCacheTarget::Reverse)
    } else if method_id == ids.array_distinct {
        Some(StandardMethodInlineCacheTarget::Distinct)
    } else if method_id == ids.array_join {
        Some(StandardMethodInlineCacheTarget::Join)
    } else if method_id == ids.array_sort {
        Some(StandardMethodInlineCacheTarget::Sort)
    } else if method_id == ids.array_min {
        Some(StandardMethodInlineCacheTarget::Min)
    } else if method_id == ids.array_max {
        Some(StandardMethodInlineCacheTarget::Max)
    } else if method_id == ids.array_sum {
        Some(StandardMethodInlineCacheTarget::Sum)
    } else if method_id == ids.array_iter {
        Some(StandardMethodInlineCacheTarget::Iter)
    } else if method_id == ids.array_values {
        Some(StandardMethodInlineCacheTarget::Values)
    } else {
        None
    }
}

fn map_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if method_id == ids.map_len {
        Some(StandardMethodInlineCacheTarget::Len)
    } else if method_id == ids.map_is_empty {
        Some(StandardMethodInlineCacheTarget::IsEmpty)
    } else if method_id == ids.map_has {
        Some(StandardMethodInlineCacheTarget::Has)
    } else if method_id == ids.map_contains_key {
        Some(StandardMethodInlineCacheTarget::ContainsKey)
    } else if method_id == ids.map_get {
        Some(StandardMethodInlineCacheTarget::Get)
    } else if method_id == ids.map_get_or {
        Some(StandardMethodInlineCacheTarget::GetOr)
    } else if method_id == ids.map_get_or_insert {
        Some(StandardMethodInlineCacheTarget::GetOrInsert)
    } else if method_id == ids.map_set {
        Some(StandardMethodInlineCacheTarget::Set)
    } else if method_id == ids.map_insert {
        Some(StandardMethodInlineCacheTarget::Insert)
    } else if method_id == ids.map_remove {
        Some(StandardMethodInlineCacheTarget::Remove)
    } else if method_id == ids.map_clear {
        Some(StandardMethodInlineCacheTarget::Clear)
    } else if method_id == ids.map_extend {
        Some(StandardMethodInlineCacheTarget::Extend)
    } else if method_id == ids.map_keys {
        Some(StandardMethodInlineCacheTarget::Keys)
    } else if method_id == ids.map_values {
        Some(StandardMethodInlineCacheTarget::Values)
    } else if method_id == ids.map_entries {
        Some(StandardMethodInlineCacheTarget::Entries)
    } else if method_id == ids.map_merge {
        Some(StandardMethodInlineCacheTarget::Merge)
    } else if method_id == ids.map_iter {
        Some(StandardMethodInlineCacheTarget::Iter)
    } else {
        None
    }
}

fn set_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if method_id == ids.set_len {
        Some(StandardMethodInlineCacheTarget::Len)
    } else if method_id == ids.set_is_empty {
        Some(StandardMethodInlineCacheTarget::IsEmpty)
    } else if method_id == ids.set_has {
        Some(StandardMethodInlineCacheTarget::Has)
    } else if method_id == ids.set_contains {
        Some(StandardMethodInlineCacheTarget::Contains)
    } else if method_id == ids.set_add {
        Some(StandardMethodInlineCacheTarget::Add)
    } else if method_id == ids.set_insert {
        Some(StandardMethodInlineCacheTarget::Insert)
    } else if method_id == ids.set_remove {
        Some(StandardMethodInlineCacheTarget::Remove)
    } else if method_id == ids.set_clear {
        Some(StandardMethodInlineCacheTarget::Clear)
    } else if method_id == ids.set_extend {
        Some(StandardMethodInlineCacheTarget::Extend)
    } else if method_id == ids.set_values {
        Some(StandardMethodInlineCacheTarget::Values)
    } else if method_id == ids.set_union {
        Some(StandardMethodInlineCacheTarget::Union)
    } else if method_id == ids.set_intersection {
        Some(StandardMethodInlineCacheTarget::Intersection)
    } else if method_id == ids.set_difference {
        Some(StandardMethodInlineCacheTarget::Difference)
    } else if method_id == ids.set_symmetric_difference {
        Some(StandardMethodInlineCacheTarget::SymmetricDifference)
    } else if method_id == ids.set_is_subset {
        Some(StandardMethodInlineCacheTarget::IsSubset)
    } else if method_id == ids.set_is_superset {
        Some(StandardMethodInlineCacheTarget::IsSuperset)
    } else if method_id == ids.set_is_disjoint {
        Some(StandardMethodInlineCacheTarget::IsDisjoint)
    } else if method_id == ids.set_iter {
        Some(StandardMethodInlineCacheTarget::Iter)
    } else {
        None
    }
}

fn iterator_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if method_id == ids.iterator_take {
        Some(StandardMethodInlineCacheTarget::Take)
    } else if method_id == ids.iterator_skip {
        Some(StandardMethodInlineCacheTarget::Skip)
    } else {
        None
    }
}

fn option_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if method_id == ids.option_is_some {
        Some(StandardMethodInlineCacheTarget::IsSome)
    } else if method_id == ids.option_is_none {
        Some(StandardMethodInlineCacheTarget::IsNone)
    } else if method_id == ids.option_unwrap_or {
        Some(StandardMethodInlineCacheTarget::UnwrapOr)
    } else if method_id == ids.option_ok_or {
        Some(StandardMethodInlineCacheTarget::OkOr)
    } else if method_id == ids.option_flatten {
        Some(StandardMethodInlineCacheTarget::Flatten)
    } else {
        None
    }
}

fn result_family_target(
    method_id: MethodId,
    ids: &crate::std_method_ids::StdMethodIds,
) -> Option<StandardMethodInlineCacheTarget> {
    if method_id == ids.result_is_ok {
        Some(StandardMethodInlineCacheTarget::IsOk)
    } else if method_id == ids.result_is_err {
        Some(StandardMethodInlineCacheTarget::IsErr)
    } else if method_id == ids.result_unwrap_or {
        Some(StandardMethodInlineCacheTarget::UnwrapOr)
    } else if method_id == ids.result_to_option {
        Some(StandardMethodInlineCacheTarget::ToOption)
    } else if method_id == ids.result_to_error_option {
        Some(StandardMethodInlineCacheTarget::ToErrorOption)
    } else if method_id == ids.result_flatten {
        Some(StandardMethodInlineCacheTarget::Flatten)
    } else {
        None
    }
}

pub(crate) fn call_standard_cached(
    receiver: &Value,
    cache: StandardMethodInlineCacheEntry,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    if array_target_requires_method_runtime(cache) {
        return None;
    }
    if cache.receiver == StandardMethodReceiver::Range
        && matches!(
            cache.target,
            StandardMethodInlineCacheTarget::Len | StandardMethodInlineCacheTarget::IsEmpty
        )
    {
        return call_readonly_cached(receiver, cache, args, heap.as_deref());
    }
    match cache.target {
        StandardMethodInlineCacheTarget::First
        | StandardMethodInlineCacheTarget::Get
        | StandardMethodInlineCacheTarget::Last
            if cache.receiver == StandardMethodReceiver::Array =>
        {
            return call_cached_array_lookup_option(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Get if cache.receiver == StandardMethodReceiver::Map => {
            return call_cached_map_get_option(receiver, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Merge if cache.receiver == StandardMethodReceiver::Map => {
            return call_cached_map_materialization(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::GetOrInsert
        | StandardMethodInlineCacheTarget::Set
        | StandardMethodInlineCacheTarget::Insert
        | StandardMethodInlineCacheTarget::Remove
        | StandardMethodInlineCacheTarget::Clear
        | StandardMethodInlineCacheTarget::Extend
            if cache.receiver == StandardMethodReceiver::Map =>
        {
            return call_cached_map_mutation(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Union
        | StandardMethodInlineCacheTarget::Intersection
        | StandardMethodInlineCacheTarget::Difference
        | StandardMethodInlineCacheTarget::SymmetricDifference
            if cache.receiver == StandardMethodReceiver::Set =>
        {
            return call_cached_set_materialization(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Add
        | StandardMethodInlineCacheTarget::Insert
        | StandardMethodInlineCacheTarget::Remove
        | StandardMethodInlineCacheTarget::Clear
        | StandardMethodInlineCacheTarget::Extend
            if cache.receiver == StandardMethodReceiver::Set =>
        {
            return call_cached_set_mutation(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::OkOr
        | StandardMethodInlineCacheTarget::ToOption
        | StandardMethodInlineCacheTarget::ToErrorOption
        | StandardMethodInlineCacheTarget::Flatten
            if matches!(
                cache.receiver,
                StandardMethodReceiver::Option | StandardMethodReceiver::Result
            ) =>
        {
            return call_cached_option_result_materialization(
                receiver,
                cache.receiver,
                cache.target,
                args,
                heap,
                budget,
            );
        }
        target
            if cache.receiver == StandardMethodReceiver::String
                && string_parse_cache::is_parse_target(target) =>
        {
            return call_cached_string_parse_option(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Find
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
        StandardMethodInlineCacheTarget::Iter | StandardMethodInlineCacheTarget::Values
            if cache.receiver == StandardMethodReceiver::Bytes =>
        {
            return Some(crate::iteration::iter_method(receiver, args, heap, budget));
        }
        StandardMethodInlineCacheTarget::ToString
            if cache.receiver == StandardMethodReceiver::Char =>
        {
            return Some(crate::char_methods::to_string(receiver, args, heap, budget));
        }
        StandardMethodInlineCacheTarget::Values
            if cache.receiver == StandardMethodReceiver::Array =>
        {
            return Some(crate::iteration::iter_method(receiver, args, heap, budget));
        }
        StandardMethodInlineCacheTarget::Iter
            if matches!(
                cache.receiver,
                StandardMethodReceiver::Array
                    | StandardMethodReceiver::Map
                    | StandardMethodReceiver::Set
                    | StandardMethodReceiver::Range
            ) =>
        {
            return Some(crate::iteration::iter_method(receiver, args, heap, budget));
        }
        StandardMethodInlineCacheTarget::Chars
            if cache.receiver == StandardMethodReceiver::String =>
        {
            return Some(crate::iteration::chars_method(receiver, args, heap, budget));
        }
        StandardMethodInlineCacheTarget::Bytes
            if cache.receiver == StandardMethodReceiver::String =>
        {
            return Some(crate::iteration::string_bytes_method(
                receiver, args, heap, budget,
            ));
        }
        StandardMethodInlineCacheTarget::Take | StandardMethodInlineCacheTarget::Skip
            if cache.receiver == StandardMethodReceiver::Iterator =>
        {
            return match cache.target {
                StandardMethodInlineCacheTarget::Take => {
                    Some(crate::iteration::take_method(receiver, args, heap, budget))
                }
                StandardMethodInlineCacheTarget::Skip => {
                    Some(crate::iteration::skip_method(receiver, args, heap, budget))
                }
                _ => None,
            };
        }
        StandardMethodInlineCacheTarget::Slice
        | StandardMethodInlineCacheTarget::Reverse
        | StandardMethodInlineCacheTarget::Join
        | StandardMethodInlineCacheTarget::Sum
            if cache.receiver == StandardMethodReceiver::Array =>
        {
            if cache.target == StandardMethodInlineCacheTarget::Sum {
                return call_cached_array_sum(receiver, args, heap.as_deref());
            }
            return call_cached_array_materialization(receiver, cache.target, args, heap, budget);
        }
        StandardMethodInlineCacheTarget::Push
        | StandardMethodInlineCacheTarget::Pop
        | StandardMethodInlineCacheTarget::Insert
        | StandardMethodInlineCacheTarget::RemoveAt
        | StandardMethodInlineCacheTarget::Clear
        | StandardMethodInlineCacheTarget::Extend
            if cache.receiver == StandardMethodReceiver::Array =>
        {
            return call_cached_array_mutation(receiver, cache.target, args, heap, budget);
        }
        _ => {}
    }
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
        (StandardMethodReceiver::String, target) if string_parse_cache::is_parse_target(target) => {
            string_parse_cache::call_parse_method(target, receiver, args, heap, budget)
                .expect("parse target should be handled")
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
        (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Iter)
        | (StandardMethodReceiver::Bytes, StandardMethodInlineCacheTarget::Values) => {
            crate::iteration::iter_method(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Char, StandardMethodInlineCacheTarget::ToString) => {
            crate::char_methods::to_string(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::First) => {
            array_methods::first(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Get) => {
            array_methods::get(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Last) => {
            array_methods::last(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Contains) => {
            array_methods::contains_by_key(receiver, args, heap.as_deref()).map(Value::Bool)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::IndexOf) => {
            array_methods::index_of_by_key(receiver, args, heap, budget)
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
            array_methods::distinct_by_key(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Join) => {
            array_methods::join(receiver, args, heap, budget)
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Sum) => {
            array_methods::sum_values(receiver, heap.as_deref(), "method sum")
        }
        (StandardMethodReceiver::Array, StandardMethodInlineCacheTarget::Values) => {
            crate::iteration::iter_method(receiver, args, heap, budget)
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
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Insert) => {
            let mut receiver = *receiver;
            set_methods::insert(
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

fn array_target_requires_method_runtime(cache: StandardMethodInlineCacheEntry) -> bool {
    cache.receiver == StandardMethodReceiver::Array
        && matches!(
            cache.target,
            StandardMethodInlineCacheTarget::Sort
                | StandardMethodInlineCacheTarget::Min
                | StandardMethodInlineCacheTarget::Max
        )
}

fn call_cached_array_sum(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    if !args.is_empty() || !array_methods::is_array(receiver, heap) {
        return None;
    }
    Some(array_methods::sum_values(receiver, heap, "method sum"))
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
        StandardMethodInlineCacheTarget::ContainsKey
            if cache.receiver == StandardMethodReceiver::Map =>
        {
            return Some(map_methods::contains_key(receiver, args, heap).map(Value::Bool));
        }
        StandardMethodInlineCacheTarget::Contains
            if cache.receiver == StandardMethodReceiver::Array =>
        {
            return call_cached_array_contains(receiver, args, heap);
        }
        StandardMethodInlineCacheTarget::Contains
            if cache.receiver == StandardMethodReceiver::Set =>
        {
            return Some(set_methods::contains(receiver, args, heap).map(Value::Bool));
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
        StandardMethodInlineCacheTarget::Get
        | StandardMethodInlineCacheTarget::ReadU32Le
        | StandardMethodInlineCacheTarget::ReadU32Be
            if cache.receiver == StandardMethodReceiver::Bytes =>
        {
            return call_cached_bytes_accessor(receiver, cache.target, args, heap);
        }
        StandardMethodInlineCacheTarget::IsWhitespace
        | StandardMethodInlineCacheTarget::IsAscii
        | StandardMethodInlineCacheTarget::IsAsciiDigit
            if cache.receiver == StandardMethodReceiver::Char =>
        {
            return Some(match cache.target {
                StandardMethodInlineCacheTarget::IsWhitespace => {
                    crate::char_methods::is_whitespace(receiver, args)
                }
                StandardMethodInlineCacheTarget::IsAscii => {
                    crate::char_methods::is_ascii(receiver, args)
                }
                StandardMethodInlineCacheTarget::IsAsciiDigit => {
                    crate::char_methods::is_ascii_digit(receiver, args)
                }
                _ => unreachable!("char readonly target was validated above"),
            });
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
        (StandardMethodReceiver::Map, StandardMethodInlineCacheTarget::ContainsKey) => {
            map_methods::contains_key(receiver, args, heap).map(Value::Bool)
        }
        (StandardMethodReceiver::Set, StandardMethodInlineCacheTarget::Contains) => {
            set_methods::contains(receiver, args, heap).map(Value::Bool)
        }
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
        (StandardMethodReceiver::Char, StandardMethodInlineCacheTarget::IsWhitespace) => {
            crate::char_methods::is_whitespace(receiver, args)
        }
        (StandardMethodReceiver::Char, StandardMethodInlineCacheTarget::IsAscii) => {
            crate::char_methods::is_ascii(receiver, args)
        }
        (StandardMethodReceiver::Char, StandardMethodInlineCacheTarget::IsAsciiDigit) => {
            crate::char_methods::is_ascii_digit(receiver, args)
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
        StandardMethodReceiver::Char => crate::char_methods::is_char(receiver),
        StandardMethodReceiver::Range => crate::ranges::is_range(receiver, heap),
        StandardMethodReceiver::Array => array_methods::is_array(receiver, heap),
        StandardMethodReceiver::Map => map_methods::is_map(receiver, heap),
        StandardMethodReceiver::Set => set_methods::is_set(receiver, heap),
        StandardMethodReceiver::Iterator => crate::iteration::is_iterator(receiver, heap),
        StandardMethodReceiver::Option => option_result_methods::is_option(receiver, heap),
        StandardMethodReceiver::Result => option_result_methods::is_result(receiver, heap),
    }
}
