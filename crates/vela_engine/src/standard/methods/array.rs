use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn array_method_descs() -> Vec<MethodDesc> {
    descs("Array", ARRAY_METHODS, "array")
}

const ARRAY_METHODS: &[MethodSpec] = &[
    MethodSpec::new("len", &[], "i64", "Returns the array length."),
    MethodSpec::new(
        "is_empty",
        &[],
        "bool",
        "Returns true when the array has no elements.",
    ),
    MethodSpec::new(
        "push",
        &[ParamSpec::new("value", "any")],
        "null",
        "Appends a value to the array.",
    ),
    MethodSpec::new("pop", &[], "Option", "Removes and returns the last value."),
    MethodSpec::new(
        "insert",
        &[
            ParamSpec::new("index", "i64"),
            ParamSpec::new("value", "any"),
        ],
        "null",
        "Inserts a value at an index.",
    ),
    MethodSpec::new(
        "extend",
        &[ParamSpec::new("values", "array")],
        "null",
        "Appends all values from another array.",
    ),
    MethodSpec::new("clear", &[], "null", "Removes all values."),
    MethodSpec::new("first", &[], "Option", "Returns the first value."),
    MethodSpec::new("last", &[], "Option", "Returns the last value."),
    MethodSpec::new(
        "remove_at",
        &[ParamSpec::new("index", "i64")],
        "Option",
        "Removes and returns the value at an index.",
    ),
    MethodSpec::new(
        "join",
        &[ParamSpec::new("separator", "string")],
        "string",
        "Joins values into a string with a separator.",
    ),
    MethodSpec::new(
        "contains",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Returns true when the array contains a value.",
    ),
    MethodSpec::new(
        "index_of",
        &[ParamSpec::new("value", "any")],
        "Option",
        "Returns the first index of a value, or Option::None.",
    ),
    MethodSpec::new("distinct", &[], "array", "Returns unique values."),
    MethodSpec::new("reverse", &[], "array", "Returns values in reverse order."),
    MethodSpec::new(
        "slice",
        &[ParamSpec::new("start", "i64"), ParamSpec::new("end", "i64")],
        "array",
        "Returns values in the index range.",
    ),
    MethodSpec::new(
        "map",
        &[ParamSpec::new("callback", "function")],
        "array",
        "Maps each value with a callback.",
    ),
    MethodSpec::new(
        "filter",
        &[ParamSpec::new("callback", "function")],
        "array",
        "Keeps values accepted by a callback.",
    ),
    MethodSpec::new(
        "find",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Returns the first callback match, or Option::None.",
    ),
    MethodSpec::new(
        "any",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when any value matches a callback.",
    ),
    MethodSpec::new(
        "all",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when all values match a callback.",
    ),
    MethodSpec::new(
        "count",
        &[ParamSpec::new("callback", "function")],
        "i64",
        "Counts values accepted by a callback.",
    ),
    MethodSpec::new(
        "sum",
        &[ParamSpec::optional("callback", "function")],
        "any",
        "Sums values or callback results.",
    ),
    MethodSpec::new(
        "group_by",
        &[ParamSpec::new("callback", "function")],
        "map",
        "Groups values by string callback keys.",
    ),
    MethodSpec::new("sort", &[], "array", "Returns sorted values."),
    MethodSpec::new("min", &[], "Option", "Returns the minimum value."),
    MethodSpec::new("max", &[], "Option", "Returns the maximum value."),
    MethodSpec::new(
        "sort_by",
        &[ParamSpec::new("callback", "function")],
        "array",
        "Returns values sorted by callback keys.",
    ),
    MethodSpec::new(
        "iter",
        &[],
        "iterator",
        "Returns a one-shot iterator over array values.",
    ),
    MethodSpec::new(
        "values",
        &[],
        "iterator",
        "Returns a one-shot iterator over array values.",
    ),
];
