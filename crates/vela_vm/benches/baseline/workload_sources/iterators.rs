pub(crate) const ITERATOR_STRING_CHARS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let count = 0;
        let bonus = 0;
        for ch in "vela奖励hot".chars() {
            count += 1;
            if ch == '奖' {
                bonus += 10;
            }
        }
        if count != 9 || bonus != 10 {
            return 0;
        }
        total += count + bonus + tick - tick;
    }
    return total;
}
"#;

pub(crate) const ITERATOR_STRING_BYTES_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let count = 0;
        let seen_v = 0;
        for byte in "Vela".bytes() {
            count += 1;
            if byte == 86u8 {
                seen_v += 1;
            }
        }
        if count != 4 || seen_v != 1 {
            return 0;
        }
        total += count + seen_v + tick - tick;
    }
    return total;
}
"#;

pub(crate) const ITERATOR_ARRAY_PIPELINE_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..80 {
        let values = [1, 2, 3, 4, 5, 6, 7, 8];
        let collected = values
            .iter()
            .filter(|value| value % 2 == 0)
            .map(|value| value * 3 + tick - tick)
            .collect_array();
        if collected.len() != 4 || collected[0] != 6 || collected[3] != 24 {
            return 0;
        }
        total += collected.sum();
    }
    return total;
}
"#;

pub(crate) const ITERATOR_ARRAY_SHORT_CIRCUIT_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let values = [tick + 1, tick + 2, tick + 3, tick + 4, tick + 5, tick + 6];
        let any_high = values.iter().any(|value| value >= tick + 5);
        let all_positive = values.iter().all(|value| value > tick);
        let found = values.iter().find(|value| value % 5 == 0).unwrap_or(0);
        if !any_high || !all_positive || found == 0 {
            return 0;
        }
        total += found + tick - tick;
    }
    return total;
}
"#;

pub(crate) const ITERATOR_MAP_VIEWS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let scores = {
            "daily": 3,
            "raid": 8,
            "boss": 13,
            "event": 5,
        };
        let key_len = 0;
        for key in scores.keys() {
            key_len += key.len();
        }
        let value_sum = 0;
        for value in scores.values() {
            value_sum += value;
        }
        let entry_total = 0;
        for entry in scores.entries() {
            entry_total += entry.key.len() + entry.value;
        }
        if key_len != 18 || value_sum != 29 || entry_total != 47 {
            return 0;
        }
        total += key_len + value_sum + entry_total + tick - tick;
    }
    return total;
}
"#;

pub(crate) const ITERATOR_RANGE_FAST_PATH_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for outer in 0..12 {
        for value in 0..128 {
            total += value + outer - outer;
        }
    }
    return total;
}
"#;

pub(crate) const ITERATOR_HOST_ITERABLE_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..96 {
        let sum = 0;
        for score in bench::scores() {
            sum += score;
        }
        let boosted = bench::scores()
            .filter(|score| score > 3)
            .map(|score| score + tick - tick + 1)
            .collect_array()
            .sum();
        if sum != 31 || boosted != 29 {
            return 0;
        }
        total += sum + boosted;
    }
    return total;
}
"#;
