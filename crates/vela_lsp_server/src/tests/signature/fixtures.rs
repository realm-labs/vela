pub(super) fn schema_with_rewardable_function_and_trait_method() -> &'static str {
    r#"{
        "formatVersion": 1,
        "facts": {
            "traits": [
                {
                    "name": "Rewardable",
                    "fact": { "kind": "trait", "name": "Rewardable" }
                }
            ],
            "functions": [
                {
                    "name": "current_reward",
                    "fact": {
                        "kind": "function",
                        "params": [],
                        "returns": { "kind": "trait", "name": "Rewardable" }
                    }
                }
            ],
            "traitMethods": [
                {
                    "owner": "Rewardable",
                    "name": "preview",
                    "fact": {
                        "kind": "function",
                        "params": [
                            { "kind": "primitive", "name": "i64" },
                            { "kind": "primitive", "name": "i64" }
                        ],
                        "returns": { "kind": "primitive", "name": "bool" }
                    }
                }
            ]
        }
    }"#
}

pub(super) fn line(text: &str, line: usize) -> &str {
    text.lines().nth(line).expect("line should exist")
}
