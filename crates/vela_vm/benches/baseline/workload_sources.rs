#[path = "workload_sources/calls.rs"]
mod calls;

#[path = "workload_sources/ranges.rs"]
mod ranges;

#[path = "workload_sources/strings.rs"]
mod strings;

#[path = "workload_sources/iterators.rs"]
mod iterators;

#[path = "workload_sources/sets.rs"]
mod sets;

#[path = "workload_sources/typed_containers.rs"]
mod typed_containers;

pub(crate) use calls::{
    DIRECT_CLOSURE_CALLS_SOURCE, DYNAMIC_METHOD_CACHE_MISS_SOURCE,
    DYNAMIC_METHOD_POLYMORPHIC_SOURCE, DYNAMIC_SCRIPT_METHOD_MONOMORPHIC_SOURCE,
    DYNAMIC_STRING_METHOD_MONOMORPHIC_SOURCE, METHOD_DISPATCH_SOURCE, NATIVE_CALL_WIDE_ARGS_SOURCE,
    SCRIPT_CALL_SMALL_ARGS_SOURCE, SCRIPT_CALL_WIDE_ARGS_SOURCE, SCRIPT_METHOD_DISPATCH_SOURCE,
    TRAIT_METHOD_DISPATCH_SOURCE,
};
pub(crate) use iterators::{
    ITERATOR_ARRAY_PIPELINE_SOURCE, ITERATOR_ARRAY_SHORT_CIRCUIT_SOURCE,
    ITERATOR_HOST_ITERABLE_SOURCE, ITERATOR_MAP_VIEWS_SOURCE, ITERATOR_RANGE_FAST_PATH_SOURCE,
    ITERATOR_STRING_BYTES_SOURCE, ITERATOR_STRING_CHARS_SOURCE,
};
pub(crate) use ranges::RANGE_METHODS_SOURCE;
pub(crate) use sets::{
    SET_CALLBACK_PREDICATES_SOURCE, SET_COMBINATION_SOURCE, SET_LOOKUP_SOURCE, SET_MUTATION_SOURCE,
    SET_VALUES_SOURCE,
};
pub(crate) use strings::{
    STRING_METHODS_SOURCE, STRING_OPTIONS_SOURCE, STRING_PARSING_SOURCE, STRING_SPLITTING_SOURCE,
    STRING_TRANSFORMS_SOURCE,
};
pub(crate) use typed_containers::{
    TYPED_CONTAINER_ARRAY_I64_PUSH_DYNAMIC_GUARDED_SOURCE,
    TYPED_CONTAINER_ARRAY_I64_PUSH_STATIC_SOURCE, TYPED_CONTAINER_ARRAY_PUSH_ERASED_SOURCE,
    TYPED_CONTAINER_MAP_STRING_I64_UPDATE_DYNAMIC_GUARDED_SOURCE,
    TYPED_CONTAINER_MAP_STRING_I64_UPDATE_STATIC_SOURCE, TYPED_CONTAINER_MAP_UPDATE_ERASED_SOURCE,
};

pub(crate) const STDLIB_COLLECTIONS_SOURCE: &str =
    include_str!("../fixtures/baseline/stdlib_collections.vela");

pub(crate) const VM_STATE_READ_WRITE_SOURCE: &str =
    include_str!("../fixtures/baseline/vm_state_read_write.vela");

pub(crate) const MATERIALIZATION_SOURCE: &str =
    include_str!("../fixtures/baseline/materialization.vela");

pub(crate) const RECORD_TRIPLETS_SOURCE: &str =
    include_str!("../fixtures/baseline/record_triplets.vela");

pub(crate) const RECORD_QUADS_SOURCE: &str = include_str!("../fixtures/baseline/record_quads.vela");

pub(crate) const RECORD_QUINTS_SOURCE: &str =
    include_str!("../fixtures/baseline/record_quints.vela");

pub(crate) const RECORD_SEXTETS_SOURCE: &str =
    include_str!("../fixtures/baseline/record_sextets.vela");

pub(crate) const CALLBACK_COLLECTIONS_SOURCE: &str =
    include_str!("../fixtures/baseline/callback_collections.vela");

pub(crate) const ARRAY_CALLBACK_PREDICATES_SOURCE: &str =
    include_str!("../fixtures/baseline/array_callback_predicates.vela");

pub(crate) const MAP_CALLBACKS_SOURCE: &str =
    include_str!("../fixtures/baseline/map_callbacks.vela");

pub(crate) const MAP_FIND_ENTRIES_SOURCE: &str =
    include_str!("../fixtures/baseline/map_find_entries.vela");

pub(crate) const ARRAY_GROUP_BY_SOURCE: &str =
    include_str!("../fixtures/baseline/array_group_by.vela");

pub(crate) const ARRAY_SUM_SOURCE: &str = include_str!("../fixtures/baseline/array_sum.vela");

pub(crate) const BYTES_METHODS_SOURCE: &str =
    include_str!("../fixtures/baseline/bytes_methods.vela");

pub(crate) const BYTES_ACCESS_SOURCE: &str = include_str!("../fixtures/baseline/bytes_access.vela");

pub(crate) const BYTES_MATERIALIZATION_SOURCE: &str =
    include_str!("../fixtures/baseline/bytes_materialization.vela");

pub(crate) const ARRAY_LOOKUP_SOURCE: &str = include_str!("../fixtures/baseline/array_lookup.vela");

pub(crate) const ARRAY_EDGE_SOURCE: &str = include_str!("../fixtures/baseline/array_edge.vela");

pub(crate) const ARRAY_EXTEND_SOURCE: &str = include_str!("../fixtures/baseline/array_extend.vela");

pub(crate) const ARRAY_MUTATION_SOURCE: &str =
    include_str!("../fixtures/baseline/array_mutation.vela");

pub(crate) const ARRAY_EXTREMA_SOURCE: &str =
    include_str!("../fixtures/baseline/array_extrema.vela");

pub(crate) const ARRAY_SORT_SOURCE: &str = include_str!("../fixtures/baseline/array_sort.vela");

pub(crate) const ARRAY_SLICE_SOURCE: &str = include_str!("../fixtures/baseline/array_slice.vela");

pub(crate) const ARRAY_REVERSE_SOURCE: &str =
    include_str!("../fixtures/baseline/array_reverse.vela");

pub(crate) const ARRAY_DISTINCT_SOURCE: &str =
    include_str!("../fixtures/baseline/array_distinct.vela");

pub(crate) const ARRAY_JOIN_SOURCE: &str = include_str!("../fixtures/baseline/array_join.vela");

pub(crate) const MAP_LOOKUP_SOURCE: &str = include_str!("../fixtures/baseline/map_lookup.vela");

pub(crate) const MAP_VIEWS_SOURCE: &str = include_str!("../fixtures/baseline/map_views.vela");

pub(crate) const MAP_MUTATION_SOURCE: &str = include_str!("../fixtures/baseline/map_mutation.vela");

pub(crate) const MAP_MERGE_SOURCE: &str = include_str!("../fixtures/baseline/map_merge.vela");

pub(crate) const MAP_EXTEND_SOURCE: &str = include_str!("../fixtures/baseline/map_extend.vela");

pub(crate) const OPTION_RESULT_HELPERS_SOURCE: &str =
    include_str!("../fixtures/baseline/option_result_helpers.vela");

pub(crate) const OPTION_RESULT_PREDICATES_SOURCE: &str =
    include_str!("../fixtures/baseline/option_result_predicates.vela");

pub(crate) const OPTION_RESULT_CONVERSIONS_SOURCE: &str =
    include_str!("../fixtures/baseline/option_result_conversions.vela");

pub(crate) const OPTION_RESULT_CALLBACKS_SOURCE: &str =
    include_str!("../fixtures/baseline/option_result_callbacks.vela");
