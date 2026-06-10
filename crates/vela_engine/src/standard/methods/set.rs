use vela_common::{HostMethodId, stable_id};
use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};
use crate::standard::ids::{
    SET_ADD_METHOD_ID, SET_CLEAR_METHOD_ID, SET_HAS_METHOD_ID, SET_IS_DISJOINT_METHOD_ID,
    SET_IS_EMPTY_METHOD_ID, SET_IS_SUBSET_METHOD_ID, SET_IS_SUPERSET_METHOD_ID, SET_LEN_METHOD_ID,
    SET_REMOVE_METHOD_ID,
};

pub(crate) fn set_method_descs() -> Vec<MethodDesc> {
    descs(SET_METHODS, "set")
}

const SET_METHODS: &[MethodSpec] = &[
    MethodSpec::new(
        SET_LEN_METHOD_ID,
        "len",
        &[],
        "int",
        "Returns the set length.",
    ),
    MethodSpec::new(
        SET_IS_EMPTY_METHOD_ID,
        "is_empty",
        &[],
        "bool",
        "Returns true when the set has no values.",
    ),
    MethodSpec::new(
        SET_HAS_METHOD_ID,
        "has",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Returns true when a value exists.",
    ),
    MethodSpec::new(
        SET_ADD_METHOD_ID,
        "add",
        &[ParamSpec::new("value", "any")],
        "bool",
        "Adds a value and returns whether it was new.",
    ),
    MethodSpec::new(
        SET_REMOVE_METHOD_ID,
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
        "Adds all values from another set::",
    ),
    MethodSpec::new(
        SET_CLEAR_METHOD_ID,
        "clear",
        &[],
        "null",
        "Removes all values.",
    ),
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
        "Returns the first callback match, or Option::None.",
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
        "Returns values missing from the other set::",
    ),
    MethodSpec::new(
        set_id(17),
        "symmetric_difference",
        &[ParamSpec::new("other", "set")],
        "set",
        "Returns values present in exactly one set::",
    ),
    MethodSpec::new(
        SET_IS_SUBSET_METHOD_ID,
        "is_subset",
        &[ParamSpec::new("other", "set")],
        "bool",
        "Returns true when all values exist in another set::",
    ),
    MethodSpec::new(
        SET_IS_SUPERSET_METHOD_ID,
        "is_superset",
        &[ParamSpec::new("other", "set")],
        "bool",
        "Returns true when all other values exist in this set::",
    ),
    MethodSpec::new(
        SET_IS_DISJOINT_METHOD_ID,
        "is_disjoint",
        &[ParamSpec::new("other", "set")],
        "bool",
        "Returns true when two sets share no values.",
    ),
];

const fn set_id(offset: u64) -> HostMethodId {
    HostMethodId::new(stable_id("std_method_family", "Set", "").wrapping_add(offset))
}
