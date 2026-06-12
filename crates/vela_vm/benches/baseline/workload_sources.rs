pub(crate) const STDLIB_COLLECTIONS_SOURCE: &str = r#"
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
"#;

pub(crate) const RECORD_TRIPLETS_SOURCE: &str = r#"
struct Reward {
    item_id: string,
    count: i64,
    bonus: i64,
}

enum ResultState {
    Scored { item_id: string, count: i64, bonus: i64 }
}

fn main() {
    let total = 0;
    for tick in 0..96 {
        let gold = Reward { item_id: "gold", count: tick + 1, bonus: tick % 7 };
        let gold_state = ResultState::Scored {
            item_id: gold.item_id,
            count: gold.count,
            bonus: gold.bonus,
        };
        match gold_state {
            ResultState::Scored { item_id, count, bonus } => {
                total += item_id.len() + count + bonus;
            }
        }

        let xp = Reward { item_id: "xp", count: tick + 2, bonus: tick % 5 };
        let xp_state = ResultState::Scored {
            item_id: xp.item_id,
            count: xp.count,
            bonus: xp.bonus,
        };
        match xp_state {
            ResultState::Scored { item_id, count, bonus } => {
                total += item_id.len() + count + bonus;
            }
        }
    }
    return total;
}
"#;

pub(crate) const CALLBACK_COLLECTIONS_SOURCE: &str = r#"
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

pub(crate) const DIRECT_CLOSURE_CALLS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..180 {
        let add = |value| value + tick;
        let mix = |left, right| left * 3 + right + tick - tick;
        total += add(tick);
        total += add(total % 17);
        total += mix(tick, total % 23);
    }
    return total;
}
"#;

pub(crate) const SCRIPT_CALL_SMALL_ARGS_SOURCE: &str = r#"
fn add_one(value) {
    return value + 1;
}

fn mix_pair(left, right) {
    return left * 3 + right;
}

fn main() {
    let total = 0;
    for tick in 0..240 {
        total += add_one(tick);
        total += mix_pair(tick, total % 17);
    }
    return total;
}
"#;

pub(crate) const NATIVE_CALL_WIDE_ARGS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..240 {
        total += bench::mix4(tick, total % 17, tick % 5, 3);
    }
    return total;
}
"#;

pub(crate) const METHOD_DISPATCH_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let tags = ["daily", "quest", "raid", "bonus", "event", "boss"];
        let scores = {
            "daily": 3,
            "raid": 8,
            "boss": 13,
            "event": 5,
        };
        if tags.contains("raid")
            && scores.has("boss")
            && tags.any(|tag| tag.starts_with("q"))
            && scores.get_or("missing", tick - tick) == 0
        {
            total += tags.len() + scores.get_or("daily", 0);
        }
    }
    return total;
}
"#;

pub(crate) const ARRAY_LOOKUP_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let tags = ["daily", "quest", "raid", "bonus", "event", "boss"];
        let tiers = [1, 2, 3, 5, 8, 13];
        if !tags.contains("raid")
            || tags.contains("missing")
            || option::unwrap_or(tags.index_of("boss"), -1) != 5
            || !tiers.contains(8)
            || tiers.contains(tick + 20)
            || option::unwrap_or(tiers.index_of(13), -1) != 5
        {
            return 0;
        }
        total += tags.len() + tiers.len() + tick - tick;
    }
    return total;
}
"#;
