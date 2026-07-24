mod option_result_inactive;

use std::sync::OnceLock;

use vela_bytecode::LinkedProgram;
use vela_def::MethodId;

use crate::method_runtime::MethodRuntime;
use crate::{
    CallbackMethodInlineCacheEntry, CallbackMethodInlineCacheTarget, ExecutionBudget,
    HeapExecution, StandardMethodReceiver, Value, VmResult, array_methods, iteration, map_methods,
    option_result_methods, set_methods,
};
use vela_host::protocol::HostCollectionProjection;

pub(crate) struct CallbackMethodDispatch<'a, 'heap> {
    pub(crate) program: &'a LinkedProgram,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) host: Option<&'a mut dyn crate::method_runtime::HostIteratorAccess>,
}

#[derive(Clone, Copy)]
struct CallbackMethodIds {
    array_map: MethodId,
    array_filter: MethodId,
    array_retain: MethodId,
    array_find: MethodId,
    array_any: MethodId,
    array_all: MethodId,
    array_count: MethodId,
    array_sum: MethodId,
    array_group_by: MethodId,
    array_sort_by: MethodId,
    map_filter: MethodId,
    map_group_by: MethodId,
    map_retain: MethodId,
    map_find: MethodId,
    map_any: MethodId,
    map_all: MethodId,
    map_count: MethodId,
    map_map_values: MethodId,
    set_map: MethodId,
    set_filter: MethodId,
    set_retain: MethodId,
    set_find: MethodId,
    set_any: MethodId,
    set_all: MethodId,
    set_count: MethodId,
    option_map: MethodId,
    option_and_then: MethodId,
    option_or_else: MethodId,
    option_filter: MethodId,
    result_map: MethodId,
    result_map_err: MethodId,
    result_and_then: MethodId,
    result_or_else: MethodId,
    iterator_next: MethodId,
    iterator_map: MethodId,
    iterator_filter: MethodId,
    iterator_find: MethodId,
    iterator_any: MethodId,
    iterator_all: MethodId,
    iterator_count: MethodId,
    iterator_collect_array: MethodId,
    iterator_collect_set: MethodId,
    iterator_collect_map: MethodId,
}

impl CallbackMethodIds {
    fn new() -> Self {
        Self {
            array_map: standard_method_id("Array", "map"),
            array_filter: standard_method_id("Array", "filter"),
            array_retain: standard_method_id("Array", "retain"),
            array_find: standard_method_id("Array", "find"),
            array_any: standard_method_id("Array", "any"),
            array_all: standard_method_id("Array", "all"),
            array_count: standard_method_id("Array", "count"),
            array_sum: standard_method_id("Array", "sum"),
            array_group_by: standard_method_id("Array", "group_by"),
            array_sort_by: standard_method_id("Array", "sort_by"),
            map_filter: standard_method_id("Map", "filter"),
            map_group_by: standard_method_id("Map", "group_by"),
            map_retain: standard_method_id("Map", "retain"),
            map_find: standard_method_id("Map", "find"),
            map_any: standard_method_id("Map", "any"),
            map_all: standard_method_id("Map", "all"),
            map_count: standard_method_id("Map", "count"),
            map_map_values: standard_method_id("Map", "map_values"),
            set_map: standard_method_id("Set", "map"),
            set_filter: standard_method_id("Set", "filter"),
            set_retain: standard_method_id("Set", "retain"),
            set_find: standard_method_id("Set", "find"),
            set_any: standard_method_id("Set", "any"),
            set_all: standard_method_id("Set", "all"),
            set_count: standard_method_id("Set", "count"),
            option_map: standard_method_id("Option", "map"),
            option_and_then: standard_method_id("Option", "and_then"),
            option_or_else: standard_method_id("Option", "or_else"),
            option_filter: standard_method_id("Option", "filter"),
            result_map: standard_method_id("Result", "map"),
            result_map_err: standard_method_id("Result", "map_err"),
            result_and_then: standard_method_id("Result", "and_then"),
            result_or_else: standard_method_id("Result", "or_else"),
            iterator_next: standard_method_id("Iterator", "next"),
            iterator_map: standard_method_id("Iterator", "map"),
            iterator_filter: standard_method_id("Iterator", "filter"),
            iterator_find: standard_method_id("Iterator", "find"),
            iterator_any: standard_method_id("Iterator", "any"),
            iterator_all: standard_method_id("Iterator", "all"),
            iterator_count: standard_method_id("Iterator", "count"),
            iterator_collect_array: standard_method_id("Iterator", "collect_array"),
            iterator_collect_set: standard_method_id("Iterator", "collect_set"),
            iterator_collect_map: standard_method_id("Iterator", "collect_map"),
        }
    }
}

fn callback_method_ids() -> &'static CallbackMethodIds {
    static IDS: OnceLock<CallbackMethodIds> = OnceLock::new();
    IDS.get_or_init(CallbackMethodIds::new)
}

fn standard_method_id(owner: &str, name: &str) -> MethodId {
    let Some(id) = vela_stdlib::std_method_id(owner, name) else {
        panic!("missing standard method identity for {owner}::{name}");
    };
    id
}

impl<'a, 'heap> CallbackMethodDispatch<'a, 'heap> {
    fn runtime<'dispatch>(&'dispatch mut self) -> MethodRuntime<'dispatch, 'heap, 'a> {
        MethodRuntime {
            program: self.program,
            heap: self.heap.as_deref_mut(),
            budget: self.budget.as_deref_mut(),
            host: self.host.as_deref_mut(),
        }
    }

    pub(crate) fn heap_ref(&self) -> Option<&HeapExecution<'heap>> {
        self.heap.as_deref()
    }
}

pub(crate) fn call_by_id(
    method_id: MethodId,
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_>,
) -> Option<VmResult<Value>> {
    let cache = callback_cache_entry(method_id, receiver, dispatch.heap_ref())?;
    call_cached(receiver, cache, args, dispatch)
}

pub(crate) fn callback_cache_entry(
    method_id: MethodId,
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> Option<CallbackMethodInlineCacheEntry> {
    let receiver = if array_methods::is_array(receiver, heap) {
        StandardMethodReceiver::Array
    } else if map_methods::is_map(receiver, heap) {
        StandardMethodReceiver::Map
    } else if set_methods::is_set(receiver, heap) {
        StandardMethodReceiver::Set
    } else if option_result_methods::is_option(receiver, heap) {
        StandardMethodReceiver::Option
    } else if option_result_methods::is_result(receiver, heap) {
        StandardMethodReceiver::Result
    } else if iteration::is_iterator(receiver, heap) {
        StandardMethodReceiver::Iterator
    } else {
        return None;
    };
    let target = callback_method_target(receiver, method_id)?;
    Some(CallbackMethodInlineCacheEntry { receiver, target })
}

/// Resolves a callback-bearing standard collection method without inspecting
/// the receiver value.
///
/// HostRef-backed collection views retain their concrete Rust identity at
/// runtime, so they do not look like an owned Array, Map, or Set to the normal
/// value classifier. Their linked method identity still names exactly one
/// standard collection surface. This helper recovers that surface before the
/// host collection is projected into a temporary script-owned snapshot.
pub(crate) fn host_collection_callback_cache_entry(
    method_id: MethodId,
) -> Option<(CallbackMethodInlineCacheEntry, HostCollectionProjection)> {
    [
        (
            StandardMethodReceiver::Array,
            HostCollectionProjection::Values,
        ),
        (
            StandardMethodReceiver::Map,
            HostCollectionProjection::Entries,
        ),
        (
            StandardMethodReceiver::Set,
            HostCollectionProjection::Values,
        ),
    ]
    .into_iter()
    .find_map(|(receiver, projection)| {
        callback_method_target(receiver, method_id).map(|target| {
            (
                CallbackMethodInlineCacheEntry { receiver, target },
                projection,
            )
        })
    })
}

pub(crate) fn callback_cache_entry_matches_method_id(
    method_id: MethodId,
    cache: CallbackMethodInlineCacheEntry,
) -> bool {
    callback_method_target(cache.receiver, method_id) == Some(cache.target)
}

fn callback_method_target(
    receiver: StandardMethodReceiver,
    method_id: MethodId,
) -> Option<CallbackMethodInlineCacheTarget> {
    let ids = callback_method_ids();
    let target = match (receiver, method_id) {
        (StandardMethodReceiver::Array, id) if id == ids.array_map => {
            CallbackMethodInlineCacheTarget::Map
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_filter => {
            CallbackMethodInlineCacheTarget::Filter
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_retain => {
            CallbackMethodInlineCacheTarget::Retain
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_find => {
            CallbackMethodInlineCacheTarget::Find
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_any => {
            CallbackMethodInlineCacheTarget::Any
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_all => {
            CallbackMethodInlineCacheTarget::All
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_count => {
            CallbackMethodInlineCacheTarget::Count
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_sum => {
            CallbackMethodInlineCacheTarget::Sum
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_group_by => {
            CallbackMethodInlineCacheTarget::GroupBy
        }
        (StandardMethodReceiver::Array, id) if id == ids.array_sort_by => {
            CallbackMethodInlineCacheTarget::SortBy
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_filter => {
            CallbackMethodInlineCacheTarget::Filter
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_group_by => {
            CallbackMethodInlineCacheTarget::GroupBy
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_retain => {
            CallbackMethodInlineCacheTarget::Retain
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_find => {
            CallbackMethodInlineCacheTarget::Find
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_any => {
            CallbackMethodInlineCacheTarget::Any
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_all => {
            CallbackMethodInlineCacheTarget::All
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_count => {
            CallbackMethodInlineCacheTarget::Count
        }
        (StandardMethodReceiver::Map, id) if id == ids.map_map_values => {
            CallbackMethodInlineCacheTarget::MapValues
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_map => {
            CallbackMethodInlineCacheTarget::Map
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_filter => {
            CallbackMethodInlineCacheTarget::Filter
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_retain => {
            CallbackMethodInlineCacheTarget::Retain
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_find => {
            CallbackMethodInlineCacheTarget::Find
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_any => {
            CallbackMethodInlineCacheTarget::Any
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_all => {
            CallbackMethodInlineCacheTarget::All
        }
        (StandardMethodReceiver::Set, id) if id == ids.set_count => {
            CallbackMethodInlineCacheTarget::Count
        }
        (StandardMethodReceiver::Option, id) if id == ids.option_map => {
            CallbackMethodInlineCacheTarget::Map
        }
        (StandardMethodReceiver::Option, id) if id == ids.option_and_then => {
            CallbackMethodInlineCacheTarget::AndThen
        }
        (StandardMethodReceiver::Option, id) if id == ids.option_or_else => {
            CallbackMethodInlineCacheTarget::OrElse
        }
        (StandardMethodReceiver::Option, id) if id == ids.option_filter => {
            CallbackMethodInlineCacheTarget::Filter
        }
        (StandardMethodReceiver::Result, id) if id == ids.result_map => {
            CallbackMethodInlineCacheTarget::Map
        }
        (StandardMethodReceiver::Result, id) if id == ids.result_map_err => {
            CallbackMethodInlineCacheTarget::MapErr
        }
        (StandardMethodReceiver::Result, id) if id == ids.result_and_then => {
            CallbackMethodInlineCacheTarget::AndThen
        }
        (StandardMethodReceiver::Result, id) if id == ids.result_or_else => {
            CallbackMethodInlineCacheTarget::OrElse
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_next => {
            CallbackMethodInlineCacheTarget::Next
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_map => {
            CallbackMethodInlineCacheTarget::Map
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_filter => {
            CallbackMethodInlineCacheTarget::Filter
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_find => {
            CallbackMethodInlineCacheTarget::Find
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_any => {
            CallbackMethodInlineCacheTarget::Any
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_all => {
            CallbackMethodInlineCacheTarget::All
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_count => {
            CallbackMethodInlineCacheTarget::Count
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_collect_array => {
            CallbackMethodInlineCacheTarget::CollectArray
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_collect_set => {
            CallbackMethodInlineCacheTarget::CollectSet
        }
        (StandardMethodReceiver::Iterator, id) if id == ids.iterator_collect_map => {
            CallbackMethodInlineCacheTarget::CollectMap
        }
        _ => return None,
    };
    Some(target)
}

pub(crate) fn call_cached(
    receiver: &Value,
    cache: CallbackMethodInlineCacheEntry,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_>,
) -> Option<VmResult<Value>> {
    if !receiver_matches_cache(receiver, cache.receiver, dispatch.heap_ref()) {
        return None;
    }
    let result = match (cache.receiver, cache.target) {
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::Map) => {
            array_methods::map(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Set, CallbackMethodInlineCacheTarget::Map) => {
            set_methods::map(receiver, args, dispatch.runtime())
        }
        (
            StandardMethodReceiver::Option | StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::Map,
        ) => {
            if let Some(result) = option_result_inactive::call_cached(
                receiver,
                cache.receiver,
                cache.target,
                args,
                dispatch,
            ) {
                result
            } else {
                option_result_methods::map(receiver, args, dispatch.runtime())
            }
        }
        (StandardMethodReceiver::Result, CallbackMethodInlineCacheTarget::MapErr) => {
            if let Some(result) = option_result_inactive::call_cached(
                receiver,
                cache.receiver,
                cache.target,
                args,
                dispatch,
            ) {
                result
            } else {
                option_result_methods::map_err(receiver, args, dispatch.runtime())
            }
        }
        (
            StandardMethodReceiver::Option | StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::AndThen,
        ) => {
            if let Some(result) = option_result_inactive::call_cached(
                receiver,
                cache.receiver,
                cache.target,
                args,
                dispatch,
            ) {
                result
            } else {
                option_result_methods::and_then(receiver, args, dispatch.runtime())
            }
        }
        (
            StandardMethodReceiver::Option | StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::OrElse,
        ) => {
            if let Some(result) = option_result_inactive::call_cached(
                receiver,
                cache.receiver,
                cache.target,
                args,
                dispatch,
            ) {
                result
            } else {
                option_result_methods::or_else(receiver, args, dispatch.runtime())
            }
        }
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::Filter) => {
            array_methods::filter(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Map, CallbackMethodInlineCacheTarget::Filter) => {
            map_methods::filter(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Map, CallbackMethodInlineCacheTarget::GroupBy) => {
            map_methods::group_by(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Set, CallbackMethodInlineCacheTarget::Filter) => {
            set_methods::filter(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::Retain) => {
            array_methods::retain(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Map, CallbackMethodInlineCacheTarget::Retain) => {
            map_methods::retain(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Set, CallbackMethodInlineCacheTarget::Retain) => {
            set_methods::retain(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Option, CallbackMethodInlineCacheTarget::Filter) => {
            if let Some(result) = option_result_inactive::call_cached(
                receiver,
                cache.receiver,
                cache.target,
                args,
                dispatch,
            ) {
                result
            } else {
                option_result_methods::filter(receiver, args, dispatch.runtime())
            }
        }
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::Find) => {
            array_methods::find(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Map, CallbackMethodInlineCacheTarget::Find) => {
            map_methods::find(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Set, CallbackMethodInlineCacheTarget::Find) => {
            set_methods::find(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::Any) => {
            array_methods::any(receiver, args, dispatch.runtime()).map(Value::Bool)
        }
        (StandardMethodReceiver::Map, CallbackMethodInlineCacheTarget::Any) => {
            map_methods::any(receiver, args, dispatch.runtime()).map(Value::Bool)
        }
        (StandardMethodReceiver::Set, CallbackMethodInlineCacheTarget::Any) => {
            set_methods::any(receiver, args, dispatch.runtime()).map(Value::Bool)
        }
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::All) => {
            array_methods::all(receiver, args, dispatch.runtime()).map(Value::Bool)
        }
        (StandardMethodReceiver::Map, CallbackMethodInlineCacheTarget::All) => {
            map_methods::all(receiver, args, dispatch.runtime()).map(Value::Bool)
        }
        (StandardMethodReceiver::Set, CallbackMethodInlineCacheTarget::All) => {
            set_methods::all(receiver, args, dispatch.runtime()).map(Value::Bool)
        }
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::Count) => {
            array_methods::count(receiver, args, dispatch.runtime()).map(Value::i64)
        }
        (StandardMethodReceiver::Map, CallbackMethodInlineCacheTarget::Count) => {
            map_methods::count(receiver, args, dispatch.runtime()).map(Value::i64)
        }
        (StandardMethodReceiver::Set, CallbackMethodInlineCacheTarget::Count) => {
            set_methods::count(receiver, args, dispatch.runtime()).map(Value::i64)
        }
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::Sum) => {
            if args.is_empty() {
                array_methods::sum_values(receiver, dispatch.heap_ref(), "method sum")
            } else {
                array_methods::sum(receiver, args, dispatch.runtime())
            }
        }
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::GroupBy) => {
            array_methods::group_by(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::SortBy) => Err(
            crate::VmError::new(crate::VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "sort_by escaped the execution session",
            }),
        ),
        (StandardMethodReceiver::Map, CallbackMethodInlineCacheTarget::MapValues) => {
            map_methods::map_values(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::Next) => {
            iteration::next_method_runtime(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::Map) => {
            iteration::map_method(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::Filter) => {
            iteration::filter_method(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::Find) => {
            iteration::find_method(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::Any) => {
            iteration::any_method(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::All) => {
            iteration::all_method(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::Count) => {
            iteration::count_method_runtime(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::CollectArray) => {
            iteration::collect_array_method_runtime(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::CollectSet) => {
            iteration::collect_set_method_runtime(receiver, args, dispatch.runtime())
        }
        (StandardMethodReceiver::Iterator, CallbackMethodInlineCacheTarget::CollectMap) => {
            iteration::collect_map_method_runtime(receiver, args, dispatch.runtime())
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
        StandardMethodReceiver::Array => array_methods::is_array(receiver, heap),
        StandardMethodReceiver::Map => map_methods::is_map(receiver, heap),
        StandardMethodReceiver::Set => set_methods::is_set(receiver, heap),
        StandardMethodReceiver::Option => option_result_methods::is_option(receiver, heap),
        StandardMethodReceiver::Result => option_result_methods::is_result(receiver, heap),
        StandardMethodReceiver::Iterator => iteration::is_iterator(receiver, heap),
        StandardMethodReceiver::Char => false,
        StandardMethodReceiver::String
        | StandardMethodReceiver::Bytes
        | StandardMethodReceiver::Range => false,
    }
}
