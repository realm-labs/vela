use vela_common::HostMethodId;
use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn map_method_descs() -> Vec<MethodDesc> {
    descs(MAP_METHODS, "map")
}

const MAP_METHODS: &[MethodSpec] = &[
    MethodSpec::new(map_id(0), "len", &[], "int", "Returns the map length."),
    MethodSpec::new(
        map_id(1),
        "is_empty",
        &[],
        "bool",
        "Returns true when the map has no entries.",
    ),
    MethodSpec::new(
        map_id(2),
        "has",
        &[ParamSpec::new("key", "string")],
        "bool",
        "Returns true when a key exists.",
    ),
    MethodSpec::new(
        map_id(3),
        "get",
        &[ParamSpec::new("key", "string")],
        "Option",
        "Returns the value for a key, or Option.None.",
    ),
    MethodSpec::new(
        map_id(4),
        "get_or",
        &[
            ParamSpec::new("key", "string"),
            ParamSpec::new("default", "any"),
        ],
        "any",
        "Returns the value for a key or a default.",
    ),
    MethodSpec::new(
        map_id(5),
        "set",
        &[
            ParamSpec::new("key", "string"),
            ParamSpec::new("value", "any"),
        ],
        "any",
        "Sets and returns a value for a key.",
    ),
    MethodSpec::new(
        map_id(6),
        "remove",
        &[ParamSpec::new("key", "string")],
        "Option",
        "Removes and returns the value for a key.",
    ),
    MethodSpec::new(
        map_id(7),
        "extend",
        &[ParamSpec::new("values", "map")],
        "null",
        "Inserts entries from another map.",
    ),
    MethodSpec::new(map_id(8), "clear", &[], "null", "Removes all entries."),
    MethodSpec::new(
        map_id(9),
        "keys",
        &[],
        "array",
        "Returns keys in sorted order.",
    ),
    MethodSpec::new(
        map_id(10),
        "values",
        &[],
        "array",
        "Returns values in key order.",
    ),
    MethodSpec::new(
        map_id(11),
        "entries",
        &[],
        "array",
        "Returns key/value records.",
    ),
    MethodSpec::new(
        map_id(12),
        "merge",
        &[ParamSpec::new("other", "map")],
        "map",
        "Returns a merged map.",
    ),
    MethodSpec::new(
        map_id(13),
        "map_values",
        &[ParamSpec::new("callback", "function")],
        "map",
        "Maps values with a callback.",
    ),
    MethodSpec::new(
        map_id(14),
        "filter",
        &[ParamSpec::new("callback", "function")],
        "map",
        "Keeps entries accepted by a callback.",
    ),
    MethodSpec::new(
        map_id(15),
        "find",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Returns the first matching entry, or Option.None.",
    ),
    MethodSpec::new(
        map_id(16),
        "any",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when any entry matches a callback.",
    ),
    MethodSpec::new(
        map_id(17),
        "all",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when all entries match a callback.",
    ),
    MethodSpec::new(
        map_id(18),
        "count",
        &[ParamSpec::new("callback", "function")],
        "int",
        "Counts entries accepted by a callback.",
    ),
];

const fn map_id(offset: u64) -> HostMethodId {
    HostMethodId::new(0xff00_0900 + offset)
}
