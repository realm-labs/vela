use crate::logical_records::{LogicalRecordKind, map_entry};
use crate::stdlib::StdlibMethodFact;
use crate::type_fact::TypeFact;
use vela_common::{CollectionViewMutation, PrimitiveTag};

mod collections;
mod option_result;

use collections::{
    array_method_fact, bytes_method_fact, char_method_fact, iterator_method_fact, map_method_fact,
    range_method_fact, set_method_fact, string_method_fact,
};
use option_result::{OptionShape, ResultShape, option_method_fact, result_method_fact};

const ARRAY_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "get",
    "push",
    "pop",
    "insert",
    "extend",
    "clear",
    "retain",
    "first",
    "last",
    "remove_at",
    "join",
    "contains",
    "index_of",
    "distinct",
    "reverse",
    "slice",
    "map",
    "filter",
    "find",
    "any",
    "all",
    "count",
    "sum",
    "group_by",
    "sort",
    "min",
    "max",
    "sort_by",
    "values",
    "iter",
];
const MAP_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "has",
    "contains_key",
    "get",
    "get_or",
    "get_or_insert",
    "set",
    "insert",
    "remove",
    "extend",
    "clear",
    "retain",
    "keys",
    "values",
    "entries",
    "merge",
    "map_values",
    "filter",
    "find",
    "any",
    "all",
    "count",
    "iter",
];
const SET_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "has",
    "contains",
    "add",
    "insert",
    "remove",
    "extend",
    "clear",
    "retain",
    "values",
    "map",
    "filter",
    "find",
    "any",
    "all",
    "count",
    "union",
    "intersection",
    "difference",
    "symmetric_difference",
    "is_subset",
    "is_superset",
    "is_disjoint",
    "iter",
];
const STRING_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "contains",
    "find",
    "starts_with",
    "ends_with",
    "strip_prefix",
    "strip_suffix",
    "to_upper",
    "to_lower",
    "trim",
    "trim_start",
    "trim_end",
    "replace",
    "repeat",
    "slice",
    "split",
    "split_once",
    "split_lines",
    "split_whitespace",
    "parse_i8",
    "parse_i16",
    "parse_i32",
    "parse_i64",
    "parse_u8",
    "parse_u16",
    "parse_u32",
    "parse_u64",
    "parse_f32",
    "parse_f64",
    "parse_bool",
    "parse_char",
    "chars",
    "bytes",
];
const BYTES_METHOD_NAMES: &[&str] = &[
    "len",
    "is_empty",
    "slice",
    "get",
    "read_u32_le",
    "read_u32_be",
    "to_hex",
    "iter",
    "values",
];
const CHAR_METHOD_NAMES: &[&str] = &["to_string", "is_whitespace", "is_ascii", "is_ascii_digit"];
const RANGE_METHOD_NAMES: &[&str] = &["len", "is_empty", "iter"];
const ITERATOR_METHOD_NAMES: &[&str] = &[
    "next",
    "count",
    "any",
    "all",
    "find",
    "map",
    "filter",
    "take",
    "skip",
    "collect_array",
    "collect_set",
    "collect_map",
];
const OPTION_METHOD_NAMES: &[&str] = &[
    "is_some",
    "is_none",
    "unwrap_or",
    "ok_or",
    "flatten",
    "map",
    "and_then",
    "or_else",
    "filter",
];
const RESULT_METHOD_NAMES: &[&str] = &[
    "is_ok",
    "is_err",
    "unwrap_or",
    "to_option",
    "to_error_option",
    "flatten",
    "map",
    "map_err",
    "and_then",
    "or_else",
];

pub(super) fn method_fact(
    receiver: &TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
    lambda_param_count: Option<usize>,
    arguments: Option<&[TypeFact]>,
) -> Option<StdlibMethodFact> {
    match receiver {
        TypeFact::Array { element }
        | TypeFact::ArrayView { element }
        | TypeFact::ArrayMut { element, .. } => specialize_collection_source(
            method,
            arguments,
            bind_collection_receiver(
                receiver,
                method,
                array_method_fact((**element).clone(), method, lambda_return),
            ),
        ),
        TypeFact::Map { key, value }
        | TypeFact::MapView { key, value }
        | TypeFact::MapMut { key, value, .. } => specialize_collection_source(
            method,
            arguments,
            bind_collection_receiver(
                receiver,
                method,
                map_method_fact(
                    (**key).clone(),
                    (**value).clone(),
                    method,
                    lambda_return,
                    lambda_param_count,
                ),
            ),
        ),
        TypeFact::Set { element }
        | TypeFact::SetView { element }
        | TypeFact::SetMut { element, .. } => specialize_collection_source(
            method,
            arguments,
            bind_collection_receiver(
                receiver,
                method,
                set_method_fact((**element).clone(), method, lambda_return),
            ),
        ),
        TypeFact::Iterator { item } => {
            iterator_method_fact((**item).clone(), method, lambda_return)
        }
        TypeFact::Primitive(PrimitiveTag::String) => string_method_fact(method),
        TypeFact::Primitive(PrimitiveTag::Bytes) => bytes_method_fact(method),
        TypeFact::Primitive(PrimitiveTag::Char) => char_method_fact(method),
        TypeFact::Range => range_method_fact(method),
        TypeFact::Option { some } => option_method_fact(
            (**some).clone(),
            OptionShape::Maybe,
            method,
            lambda_return,
            arguments,
        ),
        TypeFact::OptionSome { some } => option_method_fact(
            (**some).clone(),
            OptionShape::Some,
            method,
            lambda_return,
            arguments,
        ),
        TypeFact::OptionNone => option_method_fact(
            TypeFact::Never,
            OptionShape::None,
            method,
            lambda_return,
            arguments,
        ),
        TypeFact::Result { ok, err } => result_method_fact(
            (**ok).clone(),
            (**err).clone(),
            ResultShape::Maybe,
            method,
            lambda_return,
            arguments,
        ),
        TypeFact::ResultOk { ok } => result_method_fact(
            (**ok).clone(),
            TypeFact::Any,
            ResultShape::Ok,
            method,
            lambda_return,
            arguments,
        ),
        TypeFact::ResultErr { err } => result_method_fact(
            TypeFact::Never,
            (**err).clone(),
            ResultShape::Err,
            method,
            lambda_return,
            arguments,
        ),
        _ => None,
    }
}

pub(super) fn method_facts(
    receiver: &TypeFact,
    lambda_return: Option<&TypeFact>,
) -> Vec<StdlibMethodFact> {
    method_names(receiver)
        .iter()
        .filter_map(|method| method_fact(receiver, method, lambda_return, None, None))
        .collect()
}

fn method_names(receiver: &TypeFact) -> &'static [&'static str] {
    match receiver {
        TypeFact::Array { .. } | TypeFact::ArrayView { .. } | TypeFact::ArrayMut { .. } => {
            ARRAY_METHOD_NAMES
        }
        TypeFact::Map { .. } | TypeFact::MapView { .. } | TypeFact::MapMut { .. } => {
            MAP_METHOD_NAMES
        }
        TypeFact::Set { .. } | TypeFact::SetView { .. } | TypeFact::SetMut { .. } => {
            SET_METHOD_NAMES
        }
        TypeFact::Iterator { .. } => ITERATOR_METHOD_NAMES,
        TypeFact::Primitive(PrimitiveTag::String) => STRING_METHOD_NAMES,
        TypeFact::Primitive(PrimitiveTag::Bytes) => BYTES_METHOD_NAMES,
        TypeFact::Primitive(PrimitiveTag::Char) => CHAR_METHOD_NAMES,
        TypeFact::Range => RANGE_METHOD_NAMES,
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            OPTION_METHOD_NAMES
        }
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            RESULT_METHOD_NAMES
        }
        _ => &[],
    }
}

fn bind_collection_receiver(
    receiver: &TypeFact,
    method: &str,
    fact: Option<StdlibMethodFact>,
) -> Option<StdlibMethodFact> {
    if !collection_method_allowed(receiver, method) {
        return None;
    }
    fact.map(|mut fact| {
        fact.receiver = receiver.clone();
        fact
    })
}

fn specialize_collection_source(
    method: &str,
    arguments: Option<&[TypeFact]>,
    mut fact: Option<StdlibMethodFact>,
) -> Option<StdlibMethodFact> {
    if method != "extend" {
        return fact;
    }
    let (Some(method_fact), Some(actual)) =
        (fact.as_mut(), arguments.and_then(|args| args.first()))
    else {
        return fact;
    };
    let Some(expected) = method_fact.params.first_mut() else {
        return fact;
    };
    if same_collection_shape(actual, expected) {
        *expected = actual.clone();
    }
    fact
}

fn same_collection_shape(actual: &TypeFact, expected: &TypeFact) -> bool {
    match (actual, expected) {
        (
            TypeFact::Array { element: actual }
            | TypeFact::ArrayView { element: actual }
            | TypeFact::ArrayMut {
                element: actual, ..
            },
            TypeFact::Array { element: expected },
        )
        | (
            TypeFact::Set { element: actual }
            | TypeFact::SetView { element: actual }
            | TypeFact::SetMut {
                element: actual, ..
            },
            TypeFact::Set { element: expected },
        ) => collection_component_matches(actual, expected),
        (
            TypeFact::Map {
                key: actual_key,
                value: actual_value,
            }
            | TypeFact::MapView {
                key: actual_key,
                value: actual_value,
            }
            | TypeFact::MapMut {
                key: actual_key,
                value: actual_value,
                ..
            },
            TypeFact::Map {
                key: expected_key,
                value: expected_value,
            },
        ) => {
            collection_component_matches(actual_key, expected_key)
                && collection_component_matches(actual_value, expected_value)
        }
        _ => false,
    }
}

fn collection_component_matches(actual: &TypeFact, expected: &TypeFact) -> bool {
    actual == expected || matches!(actual, TypeFact::Never) || matches!(expected, TypeFact::Any)
}

fn collection_method_allowed(receiver: &TypeFact, method: &str) -> bool {
    match receiver {
        TypeFact::Array { .. } | TypeFact::Map { .. } | TypeFact::Set { .. } => true,
        TypeFact::ArrayView { .. } => !matches!(
            method,
            "push" | "pop" | "insert" | "extend" | "clear" | "retain" | "remove_at"
        ),
        TypeFact::ArrayMut {
            mutation: CollectionViewMutation::Fixed,
            ..
        } => !matches!(
            method,
            "push" | "pop" | "insert" | "extend" | "clear" | "retain" | "remove_at"
        ),
        TypeFact::ArrayMut {
            mutation: CollectionViewMutation::Growable,
            ..
        } => true,
        TypeFact::MapView { .. } => !matches!(
            method,
            "get_or_insert" | "set" | "insert" | "remove" | "extend" | "clear" | "retain"
        ),
        TypeFact::MapMut {
            mutation: CollectionViewMutation::Fixed,
            ..
        } => !matches!(
            method,
            "get_or_insert" | "set" | "insert" | "remove" | "extend" | "clear" | "retain"
        ),
        TypeFact::MapMut {
            mutation: CollectionViewMutation::Growable,
            ..
        } => true,
        TypeFact::SetView { .. } => !matches!(
            method,
            "add" | "insert" | "remove" | "extend" | "clear" | "retain"
        ),
        TypeFact::SetMut {
            mutation: CollectionViewMutation::Fixed,
            ..
        } => !matches!(
            method,
            "add" | "insert" | "remove" | "extend" | "clear" | "retain"
        ),
        TypeFact::SetMut {
            mutation: CollectionViewMutation::Growable,
            ..
        } => true,
        _ => false,
    }
}

fn value_or_fallback(value: TypeFact, fallback: TypeFact) -> TypeFact {
    merge_value_facts(value, fallback)
}

fn merge_value_facts(value: TypeFact, fallback: TypeFact) -> TypeFact {
    if value == fallback || matches!(fallback, TypeFact::Unknown | TypeFact::Never) {
        return value;
    }
    if matches!(value, TypeFact::Unknown | TypeFact::Never) {
        return fallback;
    }
    match (value, fallback) {
        (TypeFact::Array { element }, TypeFact::Array { element: fallback }) => {
            TypeFact::array(merge_value_facts(*element, *fallback))
        }
        (
            TypeFact::Map { key, value },
            TypeFact::Map {
                key: fallback_key,
                value: fallback_value,
            },
        ) => TypeFact::map(
            merge_value_facts(*key, *fallback_key),
            merge_value_facts(*value, *fallback_value),
        ),
        (TypeFact::Set { element }, TypeFact::Set { element: fallback }) => {
            TypeFact::set(merge_value_facts(*element, *fallback))
        }
        (TypeFact::LogicalRecord(value), TypeFact::LogicalRecord(fallback))
            if value.kind() == LogicalRecordKind::MapEntry
                && fallback.kind() == LogicalRecordKind::MapEntry =>
        {
            let key = merge_value_facts(
                value
                    .field("key")
                    .expect("MapEntry manifest has key")
                    .fact()
                    .clone(),
                fallback
                    .field("key")
                    .expect("MapEntry manifest has key")
                    .fact()
                    .clone(),
            );
            let value = merge_value_facts(
                value
                    .field("value")
                    .expect("MapEntry manifest has value")
                    .fact()
                    .clone(),
                fallback
                    .field("value")
                    .expect("MapEntry manifest has value")
                    .fact()
                    .clone(),
            );
            map_entry(key, value)
        }
        (value, fallback) => TypeFact::union([value, fallback]),
    }
}

fn numeric_return(value: &TypeFact) -> TypeFact {
    match value {
        TypeFact::Primitive(PrimitiveTag::F64) => TypeFact::F64,
        TypeFact::Primitive(PrimitiveTag::I64) => TypeFact::I64,
        _ => TypeFact::Union(vec![TypeFact::I64, TypeFact::F64]),
    }
}
