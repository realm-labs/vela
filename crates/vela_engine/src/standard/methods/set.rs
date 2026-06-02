use vela_common::HostMethodId;
use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn set_method_descs() -> Vec<MethodDesc> {
    descs(SET_METHODS, "set")
}

const SET_METHODS: &[MethodSpec] = &[
    MethodSpec::new(set_id(0), "len", &[], "int", "Returns the set length."),
    MethodSpec::new(
        set_id(1),
        "is_empty",
        &[],
        "bool",
        "Returns true when the set has no values.",
    ),
    MethodSpec::new(
        set_id(2),
        "has",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Returns true when a value exists.",
    ),
    MethodSpec::new(
        set_id(3),
        "add",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Adds a value and returns whether it was new.",
    ),
    MethodSpec::new(
        set_id(4),
        "remove",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Removes a value and returns whether it existed.",
    ),
    MethodSpec::new(
        set_id(5),
        "extend",
        &[ParamSpec::new("values", "set")],
        "null",
        "Adds all values from another set.",
    ),
    MethodSpec::new(set_id(6), "clear", &[], "null", "Removes all values."),
    MethodSpec::new(set_id(7), "values", &[], "array", "Returns set values."),
    MethodSpec::new(
        set_id(8),
        "map",
        &[ParamSpec::new("callback", "function")],
        "set",
        "Maps values with a callback.",
    ),
    MethodSpec::new(
        set_id(9),
        "filter",
        &[ParamSpec::new("callback", "function")],
        "set",
        "Keeps values accepted by a callback.",
    ),
    MethodSpec::new(
        set_id(10),
        "find",
        &[ParamSpec::new("callback", "function")],
        "Option",
        "Returns the first callback match, or Option.None.",
    ),
    MethodSpec::new(
        set_id(11),
        "any",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when any value matches a callback.",
    ),
    MethodSpec::new(
        set_id(12),
        "all",
        &[ParamSpec::new("callback", "function")],
        "bool",
        "Returns true when all values match a callback.",
    ),
    MethodSpec::new(
        set_id(13),
        "count",
        &[ParamSpec::new("callback", "function")],
        "int",
        "Counts values accepted by a callback.",
    ),
    MethodSpec::new(
        set_id(14),
        "union",
        &[ParamSpec::new("other", "set")],
        "set",
        "Returns the union of two sets.",
    ),
    MethodSpec::new(
        set_id(15),
        "intersection",
        &[ParamSpec::new("other", "set")],
        "set",
        "Returns shared values between two sets.",
    ),
    MethodSpec::new(
        set_id(16),
        "difference",
        &[ParamSpec::new("other", "set")],
        "set",
        "Returns values missing from the other set.",
    ),
    MethodSpec::new(
        set_id(17),
        "symmetric_difference",
        &[ParamSpec::new("other", "set")],
        "set",
        "Returns values present in exactly one set.",
    ),
    MethodSpec::new(
        set_id(18),
        "is_subset",
        &[ParamSpec::new("other", "set")],
        "bool",
        "Returns true when all values exist in another set.",
    ),
    MethodSpec::new(
        set_id(19),
        "is_superset",
        &[ParamSpec::new("other", "set")],
        "bool",
        "Returns true when all other values exist in this set.",
    ),
    MethodSpec::new(
        set_id(20),
        "is_disjoint",
        &[ParamSpec::new("other", "set")],
        "bool",
        "Returns true when two sets share no values.",
    ),
];

const fn set_id(offset: u64) -> HostMethodId {
    HostMethodId::new(0xff00_0a00 + offset)
}
