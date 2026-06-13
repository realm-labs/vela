use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, descs};

pub(crate) fn range_method_descs() -> Vec<MethodDesc> {
    descs("Range", RANGE_METHODS, "range")
}

const RANGE_METHODS: &[MethodSpec] = &[
    MethodSpec::new("len", &[], "i64", "Returns the range length."),
    MethodSpec::new(
        "is_empty",
        &[],
        "bool",
        "Returns true when the range contains no values.",
    ),
    MethodSpec::new(
        "iter",
        &[],
        "iterator",
        "Returns a one-shot iterator over range values.",
    ),
];
