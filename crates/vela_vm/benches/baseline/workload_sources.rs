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

pub(crate) const STRING_METHODS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..80 {
        let label = "quest.done";
        let upper = label.to_upper();
        let replaced = label.replace(".", "_");
        let repeated = "ab".repeat(3);
        let parts = "alpha,beta".split(",");
        let pair = "count=3".split_once("=").unwrap_or([]);
        let lines = "alpha\nbeta".split_lines();
        let words = "alpha beta".split_whitespace();
        let sliced = "hello".slice(1, 4);
        let ch = "quest".char_at(1).unwrap_or("");
        let found = "daily_quest".find("quest").unwrap_or(-1);
        let stripped_prefix = "event:quest".strip_prefix("event:").unwrap_or("");
        let stripped_suffix = "quest.done".strip_suffix(".done").unwrap_or("");
        let parsed = "42".parse_int().unwrap_or(0);
        let parsed_bool = "true".parse_bool().unwrap_or(false);

        if upper != "QUEST.DONE"
            || replaced != "quest_done"
            || repeated != "ababab"
            || parts.len() != 2
            || pair.len() != 2
            || lines.len() != 2
            || words.len() != 2
            || sliced != "ell"
            || ch != "u"
            || found != 6
            || stripped_prefix != "quest"
            || stripped_suffix != "quest"
            || parsed != 42
            || !parsed_bool
        {
            return 0;
        }

        total += upper.len()
            + replaced.len()
            + repeated.len()
            + parts.join("").len()
            + pair.join("").len()
            + lines.join("").len()
            + words.join("").len()
            + sliced.len()
            + ch.len()
            + found
            + parsed
            + tick - tick;
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

pub(crate) const SET_LOOKUP_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let tags = set::from_array(["daily", "quest", "raid", "bonus", "event", "boss"]);
        let tiers = set::from_array([1, 2, 3, 5, 8, 13]);
        if !tags.has("raid") || tags.has("missing") || !tiers.has(8) || tiers.has(tick + 20) {
            return 0;
        }
        total += tags.len() + tiers.len() + tick - tick;
    }
    return total;
}
"#;

pub(crate) const SET_COMBINATION_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let base = set::from_array(["daily", "quest", "raid", "event", "boss"]);
        let active = set::from_array(["quest", "event", "bonus"]);
        let required = set::from_array(["daily", "quest"]);
        let excluded = set::from_array(["missing", "locked"]);

        let unioned = base.union(active);
        let shared = base.intersection(active);
        let only_base = base.difference(active);
        let changed = base.symmetric_difference(active);

        if !required.is_subset(base)
            || !base.is_superset(required)
            || !base.is_disjoint(excluded)
            || unioned.len() != 6
            || shared.len() != 2
            || only_base.len() != 3
            || changed.len() != 4
        {
            return 0;
        }

        total += unioned.len() + shared.len() + only_base.len() + changed.len() + tick - tick;
    }
    return total;
}
"#;
