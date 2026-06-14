pub(crate) const TYPED_CONTAINER_ARRAY_I64_PUSH_STATIC_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores: Array<i64> = [1, 2, 3];
        scores.push(tick);
        scores.push(5);
        scores.push(8);

        if scores.len() != 6 || scores[3] != tick || scores[5] != 8 {
            return 0;
        }

        total += scores.sum();
    }
    return total;
}
"#;

pub(crate) const TYPED_CONTAINER_ARRAY_I64_PUSH_DYNAMIC_GUARDED_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores: Array<i64> = [1, 2, 3];
        let tick_value: Any = tick;
        let bonus_value: Any = 5;
        let raid_value: Any = 8;
        scores.push(tick_value);
        scores.push(bonus_value);
        scores.push(raid_value);

        if scores.len() != 6 || scores[3] != tick || scores[5] != 8 {
            return 0;
        }

        total += scores.sum();
    }
    return total;
}
"#;

pub(crate) const TYPED_CONTAINER_ARRAY_PUSH_ERASED_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores: Array = [1, 2, 3];
        scores.push(tick);
        scores.push(5);
        scores.push(8);

        if scores.len() != 6 || scores[3] != tick || scores[5] != 8 {
            return 0;
        }

        total += scores.sum();
    }
    return total;
}
"#;

pub(crate) const TYPED_CONTAINER_MAP_STRING_I64_UPDATE_STATIC_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores: Map<String, i64> = {
            "base": 1,
            "bonus": 2,
        };
        scores.set("tick", tick);
        scores.set("bonus", 5);
        scores.set("raid", 8);

        if scores.get_or("tick", 0) != tick
            || scores.get_or("bonus", 0) != 5
            || scores.get_or("raid", 0) != 8
        {
            return 0;
        }

        total += scores.values().collect_array().sum();
    }
    return total;
}
"#;

pub(crate) const TYPED_CONTAINER_MAP_STRING_I64_UPDATE_DYNAMIC_GUARDED_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores: Map<String, i64> = {
            "base": 1,
            "bonus": 2,
        };
        let tick_key: Any = "tick";
        let tick_value: Any = tick;
        let bonus_key: Any = "bonus";
        let bonus_value: Any = 5;
        let raid_key: Any = "raid";
        let raid_value: Any = 8;
        scores.set(tick_key, tick_value);
        scores.set(bonus_key, bonus_value);
        scores.set(raid_key, raid_value);

        if scores.get_or("tick", 0) != tick
            || scores.get_or("bonus", 0) != 5
            || scores.get_or("raid", 0) != 8
        {
            return 0;
        }

        total += scores.values().collect_array().sum();
    }
    return total;
}
"#;

pub(crate) const TYPED_CONTAINER_MAP_UPDATE_ERASED_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores: Map = {
            "base": 1,
            "bonus": 2,
        };
        scores.set("tick", tick);
        scores.set("bonus", 5);
        scores.set("raid", 8);

        if scores.get_or("tick", 0) != tick
            || scores.get_or("bonus", 0) != 5
            || scores.get_or("raid", 0) != 8
        {
            return 0;
        }

        total += scores.values().collect_array().sum();
    }
    return total;
}
"#;
