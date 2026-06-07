use ::serde::{Deserialize, Serialize};

use crate::owned_value::OwnedValue;
use crate::serde::{from_owned_value, to_owned_value};

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PlayerSnapshot {
    id: i64,
    level: i64,
    tags: Vec<String>,
    stats: PlayerStats,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PlayerStats {
    gold: i64,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Reward {
    Gold { amount: i64 },
    None,
}

#[test]
fn serde_struct_round_trips_as_record() {
    let snapshot = PlayerSnapshot {
        id: 7,
        level: 3,
        tags: vec!["vip".to_owned(), "daily".to_owned()],
        stats: PlayerStats { gold: 12 },
    };

    let value = to_owned_value(&snapshot).expect("serialize snapshot");
    assert!(matches!(
        value,
        OwnedValue::Record {
            ref type_name,
            ..
        } if type_name == "PlayerSnapshot"
    ));

    let restored: PlayerSnapshot = from_owned_value(&value).expect("deserialize snapshot");
    assert_eq!(restored, snapshot);
}

#[test]
fn serde_enum_round_trips_as_enum_value() {
    let reward = Reward::Gold { amount: 9 };

    let value = to_owned_value(&reward).expect("serialize reward");
    assert!(matches!(
        value,
        OwnedValue::Enum {
            ref enum_name,
            ref variant,
            ..
        } if enum_name == "Reward" && variant == "Gold"
    ));

    let restored: Reward = from_owned_value(&value).expect("deserialize reward");
    assert_eq!(restored, reward);
}
