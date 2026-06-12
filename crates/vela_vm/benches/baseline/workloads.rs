use crate::workload_sources::{
    ARRAY_LOOKUP_SOURCE, CALLBACK_COLLECTIONS_SOURCE, DIRECT_CLOSURE_CALLS_SOURCE,
    MAP_LOOKUP_SOURCE, METHOD_DISPATCH_SOURCE, NATIVE_CALL_WIDE_ARGS_SOURCE,
    OPTION_RESULT_HELPERS_SOURCE, RECORD_TRIPLETS_SOURCE, SCRIPT_CALL_SMALL_ARGS_SOURCE,
    SET_COMBINATION_SOURCE, SET_LOOKUP_SOURCE, STDLIB_COLLECTIONS_SOURCE,
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
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..48 {
        let rewards = {
            "r01": 1, "r02": 2, "r03": 3, "r04": 4,
            "r05": 5, "r06": 6, "r07": 7, "r08": 8,
            "r09": 9, "r10": 10, "r11": 11, "r12": 12,
        };
        let keyed = rewards.map_values(|key, value| key.len() + value + tick - tick);
        let filtered = keyed.filter(|key, value| key.starts_with("r") && value % 3 == 0);
        if filtered.len() != 4 || filtered.get_or("r12", 0) != 15 {
            return 0;
        }
        total += keyed.values().sum() + filtered.values().sum();
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_map_find_entries",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..72 {
        let rewards = {
            "r01": 1, "r02": 2, "r03": 3, "r04": 4,
            "r05": 5, "r06": 6, "r07": 7, "r08": 8,
            "r09": 9, "r10": 10, "r11": 11, "r12": 12,
        };
        let found = rewards.find(|key, value| key == "r08" && value == 8 + tick - tick);
        let missing = rewards.find(|key, value| key == "missing" && value > 0);
        let entry = option::unwrap_or(found, MapEntry { key: "", value: 0 });
        if entry.key != "r08" || entry.value != 8 || !option::is_none(missing) {
            return 0;
        }
        total += entry.key.len() + entry.value;
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_array_group_by",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..64 {
        let names = ["boar", "bat", "wolf", "wyrm", "bear", "wasp", "boss", "wisp"];
        let groups = names.group_by(|name| if name.starts_with("w") { "w" } else { "b" });
        if groups.len() != 2
            || groups["w"].len() != 4
            || groups["b"].len() != 4
            || groups["w"][0] != "wolf"
            || groups["w"][3] != "wisp"
            || groups["b"][1] != "bat"
        {
            return 0;
        }
        total += groups["w"].join("").len() + groups["b"].join("").len() + tick - tick;
    }
    return total;
}
"#,
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
        name: "managed_heap_array_extend",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let tags = ["daily", "quest"];
        tags.extend(["raid", "event", "boss"]);
        tags.extend(["bonus"]);

        let scores = [1, 2, 3];
        scores.extend([5, 8, 13]);
        scores.extend([]);

        if tags.len() != 6
            || tags[0] != "daily"
            || tags[5] != "bonus"
            || tags.join("|") != "daily|quest|raid|event|boss|bonus"
            || scores.len() != 6
            || scores[5] != 13
            || scores.sum() != 32
        {
            return 0;
        }
        total += tags.len() + scores.sum() + tick - tick;
    }
    return total;
}
"#,
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
        name: "managed_heap_map_merge",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let base = {
            "daily": 3,
            "raid": 8,
            "boss": 13,
            "event": 5,
        };
        let patch = {
            "raid": 21,
            "bonus": 34,
            "season": 55,
        };
        let merged = base.merge(patch);
        if merged.len() != 6
            || merged["daily"] != 3
            || merged["raid"] != 21
            || merged["bonus"] != 34
            || merged["season"] != 55
        {
            return 0;
        }
        total += merged.len() + merged["raid"] + tick - tick;
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_map_extend",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores = {
            "daily": 3,
            "raid": 8,
        };
        let patch = {
            "raid": 21,
            "boss": 13,
            "event": 5,
        };
        scores.extend(patch);
        scores.extend({"bonus": 34});

        if scores.len() != 5
            || scores["daily"] != 3
            || scores["raid"] != 21
            || scores["event"] != 5
            || scores["bonus"] != 34
        {
            return 0;
        }
        total += scores.len() + scores["raid"] + tick - tick;
    }
    return total;
}
"#,
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
        source: r#"
fn main(player: Player) {
    let total = 0;
    for tick in 0..32 {
        player.level = tick + 1;
        total += player.level;
    }
    return total;
}
"#,
    },
    Workload {
        name: "host_nested_read_write",
        mode: ExecutionMode::HostAccess,
        source: r#"
fn main(player: Player) {
    let total = 0;
    for tick in 0..32 {
        player.inventory.gold = tick + 3;
        total += player.inventory.gold;
    }
    return total;
}
"#,
    },
    Workload {
        name: "host_rmw_mutation",
        mode: ExecutionMode::HostAccess,
        source: r#"
fn main(player: Player) {
    for tick in 0..32 {
        player.level += 1;
        player.exp += tick;
    }
    return player.level + player.exp;
}
"#,
    },
    Workload {
        name: "host_dynamic_key_access",
        mode: ExecutionMode::HostAccess,
        source: r#"
fn main(player: Player) {
    let item_id = "gold";
    let total = 0;
    for tick in 0..32 {
        player.inventory.items[item_id].count += 1;
        total += player.inventory.items[item_id].count + tick - tick;
    }
    return total;
}
"#,
    },
    Workload {
        name: "host_method_calls",
        mode: ExecutionMode::HostAccess,
        source: r#"
fn main(player: Player) {
    for tick in 0..32 {
        player.add_reward("gold", tick + 1);
    }
    return player.level;
}
"#,
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
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..64 {
        let base = [1, 2, 3, 4, 5, 6, 7, 8];
        let scaled = [tick, tick + 1, tick + 2, tick + 3];
        total += base.sum() + scaled.sum();
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_array_extrema",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..64 {
        let base = [9, 2, 5, 2, 8, 1, 9, 3];
        let scaled = [tick + 4, tick + 1, tick + 8, tick + 2];
        total += base.min().unwrap_or(0)
            + base.max().unwrap_or(0)
            + scaled.min().unwrap_or(0)
            + scaled.max().unwrap_or(0);
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_array_sort",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..48 {
        let base = [9, 2, 5, 2, 8, 1, 9, 3];
        let scaled = [tick + 4, tick + 1, tick + 8, tick + 2];
        let sorted = base.sort();
        let scaled_sorted = scaled.sort();
        total += sorted[0] + sorted[7] + scaled_sorted[0] + scaled_sorted[3];
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_array_slice",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..64 {
        let values = [
            tick, tick + 1, tick + 2, tick + 3,
            tick + 4, tick + 5, tick + 6, tick + 7,
            tick + 8, tick + 9, tick + 10, tick + 11,
        ];
        let middle = values.slice(3, 7);
        let tail = values.slice(8, 12);
        total += middle.sum() + tail.sum();
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_array_reverse",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..64 {
        let values = [
            tick, tick + 1, tick + 2, tick + 3,
            tick + 4, tick + 5, tick + 6, tick + 7,
        ];
        let labels = ["daily", "quest", "raid", "bonus"];
        let reversed = values.reverse();
        let reversed_labels = labels.reverse();
        total += reversed[0] + reversed[7] + reversed_labels.join("|").len();
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_array_distinct",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..48 {
        let values = [
            tick, tick + 1, tick, tick + 2,
            tick + 1, tick + 3, tick + 2, tick + 4,
        ];
        let tags = ["raid", "quest", "raid", "daily", "quest", "bonus"];
        let nested = [["daily", "quest"], ["daily", "quest"], ["raid"], ["raid"]];
        let unique = values.distinct();
        let unique_tags = tags.distinct();
        let unique_nested = nested.distinct();
        total += unique.sum() + unique_tags.join("|").len() + unique_nested.len();
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_array_join",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
fn main() {
    let total = 0;
    for tick in 0..64 {
        let tags = ["daily", "quest", "raid", "bonus", "boss", "event"];
        let route = ["zone", "shard", "tick", "phase"];
        let label = tags.join("|");
        let path = route.join(".");
        total += label.len() + path.len() + tick - tick;
    }
    return total;
}
"#,
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
        source: r#"
struct Reward {
    item_id: string,
    count: i64,
    bonus: i64,
    rarity: i64,
}

enum ResultState {
    Scored { item_id: string, count: i64, bonus: i64, rarity: i64 }
}

fn main() {
    let total = 0;
    for tick in 0..80 {
        let gold = Reward {
            item_id: "gold",
            count: tick + 1,
            bonus: tick % 7,
            rarity: 3,
        };
        let gold_state = ResultState::Scored {
            item_id: gold.item_id,
            count: gold.count,
            bonus: gold.bonus,
            rarity: gold.rarity,
        };
        match gold_state {
            ResultState::Scored { item_id, count, bonus, rarity } => {
                total += item_id.len() + count + bonus + rarity;
            }
        }

        let xp = Reward {
            item_id: "xp",
            count: tick + 2,
            bonus: tick % 5,
            rarity: 1,
        };
        let xp_state = ResultState::Scored {
            item_id: xp.item_id,
            count: xp.count,
            bonus: xp.bonus,
            rarity: xp.rarity,
        };
        match xp_state {
            ResultState::Scored { item_id, count, bonus, rarity } => {
                total += item_id.len() + count + bonus + rarity;
            }
        }
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_record_quints",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
struct Reward {
    item_id: string,
    count: i64,
    bonus: i64,
    rarity: i64,
    quality: i64,
}

enum ResultState {
    Scored { item_id: string, count: i64, bonus: i64, rarity: i64, quality: i64 }
}

fn main() {
    let total = 0;
    for tick in 0..72 {
        let gold = Reward {
            item_id: "gold",
            count: tick + 1,
            bonus: tick % 7,
            rarity: 3,
            quality: tick % 11,
        };
        let gold_state = ResultState::Scored {
            item_id: gold.item_id,
            count: gold.count,
            bonus: gold.bonus,
            rarity: gold.rarity,
            quality: gold.quality,
        };
        match gold_state {
            ResultState::Scored { item_id, count, bonus, rarity, quality } => {
                total += item_id.len() + count + bonus + rarity + quality;
            }
        }

        let xp = Reward {
            item_id: "xp",
            count: tick + 2,
            bonus: tick % 5,
            rarity: 1,
            quality: tick % 13,
        };
        let xp_state = ResultState::Scored {
            item_id: xp.item_id,
            count: xp.count,
            bonus: xp.bonus,
            rarity: xp.rarity,
            quality: xp.quality,
        };
        match xp_state {
            ResultState::Scored { item_id, count, bonus, rarity, quality } => {
                total += item_id.len() + count + bonus + rarity + quality;
            }
        }
    }
    return total;
}
"#,
    },
    Workload {
        name: "managed_heap_record_sextets",
        mode: ExecutionMode::ManagedHeap,
        source: r#"
struct Reward {
    item_id: string,
    count: i64,
    bonus: i64,
    rarity: i64,
    quality: i64,
    weight: i64,
}

enum ResultState {
    Scored {
        item_id: string,
        count: i64,
        bonus: i64,
        rarity: i64,
        quality: i64,
        weight: i64,
    }
}

fn main() {
    let total = 0;
    for tick in 0..64 {
        let gold = Reward {
            item_id: "gold",
            count: tick + 1,
            bonus: tick % 7,
            rarity: 3,
            quality: tick % 11,
            weight: 2,
        };
        let gold_state = ResultState::Scored {
            item_id: gold.item_id,
            count: gold.count,
            bonus: gold.bonus,
            rarity: gold.rarity,
            quality: gold.quality,
            weight: gold.weight,
        };
        match gold_state {
            ResultState::Scored { item_id, count, bonus, rarity, quality, weight } => {
                total += item_id.len() + count + bonus + rarity + quality + weight;
            }
        }

        let xp = Reward {
            item_id: "xp",
            count: tick + 2,
            bonus: tick % 5,
            rarity: 1,
            quality: tick % 13,
            weight: 1,
        };
        let xp_state = ResultState::Scored {
            item_id: xp.item_id,
            count: xp.count,
            bonus: xp.bonus,
            rarity: xp.rarity,
            quality: xp.quality,
            weight: xp.weight,
        };
        match xp_state {
            ResultState::Scored { item_id, count, bonus, rarity, quality, weight } => {
                total += item_id.len() + count + bonus + rarity + quality + weight;
            }
        }
    }
    return total;
}
"#,
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
