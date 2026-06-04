pub(crate) struct Workload {
    pub(crate) name: &'static str,
    pub(crate) mode: ExecutionMode,
    pub(crate) source: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) enum ExecutionMode {
    Inline,
    ManagedHeap,
    HostPatchTx,
    GameplayHost,
    GcPacing,
}

const CALLBACK_COLLECTIONS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..20 {
        let rewards = {
            "r01": 1, "r02": 2, "r03": 3, "r04": 4,
            "r05": 5, "r06": 6, "r07": 7, "r08": 8,
            "r09": 9, "r10": 10, "r11": 11, "r12": 12,
        };
        let keyed = rewards.map_values(|key, value| key.len() + value + tick - tick);
        let filtered = keyed.filter(|key, value| key.starts_with("r") && value % 3 == 0);
        let sorted = filtered.values().sort_by(|value| 20 - value);
        let tags = set::from_array(["daily", "quest", "raid", "bonus", "daily"]);
        let active = tags.filter(|tag| tag.contains("a") || tag.starts_with("q"));
        let lengths = active.map(|tag| tag.len());
        let found = active.find(|tag| tag.ends_with("d")).unwrap_or("");
        let tiers = [1, 2, 3, 4, 5, 6, 7, 8];
        let boosted = tiers.map(|tier| tier + tick - tick + 1);
        let even = boosted.filter(|tier| tier % 2 == 0);
        let first_high = boosted.find(|tier| tier > 6).unwrap_or(0);
        if filtered.len() != 4
            || sorted[0] != 15
            || sorted[3] != 6
            || active.len() != 3
            || lengths.len() != 2
            || found != "raid"
            || !active.any(|tag| tag == "quest")
            || !active.all(|tag| tag.len() >= 4)
            || active.count(|tag| tag.contains("i")) != 2
            || even.len() != 4
            || first_high != 7
            || !boosted.any(|tier| tier == 9)
            || !boosted.all(|tier| tier > 1)
            || boosted.count(|tier| tier >= 5) != 5
        {
            return 0;
        }
        total += sorted.sum() + keyed.get_or("r12", 0) + lengths.values().sum() + even.sum();
    }
    return total;
}
"#;

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
        name: "stdlib_collections",
        mode: ExecutionMode::Inline,
        source: r#"
fn main() {
    let values = [9, 2, 5, 2, 8, 1, 9, 3];
    let unique = values.distinct().sort();
    let grouped = values.group_by(|value| if value % 2 == 0 { "even" } else { "odd" });
    let scores = {"quest": 3, "raid": 8}.merge({"quest": 5, "daily": 2});
    let tags = set::from_array(["quest", "raid", "daily", "quest"]);
    if unique.first().unwrap_or(0) == 1
        && unique.last().unwrap_or(0) == 9
        && grouped.get_or("even", []).len() == 3
        && scores.get_or("quest", 0) == 5
        && tags.has("raid")
    {
        return values.sum() + unique.len() + tags.len();
    }
    return 0;
}
"#,
    },
    Workload {
        name: "callback_collections",
        mode: ExecutionMode::Inline,
        source: CALLBACK_COLLECTIONS_SOURCE,
    },
    Workload {
        name: "managed_heap_callback_collections",
        mode: ExecutionMode::ManagedHeap,
        source: CALLBACK_COLLECTIONS_SOURCE,
    },
    Workload {
        name: "host_patch_tx",
        mode: ExecutionMode::HostPatchTx,
        source: r#"
fn main(player) {
    player.level += 1;
    player.exp += 10;
    player.inventory.gold += 3;
    player.inventory.rewards.push("gold");
    return player.level + player.exp + player.inventory.gold + player.inventory.rewards.len();
}
"#,
    },
    Workload {
        name: "gameplay_monster_kill",
        mode: ExecutionMode::GameplayHost,
        source: include_str!(
            "../../../../examples/game_server_demo/scripts/monster_kill_reward.vela"
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
    count: int
}

enum ResultState {
    Done { score: int }
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
