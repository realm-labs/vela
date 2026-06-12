use super::{ExecutionMode, Workload};
use crate::workload_sources::{
    ARRAY_EDGE_SOURCE, ARRAY_EXTEND_SOURCE, ARRAY_LOOKUP_SOURCE, ARRAY_MUTATION_SOURCE,
    MAP_EXTEND_SOURCE, MAP_LOOKUP_SOURCE, MAP_MERGE_SOURCE, MAP_MUTATION_SOURCE, MAP_VIEWS_SOURCE,
    OPTION_RESULT_CALLBACKS_SOURCE, OPTION_RESULT_CONVERSIONS_SOURCE, OPTION_RESULT_HELPERS_SOURCE,
    OPTION_RESULT_PREDICATES_SOURCE, SET_COMBINATION_SOURCE, SET_LOOKUP_SOURCE,
    SET_MUTATION_SOURCE, SET_VALUES_SOURCE,
};

pub(crate) const COLLECTION_FAMILY_WORKLOADS: &[Workload] = &[
    Workload {
        name: "managed_heap_option_result_helpers",
        mode: ExecutionMode::ManagedHeap,
        source: OPTION_RESULT_HELPERS_SOURCE,
    },
    Workload {
        name: "option_result_helpers",
        mode: ExecutionMode::Inline,
        source: OPTION_RESULT_HELPERS_SOURCE,
    },
    Workload {
        name: "option_result_helpers_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: OPTION_RESULT_HELPERS_SOURCE,
    },
    Workload {
        name: "option_result_helpers_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: OPTION_RESULT_HELPERS_SOURCE,
    },
    Workload {
        name: "managed_heap_option_result_predicates",
        mode: ExecutionMode::ManagedHeap,
        source: OPTION_RESULT_PREDICATES_SOURCE,
    },
    Workload {
        name: "option_result_predicates",
        mode: ExecutionMode::Inline,
        source: OPTION_RESULT_PREDICATES_SOURCE,
    },
    Workload {
        name: "option_result_predicates_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: OPTION_RESULT_PREDICATES_SOURCE,
    },
    Workload {
        name: "option_result_predicates_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: OPTION_RESULT_PREDICATES_SOURCE,
    },
    Workload {
        name: "managed_heap_option_result_conversions",
        mode: ExecutionMode::ManagedHeap,
        source: OPTION_RESULT_CONVERSIONS_SOURCE,
    },
    Workload {
        name: "option_result_conversions",
        mode: ExecutionMode::Inline,
        source: OPTION_RESULT_CONVERSIONS_SOURCE,
    },
    Workload {
        name: "option_result_conversions_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: OPTION_RESULT_CONVERSIONS_SOURCE,
    },
    Workload {
        name: "option_result_conversions_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: OPTION_RESULT_CONVERSIONS_SOURCE,
    },
    Workload {
        name: "managed_heap_option_result_callbacks",
        mode: ExecutionMode::ManagedHeap,
        source: OPTION_RESULT_CALLBACKS_SOURCE,
    },
    Workload {
        name: "option_result_callbacks",
        mode: ExecutionMode::Inline,
        source: OPTION_RESULT_CALLBACKS_SOURCE,
    },
    Workload {
        name: "option_result_callbacks_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: OPTION_RESULT_CALLBACKS_SOURCE,
    },
    Workload {
        name: "option_result_callbacks_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: OPTION_RESULT_CALLBACKS_SOURCE,
    },
    Workload {
        name: "managed_heap_set_lookup",
        mode: ExecutionMode::ManagedHeap,
        source: SET_LOOKUP_SOURCE,
    },
    Workload {
        name: "set_lookup",
        mode: ExecutionMode::Inline,
        source: SET_LOOKUP_SOURCE,
    },
    Workload {
        name: "set_lookup_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: SET_LOOKUP_SOURCE,
    },
    Workload {
        name: "set_lookup_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: SET_LOOKUP_SOURCE,
    },
    Workload {
        name: "managed_heap_set_values",
        mode: ExecutionMode::ManagedHeap,
        source: SET_VALUES_SOURCE,
    },
    Workload {
        name: "set_values",
        mode: ExecutionMode::Inline,
        source: SET_VALUES_SOURCE,
    },
    Workload {
        name: "set_values_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: SET_VALUES_SOURCE,
    },
    Workload {
        name: "set_values_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: SET_VALUES_SOURCE,
    },
    Workload {
        name: "managed_heap_set_mutation",
        mode: ExecutionMode::ManagedHeap,
        source: SET_MUTATION_SOURCE,
    },
    Workload {
        name: "set_mutation",
        mode: ExecutionMode::Inline,
        source: SET_MUTATION_SOURCE,
    },
    Workload {
        name: "set_mutation_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: SET_MUTATION_SOURCE,
    },
    Workload {
        name: "set_mutation_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: SET_MUTATION_SOURCE,
    },
    Workload {
        name: "managed_heap_set_combination",
        mode: ExecutionMode::ManagedHeap,
        source: SET_COMBINATION_SOURCE,
    },
    Workload {
        name: "set_combination",
        mode: ExecutionMode::Inline,
        source: SET_COMBINATION_SOURCE,
    },
    Workload {
        name: "set_combination_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: SET_COMBINATION_SOURCE,
    },
    Workload {
        name: "set_combination_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: SET_COMBINATION_SOURCE,
    },
    Workload {
        name: "managed_heap_array_lookup",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_LOOKUP_SOURCE,
    },
    Workload {
        name: "array_lookup",
        mode: ExecutionMode::Inline,
        source: ARRAY_LOOKUP_SOURCE,
    },
    Workload {
        name: "array_lookup_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ARRAY_LOOKUP_SOURCE,
    },
    Workload {
        name: "array_lookup_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_LOOKUP_SOURCE,
    },
    Workload {
        name: "managed_heap_array_edges",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_EDGE_SOURCE,
    },
    Workload {
        name: "array_edges",
        mode: ExecutionMode::Inline,
        source: ARRAY_EDGE_SOURCE,
    },
    Workload {
        name: "array_edges_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ARRAY_EDGE_SOURCE,
    },
    Workload {
        name: "array_edges_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_EDGE_SOURCE,
    },
    Workload {
        name: "managed_heap_array_extend",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_EXTEND_SOURCE,
    },
    Workload {
        name: "array_extend",
        mode: ExecutionMode::Inline,
        source: ARRAY_EXTEND_SOURCE,
    },
    Workload {
        name: "array_extend_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ARRAY_EXTEND_SOURCE,
    },
    Workload {
        name: "array_extend_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_EXTEND_SOURCE,
    },
    Workload {
        name: "managed_heap_array_mutation",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_MUTATION_SOURCE,
    },
    Workload {
        name: "array_mutation",
        mode: ExecutionMode::Inline,
        source: ARRAY_MUTATION_SOURCE,
    },
    Workload {
        name: "array_mutation_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_MUTATION_SOURCE,
    },
    Workload {
        name: "managed_heap_map_lookup",
        mode: ExecutionMode::ManagedHeap,
        source: MAP_LOOKUP_SOURCE,
    },
    Workload {
        name: "map_lookup",
        mode: ExecutionMode::Inline,
        source: MAP_LOOKUP_SOURCE,
    },
    Workload {
        name: "map_lookup_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: MAP_LOOKUP_SOURCE,
    },
    Workload {
        name: "map_lookup_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: MAP_LOOKUP_SOURCE,
    },
    Workload {
        name: "managed_heap_map_views",
        mode: ExecutionMode::ManagedHeap,
        source: MAP_VIEWS_SOURCE,
    },
    Workload {
        name: "map_views",
        mode: ExecutionMode::Inline,
        source: MAP_VIEWS_SOURCE,
    },
    Workload {
        name: "map_views_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: MAP_VIEWS_SOURCE,
    },
    Workload {
        name: "map_views_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: MAP_VIEWS_SOURCE,
    },
    Workload {
        name: "managed_heap_map_mutation",
        mode: ExecutionMode::ManagedHeap,
        source: MAP_MUTATION_SOURCE,
    },
    Workload {
        name: "map_mutation",
        mode: ExecutionMode::Inline,
        source: MAP_MUTATION_SOURCE,
    },
    Workload {
        name: "map_mutation_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: MAP_MUTATION_SOURCE,
    },
    Workload {
        name: "map_mutation_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: MAP_MUTATION_SOURCE,
    },
    Workload {
        name: "managed_heap_map_merge",
        mode: ExecutionMode::ManagedHeap,
        source: MAP_MERGE_SOURCE,
    },
    Workload {
        name: "map_merge",
        mode: ExecutionMode::Inline,
        source: MAP_MERGE_SOURCE,
    },
    Workload {
        name: "map_merge_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: MAP_MERGE_SOURCE,
    },
    Workload {
        name: "map_merge_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: MAP_MERGE_SOURCE,
    },
    Workload {
        name: "managed_heap_map_extend",
        mode: ExecutionMode::ManagedHeap,
        source: MAP_EXTEND_SOURCE,
    },
    Workload {
        name: "map_extend",
        mode: ExecutionMode::Inline,
        source: MAP_EXTEND_SOURCE,
    },
    Workload {
        name: "map_extend_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: MAP_EXTEND_SOURCE,
    },
    Workload {
        name: "map_extend_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: MAP_EXTEND_SOURCE,
    },
];
