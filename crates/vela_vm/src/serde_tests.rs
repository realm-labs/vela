use std::fmt;

use ::serde::de::{SeqAccess, Visitor};
use ::serde::{Deserialize, Deserializer, Serialize, Serializer};
use vela_common::ScalarValue;

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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExplicitBytes(Vec<u8>);

impl Serialize for ExplicitBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for ExplicitBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ExplicitBytesVisitor;

        impl<'de> Visitor<'de> for ExplicitBytesVisitor {
            type Value = ExplicitBytes;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an explicit byte buffer")
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: ::serde::de::Error,
            {
                Ok(ExplicitBytes(value.to_vec()))
            }

            fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
            where
                E: ::serde::de::Error,
            {
                Ok(ExplicitBytes(value))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut bytes = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    bytes.push(byte);
                }
                Ok(ExplicitBytes(bytes))
            }
        }

        deserializer.deserialize_byte_buf(ExplicitBytesVisitor)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct PrimitiveSerdeSnapshot {
    i8_value: i8,
    i16_value: i16,
    i32_value: i32,
    i64_value: i64,
    u8_value: u8,
    u16_value: u16,
    u32_value: u32,
    u64_value: u64,
    f32_value: f32,
    f64_value: f64,
    bytes: ExplicitBytes,
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

#[test]
fn serde_preserves_exact_scalar_tags_and_explicit_bytes() {
    let snapshot = PrimitiveSerdeSnapshot {
        i8_value: i8::MIN,
        i16_value: i16::MIN,
        i32_value: i32::MIN,
        i64_value: i64::MIN,
        u8_value: u8::MAX,
        u16_value: u16::MAX,
        u32_value: u32::MAX,
        u64_value: u64::MAX,
        f32_value: 1.5,
        f64_value: 2.5,
        bytes: ExplicitBytes(vec![0, 1, 255]),
    };

    let value = to_owned_value(&snapshot).expect("serialize primitive snapshot");
    let OwnedValue::Record { fields, .. } = &value else {
        panic!("primitive snapshot should serialize as a record");
    };

    assert_eq!(
        fields.get("i8_value"),
        Some(&OwnedValue::Scalar(ScalarValue::I8(i8::MIN)))
    );
    assert_eq!(
        fields.get("i16_value"),
        Some(&OwnedValue::Scalar(ScalarValue::I16(i16::MIN)))
    );
    assert_eq!(
        fields.get("i32_value"),
        Some(&OwnedValue::Scalar(ScalarValue::I32(i32::MIN)))
    );
    assert_eq!(
        fields.get("i64_value"),
        Some(&OwnedValue::Scalar(ScalarValue::I64(i64::MIN)))
    );
    assert_eq!(
        fields.get("u8_value"),
        Some(&OwnedValue::Scalar(ScalarValue::U8(u8::MAX)))
    );
    assert_eq!(
        fields.get("u16_value"),
        Some(&OwnedValue::Scalar(ScalarValue::U16(u16::MAX)))
    );
    assert_eq!(
        fields.get("u32_value"),
        Some(&OwnedValue::Scalar(ScalarValue::U32(u32::MAX)))
    );
    assert_eq!(
        fields.get("u64_value"),
        Some(&OwnedValue::Scalar(ScalarValue::U64(u64::MAX)))
    );
    assert_eq!(
        fields.get("f32_value"),
        Some(&OwnedValue::Scalar(ScalarValue::F32(1.5)))
    );
    assert_eq!(
        fields.get("f64_value"),
        Some(&OwnedValue::Scalar(ScalarValue::F64(2.5)))
    );
    assert_eq!(
        fields.get("bytes"),
        Some(&OwnedValue::Bytes(vec![0, 1, 255]))
    );

    let restored: PrimitiveSerdeSnapshot =
        from_owned_value(&value).expect("deserialize primitive snapshot");
    assert_eq!(restored, snapshot);
}

#[test]
fn serde_rejects_implicit_numeric_conversions() {
    assert!(from_owned_value::<u64>(&OwnedValue::Scalar(ScalarValue::I64(7))).is_err());
    assert!(from_owned_value::<i64>(&OwnedValue::Scalar(ScalarValue::U64(7))).is_err());
    assert!(from_owned_value::<f64>(&OwnedValue::Scalar(ScalarValue::F32(1.5))).is_err());
    assert!(from_owned_value::<f32>(&OwnedValue::Scalar(ScalarValue::F64(1.5))).is_err());
}

#[test]
fn json_policy_uses_byte_arrays_and_exact_u64_numbers() {
    let snapshot = PrimitiveSerdeSnapshot {
        i8_value: -8,
        i16_value: -16,
        i32_value: -32,
        i64_value: -64,
        u8_value: 8,
        u16_value: 16,
        u32_value: 32,
        u64_value: u64::MAX,
        f32_value: 1.25,
        f64_value: 2.5,
        bytes: ExplicitBytes(vec![0, 1, 255]),
    };

    let json = serde_json::to_string(&snapshot).expect("serialize json snapshot");
    assert!(json.contains("\"u64_value\":18446744073709551615"));
    assert!(json.contains("\"bytes\":[0,1,255]"));

    let restored: PrimitiveSerdeSnapshot =
        serde_json::from_str(&json).expect("deserialize json snapshot");
    assert_eq!(restored, snapshot);

    let value = to_owned_value(&u64::MAX).expect("serialize u64 max");
    assert_eq!(value, OwnedValue::Scalar(ScalarValue::U64(u64::MAX)));
    let restored = from_owned_value::<u64>(&value).expect("deserialize u64 max");
    assert_eq!(restored, u64::MAX);
}
