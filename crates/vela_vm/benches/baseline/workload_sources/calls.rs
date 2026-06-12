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

pub(crate) const SCRIPT_CALL_WIDE_ARGS_SOURCE: &str = r#"
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

pub(crate) const SCRIPT_METHOD_DISPATCH_SOURCE: &str = r#"
struct Account {
    balance: i64,
    tier: i64,
}

impl Account {
    fn score(self, bonus) -> i64 {
        return self.balance + self.tier + bonus;
    }

    fn boosted(self, bonus, multiplier) -> i64 {
        return self.score(bonus) * multiplier;
    }
}

fn main() {
    let total = 0;
    for tick in 0..96 {
        let account = Account {
            balance: tick + 10,
            tier: tick % 5,
        };
        total += account.score(3) + account.boosted(1, 2);
    }
    return total;
}
"#;

pub(crate) const TRAIT_METHOD_DISPATCH_SOURCE: &str = r#"
trait AccountScoring {
    fn score(self, bonus) -> i64;
    fn label(self) -> string { return self.name; }
    fn boosted(self, bonus, multiplier) -> i64 {
        return self.score(bonus) * multiplier;
    }
}

struct Account {
    balance: i64,
    tier: i64,
    name: string,
}

impl AccountScoring for Account {
    fn score(self, bonus) -> i64 {
        return self.balance + self.tier + bonus;
    }
}

fn main() {
    let total = 0;
    for tick in 0..80 {
        let account = Account {
            balance: tick + 10,
            tier: tick % 7,
            name: "primary",
        };
        if account.label() != "primary" {
            return 0;
        }
        total += account.score(2) + account.boosted(1, 3);
    }
    return total;
}
"#;
