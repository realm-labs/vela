use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn map_method_descs() -> Vec<MethodDesc> {
    descs("Map", MAP_METHODS, "map")
}

const MAP_METHODS: &[MethodSpec] = &[
    MethodSpec::new("len", &[], "i64", "Returns the map length."),
    MethodSpec::new(
        "is_empty",
        &[],
        "bool",
        "Returns true when the map has no entries.",
    ),
    MethodSpec::new(
        "has",
        &[ParamSpec::new("key", "string")],
        "bool",
        "Returns true when a key exists.",
    ),
    MethodSpec::new(
        "get",
        &[ParamSpec::new("key", "string")],
        "Option",
        "Returns the value for a key, or Option::None.",
    ),
    MethodSpec::new(
        "get_or",
        &[
            ParamSpec::new("key", "string"),
            ParamSpec::new("default", "any"),
        ],
        "any",
        "Returns the value for a key or a default.",
    ),
    MethodSpec::new(
        "set",
        &[
            ParamSpec::new("key", "string"),
            ParamSpec::new("value", "any"),
        ],
        "any",
        "Sets and returns a value for a key.",
    ),
    MethodSpec::new(
        "remove",
        &[ParamSpec::new("key", "string")],
        "Option",
        "Removes and returns the value for a key.",
    ),
    MethodSpec::new(
        "extend",
        &[ParamSpec::new("values", "map")],
        "null",
        "Inserts entries from another map.",
    ),
    MethodSpec::new("clear", &[], "null", "Removes all entries."),
    MethodSpec::new("keys", &[], "iterator", "Returns keys in sorted order."),
    MethodSpec::new("values", &[], "iterator", "Returns values in key order."),
    MethodSpec::new("entries", &[], "iterator", "Returns key/value records."),
    MethodSpec::new(
        "merge",
        &[ParamSpec::new("other", "map")],
        "map",
        "Returns a merged map.",
    ),
    MethodSpec::new(
        "map_values",
        &[ParamSpec::new("callback", "function")],
        "map",
        "Maps values with a callback.",
    ),
    MethodSpec::new(
        "filter",
        &[ParamSpec::new("callback", "function")],
        "map",
        "Keeps entries accepted by a callback.",
    ),
    MethodSpec::new(
        "find",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Returns the first matching entry, or Option::None.",
    ),
    MethodSpec::new(
        "any",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when any entry matches a callback.",
    ),
    MethodSpec::new(
        "all",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when all entries match a callback.",
    ),
    MethodSpec::new(
        "count",
        &[ParamSpec::new("callback", "function")],
        "i64",
        "Counts entries accepted by a callback.",
    ),
    MethodSpec::new(
        "iter",
        &[],
        "iterator",
        "Returns a one-shot iterator over map values in key order.",
    ),
];
