use crate::workload_sources::{
    ARRAY_DISTINCT_SOURCE, ARRAY_EDGE_SOURCE, ARRAY_EXTEND_SOURCE, ARRAY_EXTREMA_SOURCE,
    ARRAY_GROUP_BY_SOURCE, ARRAY_JOIN_SOURCE, ARRAY_LOOKUP_SOURCE, ARRAY_MUTATION_SOURCE,
    ARRAY_REVERSE_SOURCE, ARRAY_SLICE_SOURCE, ARRAY_SORT_SOURCE, ARRAY_SUM_SOURCE,
    BYTES_METHODS_SOURCE, CALLBACK_COLLECTIONS_SOURCE, DIRECT_CLOSURE_CALLS_SOURCE,
    HOST_DYNAMIC_KEY_ACCESS_SOURCE, HOST_FIELD_READ_WRITE_SOURCE, HOST_METHOD_CALLS_SOURCE,
    HOST_NESTED_READ_WRITE_SOURCE, HOST_RMW_MUTATION_SOURCE, MAP_CALLBACKS_SOURCE,
    MAP_EXTEND_SOURCE, MAP_FIND_ENTRIES_SOURCE, MAP_LOOKUP_SOURCE, MAP_MERGE_SOURCE,
    MAP_MUTATION_SOURCE, MAP_VIEWS_SOURCE, METHOD_DISPATCH_SOURCE, NATIVE_CALL_WIDE_ARGS_SOURCE,
    OPTION_RESULT_HELPERS_SOURCE, RECORD_QUADS_SOURCE, RECORD_QUINTS_SOURCE, RECORD_SEXTETS_SOURCE,
    RECORD_TRIPLETS_SOURCE, SCRIPT_CALL_SMALL_ARGS_SOURCE, SCRIPT_METHOD_DISPATCH_SOURCE,
    SET_COMBINATION_SOURCE, SET_LOOKUP_SOURCE, SET_MUTATION_SOURCE, SET_VALUES_SOURCE,
    STDLIB_COLLECTIONS_SOURCE, STRING_METHODS_SOURCE, TRAIT_METHOD_DISPATCH_SOURCE,
};

pub(crate) struct Workload {
    pub(crate) name: &'static str,
    pub(crate) mode: ExecutionMode,
    pub(crate) source: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) enum ExecutionMode {
    Inline,
    CacheEnabled,
    ScriptProgram,
    ScriptProgramCacheEnabled,
    ManagedHeap,
    HostAccess,
    HostAccessCacheEnabled,
    HostManagedHeapReadConversion,
    HostManagedHeapHostAccess,
    GameplayHost,
    GcPacing,
}

pub(crate) const WORKLOADS: &[Workload] = &[
    Workload {
        name: "scalar_branch_loop",
        mode: ExecutionMode::Inline,
        source: r#"
fn main() {
    let total = 0;
    for value in 0..200 {
        if value % 3 == 0 {
            total += value * 2;
            continue;
        }
        if value > 180 {
            break;
        }
        total += (value * 5) % 17;
    }
    return total;
}
"#,
    },
    Workload {
        name: "range_iteration",
        mode: ExecutionMode::Inline,
        source: r#"
fn main() {
    let total = 0;
    for outer in 0..8 {
        for value in 0..128 {
            total += value + outer - outer;
        }
    }
    for value in 0..=63 {
        total += value;
    }
    return total;
}
"#,
    },
    Workload {
        name: "scalar_dispatch_mix",
        mode: ExecutionMode::Inline,
        source: r#"
fn main() {
    let total = 0;
    let drift = 0.5;
    let label = "tick";
    let enabled = true;
    for tick in 0..180 {
        drift += 0.25;
        if drift > 12.0 {
            drift = 0.5;
        }
        if enabled && (tick % 2 == 0 || label == "tick") {
            total += tick * 3 - 1;
        }
        if !(label != "tick") && drift >= 1.0 {
            total += 2;
        }
        if tick > 150 && drift < 5.0 {
            break;
        }
    }
    return total;
}
"#,
    },
    Workload {
        name: "script_call_small_args",
        mode: ExecutionMode::ScriptProgram,
        source: SCRIPT_CALL_SMALL_ARGS_SOURCE,
    },
    Workload {
        name: "script_call_small_args_cache_hot_offsets",
        mode: ExecutionMode::ScriptProgramCacheEnabled,
        source: SCRIPT_CALL_SMALL_ARGS_SOURCE,
    },
    Workload {
        name: "script_call_wide_args",
        mode: ExecutionMode::ScriptProgram,
        source: r#"
fn mix_three(left, middle, right) {
    return left * 2 + middle - right;
}

fn mix_four(first, second, third, fourth) {
    return first + second * 3 - third + fourth;
}

fn main() {
    let total = 0;
    for tick in 0..240 {
        total += mix_three(tick, total % 19, tick % 7);
        total += mix_four(tick, total % 23, tick % 11, 5);
    }
    return total;
}
"#,
    },
    Workload {
        name: "native_call_wide_args",
        mode: ExecutionMode::Inline,
        source: NATIVE_CALL_WIDE_ARGS_SOURCE,
    },
    Workload {
        name: "native_call_wide_args_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: NATIVE_CALL_WIDE_ARGS_SOURCE,
    },
    Workload {
        name: "stdlib_collections",
        mode: ExecutionMode::Inline,
        source: STDLIB_COLLECTIONS_SOURCE,
    },
    Workload {
        name: "stdlib_collections_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: STDLIB_COLLECTIONS_SOURCE,
    },
    Workload {
        name: "managed_heap_string_methods",
        mode: ExecutionMode::ManagedHeap,
        source: STRING_METHODS_SOURCE,
    },
    Workload {
        name: "string_methods_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: STRING_METHODS_SOURCE,
    },
    Workload {
        name: "managed_heap_bytes_methods",
        mode: ExecutionMode::ManagedHeap,
        source: BYTES_METHODS_SOURCE,
    },
    Workload {
        name: "bytes_methods_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: BYTES_METHODS_SOURCE,
    },
    Workload {
        name: "callback_collections",
        mode: ExecutionMode::Inline,
        source: CALLBACK_COLLECTIONS_SOURCE,
    },
    Workload {
        name: "callback_collections_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: CALLBACK_COLLECTIONS_SOURCE,
    },
    Workload {
        name: "direct_closure_calls",
        mode: ExecutionMode::Inline,
        source: DIRECT_CLOSURE_CALLS_SOURCE,
    },
    Workload {
        name: "direct_closure_calls_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: DIRECT_CLOSURE_CALLS_SOURCE,
    },
    Workload {
        name: "method_dispatch",
        mode: ExecutionMode::Inline,
        source: METHOD_DISPATCH_SOURCE,
    },
    Workload {
        name: "method_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: METHOD_DISPATCH_SOURCE,
    },
    Workload {
        name: "script_method_dispatch",
        mode: ExecutionMode::ScriptProgram,
        source: SCRIPT_METHOD_DISPATCH_SOURCE,
    },
    Workload {
        name: "script_method_cache_hot_offsets",
        mode: ExecutionMode::ScriptProgramCacheEnabled,
        source: SCRIPT_METHOD_DISPATCH_SOURCE,
    },
    Workload {
        name: "trait_method_dispatch",
        mode: ExecutionMode::ScriptProgram,
        source: TRAIT_METHOD_DISPATCH_SOURCE,
    },
    Workload {
        name: "trait_method_cache_hot_offsets",
        mode: ExecutionMode::ScriptProgramCacheEnabled,
        source: TRAIT_METHOD_DISPATCH_SOURCE,
    },
    Workload {
        name: "managed_heap_callback_collections",
        mode: ExecutionMode::ManagedHeap,
        source: CALLBACK_COLLECTIONS_SOURCE,
    },
    Workload {
        name: "managed_heap_direct_closure_calls",
        mode: ExecutionMode::ManagedHeap,
        source: DIRECT_CLOSURE_CALLS_SOURCE,
    },
    Workload {
        name: "managed_heap_map_callbacks",
        mode: ExecutionMode::ManagedHeap,
        source: MAP_CALLBACKS_SOURCE,
    },
    Workload {
        name: "map_callbacks_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: MAP_CALLBACKS_SOURCE,
    },
    Workload {
        name: "managed_heap_map_find_entries",
        mode: ExecutionMode::ManagedHeap,
        source: MAP_FIND_ENTRIES_SOURCE,
    },
    Workload {
        name: "map_find_entries_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: MAP_FIND_ENTRIES_SOURCE,
    },
    Workload {
        name: "managed_heap_array_group_by",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_GROUP_BY_SOURCE,
    },
    Workload {
        name: "array_group_by_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_GROUP_BY_SOURCE,
    },
    Workload {
        name: "managed_heap_option_result_helpers",
        mode: ExecutionMode::ManagedHeap,
        source: OPTION_RESULT_HELPERS_SOURCE,
    },
    Workload {
        name: "option_result_helpers_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: OPTION_RESULT_HELPERS_SOURCE,
    },
    Workload {
        name: "managed_heap_set_lookup",
        mode: ExecutionMode::ManagedHeap,
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
        name: "map_extend_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: MAP_EXTEND_SOURCE,
    },
    Workload {
        name: "host_access",
        mode: ExecutionMode::HostAccess,
        source: r#"
fn main(player: Player) {
    player.level += 1;
    player.exp += 10;
    player.inventory.gold += 3;
    return player.level + player.exp + player.inventory.gold;
}
"#,
    },
    Workload {
        name: "host_field_read_write",
        mode: ExecutionMode::HostAccess,
        source: HOST_FIELD_READ_WRITE_SOURCE,
    },
    Workload {
        name: "host_field_read_write_cache_hot_offsets",
        mode: ExecutionMode::HostAccessCacheEnabled,
        source: HOST_FIELD_READ_WRITE_SOURCE,
    },
    Workload {
        name: "host_nested_read_write",
        mode: ExecutionMode::HostAccess,
        source: HOST_NESTED_READ_WRITE_SOURCE,
    },
    Workload {
        name: "host_nested_read_write_cache_hot_offsets",
        mode: ExecutionMode::HostAccessCacheEnabled,
        source: HOST_NESTED_READ_WRITE_SOURCE,
    },
    Workload {
        name: "host_rmw_mutation",
        mode: ExecutionMode::HostAccess,
        source: HOST_RMW_MUTATION_SOURCE,
    },
    Workload {
        name: "host_rmw_mutation_cache_hot_offsets",
        mode: ExecutionMode::HostAccessCacheEnabled,
        source: HOST_RMW_MUTATION_SOURCE,
    },
    Workload {
        name: "host_dynamic_key_access",
        mode: ExecutionMode::HostAccess,
        source: HOST_DYNAMIC_KEY_ACCESS_SOURCE,
    },
    Workload {
        name: "host_dynamic_key_access_cache_hot_offsets",
        mode: ExecutionMode::HostAccessCacheEnabled,
        source: HOST_DYNAMIC_KEY_ACCESS_SOURCE,
    },
    Workload {
        name: "host_method_calls",
        mode: ExecutionMode::HostAccess,
        source: HOST_METHOD_CALLS_SOURCE,
    },
    Workload {
        name: "host_method_calls_cache_hot_offsets",
        mode: ExecutionMode::HostAccessCacheEnabled,
        source: HOST_METHOD_CALLS_SOURCE,
    },
    Workload {
        name: "host_access_cache_hot_offsets",
        mode: ExecutionMode::HostAccessCacheEnabled,
        source: r#"
fn main(player: Player) {
    let total = 0;
    for tick in 0..48 {
        player.level = tick + 1;
        player.exp += tick;
        player.inventory.gold += 1;
        total += player.level + player.exp + player.inventory.gold;
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_host_conversion",
        mode: ExecutionMode::HostManagedHeapHostAccess,
        source: r#"
fn main(player: Player) {
    let total = 0;
    for tick in 0..24 {
        player.level = tick + 3;
        player.exp = tick + 1;
        player.inventory.gold = tick + 2;
        total += player.level + player.exp + player.inventory.gold;
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_host_read_conversion",
        mode: ExecutionMode::HostManagedHeapReadConversion,
        source: r#"
fn main(player: Player) {
    let total = 0;
    for tick in 0..48 {
        total += player.level + player.exp + player.inventory.gold + tick - tick;
    }
    return total;
}
"#,
    },
    Workload {
        name: "gameplay_monster_kill",
        mode: ExecutionMode::GameplayHost,
        source: include_str!(
            "../../../../examples/src/bin/monster_kill_reward/monster_kill_reward.vela"
        ),
    },
    Workload {
        name: "managed_heap_array_sum",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_SUM_SOURCE,
    },
    Workload {
        name: "array_sum_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_SUM_SOURCE,
    },
    Workload {
        name: "managed_heap_array_extrema",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_EXTREMA_SOURCE,
    },
    Workload {
        name: "array_extrema_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_EXTREMA_SOURCE,
    },
    Workload {
        name: "managed_heap_array_sort",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_SORT_SOURCE,
    },
    Workload {
        name: "array_sort_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_SORT_SOURCE,
    },
    Workload {
        name: "managed_heap_array_slice",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_SLICE_SOURCE,
    },
    Workload {
        name: "array_slice_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_SLICE_SOURCE,
    },
    Workload {
        name: "managed_heap_array_reverse",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_REVERSE_SOURCE,
    },
    Workload {
        name: "array_reverse_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_REVERSE_SOURCE,
    },
    Workload {
        name: "managed_heap_array_distinct",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_DISTINCT_SOURCE,
    },
    Workload {
        name: "array_distinct_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_DISTINCT_SOURCE,
    },
    Workload {
        name: "managed_heap_array_join",
        mode: ExecutionMode::ManagedHeap,
        source: ARRAY_JOIN_SOURCE,
    },
    Workload {
        name: "array_join_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: ARRAY_JOIN_SOURCE,
    },
    Workload {
        name: "managed_heap_materialization",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
struct Reward {
    item_id: string
    count: i64
}

enum ResultState {
    Done { score: i64 }
    Blocked(reason: string)
}

fn main() {
    let command = " reward:gold count=3 enabled=true ".trim();
    let parts = command.replace(":", " ").split_whitespace();
    let item = parts[1];
    let count = parts[2].strip_prefix("count=").unwrap_or("0").parse_int().unwrap_or(0);
    let reward = Reward { item_id: item, count };
    let outcome = ResultState::Done { score: reward.count + item.len() };
    let label = "quest.reward.done".strip_suffix(".done").unwrap_or("");
    match outcome {
        ResultState::Done { score } if label.starts_with("quest") => score + label.len(),
        ResultState::Blocked(reason) => reason.len(),
        _ => 0,
    }
}
"#,
    },
    Workload {
        name: "managed_heap_record_triplets",
        mode: ExecutionMode::ManagedHeap,
        source: RECORD_TRIPLETS_SOURCE,
    },
    Workload {
        name: "record_fields_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: RECORD_TRIPLETS_SOURCE,
    },
    Workload {
        name: "managed_heap_record_quads",
        mode: ExecutionMode::ManagedHeap,
        source: RECORD_QUADS_SOURCE,
    },
    Workload {
        name: "record_quads_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: RECORD_QUADS_SOURCE,
    },
    Workload {
        name: "managed_heap_record_quints",
        mode: ExecutionMode::ManagedHeap,
        source: RECORD_QUINTS_SOURCE,
    },
    Workload {
        name: "record_quints_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: RECORD_QUINTS_SOURCE,
    },
    Workload {
        name: "managed_heap_record_sextets",
        mode: ExecutionMode::ManagedHeap,
        source: RECORD_SEXTETS_SOURCE,
    },
    Workload {
        name: "record_sextets_cache_hot_offsets",
        mode: ExecutionMode::CacheEnabled,
        source: RECORD_SEXTETS_SOURCE,
    },
    Workload {
        name: "gc_pacing",
        mode: ExecutionMode::GcPacing,
        source: r#"
fn main() {
    let total = 0;
    for batch in 0..24 {
        let values = [];
        for item in 0..16 {
            values.push("gc".repeat((item % 4) + 1));
        }
        total += values.len() + batch;
    }
    return total;
}
"#,
    },
];
