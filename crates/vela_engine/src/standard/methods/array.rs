use vela_common::HostMethodId;
use vela_reflect::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn array_method_descs() -> Vec<MethodDesc> {
    descs(ARRAY_METHODS, "array")
}

const ARRAY_METHODS: &[MethodSpec] = &[
    MethodSpec::new(array_id(0), "len", &[], "int", "Returns the array length."),
    MethodSpec::new(
        array_id(1),
        "is_empty",
        &[],
        "bool",
        "Returns true when the array has no elements.",
    ),
    MethodSpec::new(
        array_id(2),
        "push",
        &[ParamSpec::new("value", "any")],
        "null",
        "Appends a value to the array.",
    ),
    MethodSpec::new(
        array_id(3),
        "pop",
        &[],
        "Option",
        "Removes and returns the last value.",
    ),
    MethodSpec::new(
        array_id(4),
        "insert",
        &[
            ParamSpec::new("index", "int"),
            ParamSpec::new("value", "any"),
        ],
        "null",
        "Inserts a value at an index.",
    ),
    MethodSpec::new(
        array_id(5),
        "extend",
        &[ParamSpec::new("values", "array")],
        "null",
        "Appends all values from another array.",
    ),
    MethodSpec::new(array_id(6), "clear", &[], "null", "Removes all values."),
    MethodSpec::new(
        array_id(7),
        "first",
        &[],
        "Option",
        "Returns the first value.",
    ),
    MethodSpec::new(
        array_id(8),
        "last",
        &[],
        "Option",
        "Returns the last value.",
    ),
    MethodSpec::new(
        array_id(9),
        "remove_at",
        &[ParamSpec::new("index", "int")],
        "Option",
        "Removes and returns the value at an index.",
    ),
    MethodSpec::new(
        array_id(10),
        "join",
        &[ParamSpec::new("separator", "string")],
        "string",
        "Joins values into a string with a separator.",
    ),
    MethodSpec::new(
        array_id(11),
        "contains",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Returns true when the array contains a value.",
    ),
    MethodSpec::new(
        array_id(12),
        "index_of",
        &[ParamSpec::new("value", "any")],
        "Option",
        "Returns the first index of a value, or Option.None.",
    ),
    MethodSpec::new(
        array_id(13),
        "distinct",
        &[],
        "array",
        "Returns unique values.",
    ),
    MethodSpec::new(
        array_id(14),
        "reverse",
        &[],
        "array",
        "Returns values in reverse order.",
    ),
    MethodSpec::new(
        array_id(15),
        "slice",
        &[ParamSpec::new("start", "int"), ParamSpec::new("end", "int")],
        "array",
        "Returns values in the index range.",
    ),
    MethodSpec::new(
        array_id(16),
        "map",
        &[ParamSpec::new("callback", "function")],
        "array",
        "Maps each value with a callback.",
    ),
    MethodSpec::new(
        array_id(17),
        "filter",
        &[ParamSpec::new("callback", "function")],
        "array",
        "Keeps values accepted by a callback.",
    ),
    MethodSpec::new(
        array_id(18),
        "find",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Returns the first callback match, or Option.None.",
    ),
    MethodSpec::new(
        array_id(19),
        "any",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when any value matches a callback.",
    ),
    MethodSpec::new(
        array_id(20),
        "all",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when all values match a callback.",
    ),
    MethodSpec::new(
        array_id(21),
        "count",
        &[ParamSpec::new("callback", "function")],
        "int",
        "Counts values accepted by a callback.",
    ),
    MethodSpec::new(
        array_id(22),
        "sum",
        &[ParamSpec::optional("callback", "function")],
        "any",
        "Sums values or callback results.",
    ),
    MethodSpec::new(
        array_id(23),
        "group_by",
        &[ParamSpec::new("callback", "function")],
        "map",
        "Groups values by string callback keys.",
    ),
    MethodSpec::new(array_id(24), "sort", &[], "array", "Returns sorted values."),
    MethodSpec::new(
        array_id(25),
        "min",
        &[],
        "Option",
        "Returns the minimum value.",
    ),
    MethodSpec::new(
        array_id(26),
        "max",
        &[],
        "Option",
        "Returns the maximum value.",
    ),
    MethodSpec::new(
        array_id(27),
        "sort_by",
        &[ParamSpec::new("callback", "function")],
        "array",
        "Returns values sorted by callback keys.",
    ),
];

const fn array_id(offset: u32) -> HostMethodId {
    HostMethodId::new(0xff00_0800 + offset)
}
