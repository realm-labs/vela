window.VELA_PLAYGROUND_EXAMPLES = [
  {
    title: "Level reward",
    entry: "main",
    source: `struct Reward {
    enabled: Bool,
    base: Int,
    multiplier: Int,
}

fn score_reward(reward) {
    if !reward.enabled {
        return 0;
    }
    return reward.base * reward.multiplier;
}

fn main() {
    let reward = Reward {
        enabled: true,
        base: 12,
        multiplier: 3,
    };
    return score_reward(reward) + 4;
}`,
  },
  {
    title: "Collections",
    entry: "main",
    source: `fn main() {
    let rewards = { "gold": 10, "gem": 3, "xp": 25 };
    let tags = set::from_array(["daily", "vip", "daily"]);
    let total = rewards["gold"] + rewards["xp"];

    if tags.has("vip") && rewards.has("gem") {
        return total + tags.len();
    }
    return total;
}`,
  },
  {
    title: "Methods",
    entry: "main",
    source: `struct DamageResult {
    actor: String,
    applied: Int,
}

impl DamageResult {
    fn score(self, bonus) -> Int {
        return self.applied + bonus;
    }
}

fn main() {
    let result = DamageResult {
        actor: "knight",
        applied: 42,
    };
    return result.score(8);
}`,
  },
  {
    title: "Runtime error",
    entry: "main",
    source: `fn main() {
    let before = 10;
    return before / 0;
}`,
  },
];
