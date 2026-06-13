use super::{ExecutionMode, Workload};
use crate::workload_sources::{
    ITERATOR_ARRAY_PIPELINE_SOURCE, ITERATOR_ARRAY_SHORT_CIRCUIT_SOURCE,
    ITERATOR_HOST_ITERABLE_SOURCE, ITERATOR_MAP_VIEWS_SOURCE, ITERATOR_RANGE_FAST_PATH_SOURCE,
    ITERATOR_STRING_BYTES_SOURCE, ITERATOR_STRING_CHARS_SOURCE,
};

pub(crate) const ITERATOR_SEQUENCE_WORKLOADS: &[Workload] = &[
    Workload {
        name: "string_iterator_chars",
        mode: ExecutionMode::Inline,
        source: ITERATOR_STRING_CHARS_SOURCE,
    },
    Workload {
        name: "string_iterator_chars_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ITERATOR_STRING_CHARS_SOURCE,
    },
    Workload {
        name: "string_iterator_chars_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ITERATOR_STRING_CHARS_SOURCE,
    },
    Workload {
        name: "string_iterator_bytes",
        mode: ExecutionMode::Inline,
        source: ITERATOR_STRING_BYTES_SOURCE,
    },
    Workload {
        name: "string_iterator_bytes_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ITERATOR_STRING_BYTES_SOURCE,
    },
    Workload {
        name: "string_iterator_bytes_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ITERATOR_STRING_BYTES_SOURCE,
    },
    Workload {
        name: "collection_array_iter_pipeline",
        mode: ExecutionMode::Inline,
        source: ITERATOR_ARRAY_PIPELINE_SOURCE,
    },
    Workload {
        name: "collection_array_iter_pipeline_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ITERATOR_ARRAY_PIPELINE_SOURCE,
    },
    Workload {
        name: "collection_array_iter_pipeline_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ITERATOR_ARRAY_PIPELINE_SOURCE,
    },
    Workload {
        name: "collection_array_iter_short_circuit",
        mode: ExecutionMode::Inline,
        source: ITERATOR_ARRAY_SHORT_CIRCUIT_SOURCE,
    },
    Workload {
        name: "collection_array_iter_short_circuit_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ITERATOR_ARRAY_SHORT_CIRCUIT_SOURCE,
    },
    Workload {
        name: "collection_array_iter_short_circuit_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ITERATOR_ARRAY_SHORT_CIRCUIT_SOURCE,
    },
    Workload {
        name: "collection_map_iterator_views",
        mode: ExecutionMode::Inline,
        source: ITERATOR_MAP_VIEWS_SOURCE,
    },
    Workload {
        name: "collection_map_iterator_views_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ITERATOR_MAP_VIEWS_SOURCE,
    },
    Workload {
        name: "collection_map_iterator_views_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ITERATOR_MAP_VIEWS_SOURCE,
    },
    Workload {
        name: "range_iterator_fast_path",
        mode: ExecutionMode::Inline,
        source: ITERATOR_RANGE_FAST_PATH_SOURCE,
    },
    Workload {
        name: "range_iterator_fast_path_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ITERATOR_RANGE_FAST_PATH_SOURCE,
    },
    Workload {
        name: "range_iterator_fast_path_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ITERATOR_RANGE_FAST_PATH_SOURCE,
    },
    Workload {
        name: "host_iterable_iteration",
        mode: ExecutionMode::Inline,
        source: ITERATOR_HOST_ITERABLE_SOURCE,
    },
    Workload {
        name: "host_iterable_iteration_hot_offsets",
        mode: ExecutionMode::ProfileOnly,
        source: ITERATOR_HOST_ITERABLE_SOURCE,
    },
    Workload {
        name: "host_iterable_iteration_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ITERATOR_HOST_ITERABLE_SOURCE,
    },
];
