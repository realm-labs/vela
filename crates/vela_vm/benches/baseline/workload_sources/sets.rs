pub(crate) const SET_CALLBACK_PREDICATES_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let values = set::from_array([tick + 1, tick + 2, tick + 3, tick + 4, tick + 5, tick + 6]);
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

pub(crate) const SET_VALUES_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let tags = set::from_array(["daily", "quest", "raid", "bonus", "event", "boss"]);
        let tiers = set::from_array([1, 2, 3, 5, 8, 13]);
        let tag_values = tags.values().sort();
        let tier_values = tiers.values().sort();
        if tag_values.join("|") != "bonus|boss|daily|event|quest|raid"
            || tier_values.sum() != 32
        {
            return 0;
        }
        total += tag_values.join("").len() + tier_values.sum() + tick - tick;
    }
    return total;
}
"#;

pub(crate) const SET_MUTATION_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let active = set::from_array(["quest", "raid"]);
        let added = active.add("event");
        let duplicate = active.add("quest");
        let removed = active.remove("raid");
        let missing = active.remove("missing");
        active.extend(set::from_array(["bonus", "boss"]));
        let before_clear = active.values().sort().join("|");
        active.clear();
        if !added || duplicate || !removed || missing
            || before_clear != "bonus|boss|event|quest"
            || !active.is_empty()
        {
            return 0;
        }
        total += before_clear.len() + active.len() + tick - tick;
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
