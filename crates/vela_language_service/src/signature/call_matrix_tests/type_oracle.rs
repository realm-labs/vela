//! Decode independently authored structural facts without a production hint,
//! registry artifact, or signature parser.
use serde_json::Value;
use vela_analysis::type_fact::TypeFact;
use vela_common::{CollectionViewMutation, PrimitiveTag};

pub(super) fn expected(value: &Value) -> TypeFact {
    let name = || value["name"].as_str().expect("authored type name");
    let child = |key: &str| Box::new(expected(&value[key]));
    let list = |key: &str| {
        value[key]
            .as_array()
            .expect("authored type list")
            .iter()
            .map(expected)
            .collect()
    };
    match value["kind"].as_str().expect("authored type kind") {
        "unknown" => TypeFact::Unknown,
        "any" => TypeFact::Any,
        "range" => TypeFact::Range,
        "closure" => TypeFact::Closure,
        "primitive" => TypeFact::Primitive(match name() {
            "unit" => PrimitiveTag::Unit,
            "bool" => PrimitiveTag::Bool,
            "char" => PrimitiveTag::Char,
            "i8" => PrimitiveTag::I8,
            "i16" => PrimitiveTag::I16,
            "i32" => PrimitiveTag::I32,
            "i64" => PrimitiveTag::I64,
            "u8" => PrimitiveTag::U8,
            "u16" => PrimitiveTag::U16,
            "u32" => PrimitiveTag::U32,
            "u64" => PrimitiveTag::U64,
            "f32" => PrimitiveTag::F32,
            "f64" => PrimitiveTag::F64,
            "string" => PrimitiveTag::String,
            "bytes" => PrimitiveTag::Bytes,
            other => panic!("unreviewed primitive {other}"),
        }),
        "array" => TypeFact::Array {
            element: child("element"),
        },
        "arrayView" => TypeFact::ArrayView {
            element: child("element"),
        },
        "arrayMut" => TypeFact::ArrayMut {
            element: child("element"),
            mutation: CollectionViewMutation::Fixed,
        },
        "map" => TypeFact::Map {
            key: child("key"),
            value: child("value"),
        },
        "mapView" => TypeFact::MapView {
            key: child("key"),
            value: child("value"),
        },
        "mapMut" => TypeFact::MapMut {
            key: child("key"),
            value: child("value"),
            mutation: CollectionViewMutation::Growable,
        },
        "set" => TypeFact::Set {
            element: child("element"),
        },
        "setView" => TypeFact::SetView {
            element: child("element"),
        },
        "setMut" => TypeFact::SetMut {
            element: child("element"),
            mutation: CollectionViewMutation::Growable,
        },
        "iterator" => TypeFact::Iterator {
            item: child("item"),
        },
        "tuple" => TypeFact::Tuple {
            elements: list("elements"),
        },
        "option" => TypeFact::Option {
            some: child("some"),
        },
        "result" => TypeFact::Result {
            ok: child("ok"),
            err: child("err"),
        },
        "function" => TypeFact::Function {
            params: list("params"),
            returns: child("returns"),
        },
        "record" => TypeFact::Record {
            name: name().into(),
        },
        "host" => TypeFact::Host {
            name: name().into(),
        },
        "enum" => TypeFact::Enum {
            name: name().into(),
            variant: None,
        },
        "trait" => TypeFact::Trait {
            name: name().into(),
        },
        other => panic!("unreviewed structural fact {other}"),
    }
}
