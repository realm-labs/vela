use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn set_method_descs() -> Vec<MethodDesc> {
    descs("Set", SET_METHODS, "set")
}

const SET_METHODS: &[MethodSpec] = &[
    MethodSpec::new("len", &[], "i64", "Returns the set length."),
    MethodSpec::new(
        "is_empty",
        &[],
        "bool",
        "Returns true when the set has no values.",
    ),
    MethodSpec::new(
        "has",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Returns true when a value exists.",
    ),
    MethodSpec::new(
        "add",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Adds a value and returns whether it was new.",
    ),
    MethodSpec::new(
        "remove",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Removes a value and returns whether it existed.",
    ),
    MethodSpec::new(
        "extend",
        &[ParamSpec::new("values", "set")],
        "null",
        "Adds all values from another set::",
    ),
    MethodSpec::new("clear", &[], "null", "Removes all values."),
    MethodSpec::new("values", &[], "array", "Returns set values."),
    MethodSpec::new(
        "map",
        &[ParamSpec::new("callback", "function")],
        "set",
        "Maps values with a callback.",
    ),
    MethodSpec::new(
        "filter",
        &[ParamSpec::new("callback", "function")],
        "set",
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
        "union",
        &[ParamSpec::new("other", "set")],
        "set",
        "Returns the union of two sets.",
    ),
    MethodSpec::new(
        "intersection",
        &[ParamSpec::new("other", "set")],
        "set",
        "Returns shared values between two sets.",
    ),
    MethodSpec::new(
        "difference",
        &[ParamSpec::new("other", "set")],
        "set",
        "Returns values missing from the other set::",
    ),
    MethodSpec::new(
        "symmetric_difference",
        &[ParamSpec::new("other", "set")],
        "set",
        "Returns values present in exactly one set::",
    ),
    MethodSpec::new(
        "is_subset",
        &[ParamSpec::new("other", "set")],
        "bool",
        "Returns true when all values exist in another set::",
    ),
    MethodSpec::new(
        "is_superset",
        &[ParamSpec::new("other", "set")],
        "bool",
        "Returns true when all other values exist in this set::",
    ),
    MethodSpec::new(
        "is_disjoint",
        &[ParamSpec::new("other", "set")],
        "bool",
        "Returns true when two sets share no values.",
    ),
];
