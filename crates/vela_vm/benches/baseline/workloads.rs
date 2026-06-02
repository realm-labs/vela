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
        name: "host_patch_tx",
        mode: ExecutionMode::HostPatchTx,
        source: r#"
fn main(player) {
    player.level += 1;
    player.exp += 10;
    player.inventory.gold += 3;
    return player.level + player.exp + player.inventory.gold;
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
