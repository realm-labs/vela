#[path = "workload_sources/calls.rs"]
mod calls;

#[path = "workload_sources/strings.rs"]
mod strings;

#[path = "workload_sources/sets.rs"]
mod sets;

pub(crate) use calls::{
    DIRECT_CLOSURE_CALLS_SOURCE, METHOD_DISPATCH_SOURCE, NATIVE_CALL_WIDE_ARGS_SOURCE,
    SCRIPT_CALL_SMALL_ARGS_SOURCE, SCRIPT_CALL_WIDE_ARGS_SOURCE, SCRIPT_METHOD_DISPATCH_SOURCE,
    TRAIT_METHOD_DISPATCH_SOURCE,
};
pub(crate) use sets::{
    SET_CALLBACK_PREDICATES_SOURCE, SET_COMBINATION_SOURCE, SET_LOOKUP_SOURCE, SET_MUTATION_SOURCE,
    SET_VALUES_SOURCE,
};
pub(crate) use strings::{
    STRING_METHODS_SOURCE, STRING_OPTIONS_SOURCE, STRING_PARSING_SOURCE, STRING_SPLITTING_SOURCE,
};

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

pub(crate) const HOST_FIELD_READ_WRITE_SOURCE: &str = r#"
fn main(player: Player) {
    let total = 0;
    for tick in 0..32 {
        player.level = tick + 1;
        total += player.level;
    }
    return total;
}
"#;

pub(crate) const HOST_NESTED_READ_WRITE_SOURCE: &str = r#"
fn main(player: Player) {
    let total = 0;
    for tick in 0..32 {
        player.inventory.gold = tick + 3;
        total += player.inventory.gold;
    }
    return total;
}
"#;

pub(crate) const HOST_RMW_MUTATION_SOURCE: &str = r#"
fn main(player: Player) {
    for tick in 0..32 {
        player.level += 1;
        player.exp += tick;
    }
    return player.level + player.exp;
}
"#;

pub(crate) const HOST_DYNAMIC_KEY_ACCESS_SOURCE: &str = r#"
fn main(player: Player) {
    let item_id = "gold";
    let total = 0;
    for tick in 0..32 {
        player.inventory.items[item_id].count += 1;
        total += player.inventory.items[item_id].count + tick - tick;
    }
    return total;
}
"#;

pub(crate) const HOST_METHOD_CALLS_SOURCE: &str = r#"
fn main(player: Player) {
    for tick in 0..32 {
        player.add_reward("gold", tick + 1);
    }
    return player.level;
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

pub(crate) const RECORD_QUADS_SOURCE: &str = r#"
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
"#;

pub(crate) const RECORD_QUINTS_SOURCE: &str = r#"
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
"#;

pub(crate) const RECORD_SEXTETS_SOURCE: &str = r#"
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

pub(crate) const ARRAY_CALLBACK_PREDICATES_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let values = [tick + 1, tick + 2, tick + 3, tick + 4, tick + 5, tick + 6];
        let found = values.find(|value| value % 5 == 0).unwrap_or(0);

        if found == 0
            || !values.any(|value| value == found)
            || !values.all(|value| value > tick)
            || values.count(|value| value % 2 == 0) != 3
        {
            return 0;
        }

        total += found + values.count(|value| value >= tick + 4) + tick - tick;
    }
    return total;
}
"#;

pub(crate) const MAP_CALLBACKS_SOURCE: &str = r#"
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
"#;

pub(crate) const MAP_FIND_ENTRIES_SOURCE: &str = r#"
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
"#;

pub(crate) const ARRAY_GROUP_BY_SOURCE: &str = r#"
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
"#;

pub(crate) const ARRAY_SUM_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..64 {
        let base = [1, 2, 3, 4, 5, 6, 7, 8];
        let scaled = [tick, tick + 1, tick + 2, tick + 3];
        total += base.sum() + scaled.sum();
    }
    return total;
}
"#;

pub(crate) const BYTES_METHODS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let data = b"\x01\x02\x03\x04\xff";
        let middle = data.slice(1, 4);
        let decoded = result::unwrap_or(bytes::from_hex("00ff"), b"bad");
        let hex = data.to_hex();

        if data.get(4) != 255u8
            || data.read_u32_le(0) != 0x04030201u32
            || data.read_u32_be(0) != 0x01020304u32
            || middle != b"\x02\x03\x04"
            || decoded != b"\x00\xff"
            || hex != "01020304ff"
        {
            return 0;
        }

        total += 10 + tick - tick;
    }
    return total;
}
"#;

pub(crate) const BYTES_ACCESS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let data = b"\x01\x02\x03\x04\xff\x10";

        if data.len() != 6
            || data.is_empty()
            || data.get(0) != 1u8
            || data.get(4) != 255u8
            || data.get(5) != 16u8
            || data.read_u32_le(0) != 0x04030201u32
            || data.read_u32_be(0) != 0x01020304u32
        {
            return 0;
        }

        total += data.len() + 16 + tick - tick;
    }
    return total;
}
"#;

pub(crate) const BYTES_MATERIALIZATION_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let data = b"\x0a\x0b\x0c\x0d\x0e\x0f";
        let head = data.slice(0, 3);
        let tail = data.slice(3, 6);
        let encoded = data.to_hex();
        let decoded = result::unwrap_or(bytes::from_hex(encoded), b"bad");

        if head != b"\x0a\x0b\x0c"
            || tail != b"\x0d\x0e\x0f"
            || encoded != "0a0b0c0d0e0f"
            || decoded != data
        {
            return 0;
        }

        total += 6 + tick - tick;
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

pub(crate) const ARRAY_EDGE_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let tags = ["daily", "quest", "raid", "bonus", "event", "boss"];
        let tiers = [1, 2, 3, 5, 8, 13];
        let empty = [];
        if option::unwrap_or(tags.first(), "") != "daily"
            || option::unwrap_or(tags.last(), "") != "boss"
            || option::unwrap_or(tiers.first(), 0) != 1
            || option::unwrap_or(tiers.last(), 0) != 13
            || !option::is_none(empty.first())
            || !option::is_none(empty.last())
        {
            return 0;
        }
        total += tags.first().unwrap_or("").len()
            + tags.last().unwrap_or("").len()
            + tiers.first().unwrap_or(0)
            + tiers.last().unwrap_or(0)
            + tick - tick;
    }
    return total;
}
"#;

pub(crate) const ARRAY_EXTEND_SOURCE: &str = r#"
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
"#;

pub(crate) const ARRAY_MUTATION_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let tags = ["daily", "quest"];
        tags.push("raid");
        tags.insert(1, "bonus");
        let removed = tags.remove_at(2);
        let popped = tags.pop();
        tags.extend(["event", "boss"]);
        let label = tags.join("|");
        tags.clear();

        let scores = [1, 2, 3];
        scores.push(5);
        scores.insert(1, 8);
        let removed_score = scores.remove_at(2);
        let popped_score = scores.pop();
        scores.extend([13, 21]);
        let score_total = scores.sum();
        scores.clear();

        if option::unwrap_or(removed, "") != "quest"
            || option::unwrap_or(popped, "") != "raid"
            || label != "daily|bonus|event|boss"
            || !tags.is_empty()
            || option::unwrap_or(removed_score, 0) != 2
            || option::unwrap_or(popped_score, 0) != 5
            || score_total != 46
            || !scores.is_empty()
        {
            return 0;
        }
        total += label.len() + score_total + tags.len() + scores.len() + tick - tick;
    }
    return total;
}
"#;

pub(crate) const ARRAY_EXTREMA_SOURCE: &str = r#"
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
"#;

pub(crate) const ARRAY_SORT_SOURCE: &str = r#"
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
"#;

pub(crate) const ARRAY_SLICE_SOURCE: &str = r#"
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
"#;

pub(crate) const ARRAY_REVERSE_SOURCE: &str = r#"
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
"#;

pub(crate) const ARRAY_DISTINCT_SOURCE: &str = r#"
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
"#;

pub(crate) const ARRAY_JOIN_SOURCE: &str = r#"
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
"#;

pub(crate) const MAP_LOOKUP_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let states = {
            "daily": "done",
            "raid": "active",
            "boss": "ready",
            "event": "open",
        };
        let scores = {
            "daily": 3,
            "raid": 8,
            "boss": 13,
            "event": 5,
        };
        if !states.has("raid")
            || states.has("missing")
            || option::unwrap_or(states.get("boss"), "") != "ready"
            || states.get_or("missing", "fallback") != "fallback"
            || !scores.has("boss")
            || scores.get_or("raid", 0) != 8
            || option::unwrap_or(scores.get("missing"), -1) != -1
        {
            return 0;
        }
        total += states.len() + scores.get_or("daily", 0) + tick - tick;
    }
    return total;
}
"#;

pub(crate) const MAP_VIEWS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores = {
            "daily": 3,
            "raid": 8,
            "boss": 13,
            "event": 5,
        };
        let keys = scores.keys();
        let values = scores.values();
        let entries = scores.entries();
        let entry_total = entries[0].key.len()
            + entries[0].value
            + entries[1].key.len()
            + entries[1].value
            + entries[2].key.len()
            + entries[2].value
            + entries[3].key.len()
            + entries[3].value;
        total += keys.join("|").len() + values.sum() + entry_total + tick - tick;
    }
    return total;
}
"#;

pub(crate) const MAP_MUTATION_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores = {
            "daily": 3,
            "raid": 8,
        };
        scores.set("boss", 13);
        scores.set("raid", 21);
        let removed = scores.remove("daily");
        let missing = scores.remove("missing");
        scores.extend({"event": 5, "bonus": 34});
        let keys = scores.keys().sort().join("|");
        let values = scores.values().sum();
        scores.clear();
        if option::unwrap_or(removed, 0) != 3
            || !option::is_none(missing)
            || keys != "bonus|boss|event|raid"
            || values != 73
            || !scores.is_empty()
        {
            return 0;
        }
        total += keys.len() + values + scores.len() + tick - tick;
    }
    return total;
}
"#;

pub(crate) const MAP_MERGE_SOURCE: &str = r#"
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
"#;

pub(crate) const MAP_EXTEND_SOURCE: &str = r#"
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
"#;

pub(crate) const OPTION_RESULT_HELPERS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..72 {
        let some = option::some(["quest", "done"]);
        let none = option::none();
        let ok = result::ok(["done"]);
        let err = result::err(["blocked"]);
        let converted_ok = some.ok_or(["missing"]);
        let converted_err = none.ok_or(["missing"]);
        let flattened_some = option::some(option::some(["quest", "done"])).flatten();
        let flattened_none = option::some(option::none()).flatten();
        let flattened_ok = result::ok(result::ok(["done"])).flatten();
        let flattened_err = result::ok(result::err(["nested"])).flatten();

        if !some.is_some()
            || !none.is_none()
            || !ok.is_ok()
            || !err.is_err()
            || !converted_ok.is_ok()
            || !converted_err.is_err()
            || some.unwrap_or([]).join(".") != "quest.done"
            || none.unwrap_or(["fallback"]).join(".") != "fallback"
            || ok.unwrap_or([]).join(".") != "done"
            || err.unwrap_or(["fallback"]).join(".") != "fallback"
            || converted_ok.to_option().unwrap_or([]).join(".") != "quest.done"
            || converted_err.to_option().unwrap_or(["fallback"]).join(".") != "fallback"
            || !converted_ok.to_error_option().is_none()
            || converted_err.to_error_option().unwrap_or(["fallback"]).join(".") != "missing"
            || flattened_some.unwrap_or([]).join(".") != "quest.done"
            || !flattened_none.is_none()
            || flattened_ok.unwrap_or([]).join(".") != "done"
            || flattened_err.to_error_option().unwrap_or([]).join(".") != "nested"
        {
            return 0;
        }

        total += tick + some.unwrap_or([]).len() + ok.unwrap_or([]).len();
    }
    return total;
}
"#;

pub(crate) const OPTION_RESULT_PREDICATES_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let some = option::some(["quest", "done"]);
        let none = option::none();
        let ok = result::ok(["done"]);
        let err = result::err(["blocked"]);

        if !some.is_some()
            || some.is_none()
            || !none.is_none()
            || !ok.is_ok()
            || ok.is_err()
            || err.is_ok()
            || !err.is_err()
        {
            return 0;
        }

        total += some.unwrap_or([]).len()
            + none.unwrap_or(["fallback"]).len()
            + ok.unwrap_or([]).len()
            + err.unwrap_or(["fallback"]).len()
            + tick - tick;
    }
    return total;
}
"#;

pub(crate) const OPTION_RESULT_CONVERSIONS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..72 {
        let some = option::some(["quest", "done"]);
        let none = option::none();
        let converted_ok = some.ok_or(["missing"]);
        let converted_err = none.ok_or(["missing"]);
        let flattened_some = option::some(option::some(["quest", "done"])).flatten();
        let flattened_none = option::some(option::none()).flatten();
        let flattened_ok = result::ok(result::ok(["done"])).flatten();
        let flattened_err = result::ok(result::err(["nested"])).flatten();

        if !converted_ok.is_ok()
            || !converted_err.is_err()
            || converted_ok.to_option().unwrap_or([]).join(".") != "quest.done"
            || converted_err.to_option().unwrap_or(["fallback"]).join(".") != "fallback"
            || !converted_ok.to_error_option().is_none()
            || converted_err.to_error_option().unwrap_or(["fallback"]).join(".") != "missing"
            || flattened_some.unwrap_or([]).join(".") != "quest.done"
            || !flattened_none.is_none()
            || flattened_ok.unwrap_or([]).join(".") != "done"
            || flattened_err.to_error_option().unwrap_or([]).join(".") != "nested"
        {
            return 0;
        }

        total += tick + flattened_some.unwrap_or([]).len() + flattened_ok.unwrap_or([]).len();
    }
    return total;
}
"#;

pub(crate) const OPTION_RESULT_CALLBACKS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let option_chain = option::some("quest")
            .map(|value| value.to_upper())
            .filter(|value| value.starts_with("Q"));
        let option_fallback = option::none().or_else(| | option::some("fallback"));
        let result_chain = result::ok(["gold", "xp"])
            .and_then(|values| result::ok(values.join(".")));
        let mapped_err = result::err(["bad", "level"]).map_err(|errors| errors.join("."));
        let recovered = result::err("missing").or_else(|error| result::ok("fallback"));

        if option::unwrap_or(option_chain, "") != "QUEST"
            || option::unwrap_or(option_fallback, "") != "fallback"
            || result::unwrap_or(result_chain, "") != "gold.xp"
            || option::unwrap_or(mapped_err.to_error_option(), "") != "bad.level"
            || result::unwrap_or(recovered, "") != "fallback"
        {
            return 0;
        }

        total += 37 + tick - tick;
    }
    return total;
}
"#;
