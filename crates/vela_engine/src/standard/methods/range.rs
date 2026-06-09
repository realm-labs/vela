use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, descs};
use crate::standard::ids::{RANGE_IS_EMPTY_METHOD_ID, RANGE_LEN_METHOD_ID};

pub(crate) fn range_method_descs() -> Vec<MethodDesc> {
    descs(RANGE_METHODS, "range")
}

const RANGE_METHODS: &[MethodSpec] = &[
    MethodSpec::new(
        RANGE_LEN_METHOD_ID,
        "len",
        &[],
        "int",
        "Returns the range length.",
    ),
    MethodSpec::new(
        RANGE_IS_EMPTY_METHOD_ID,
        "is_empty",
        &[],
        "bool",
        "Returns true when the range contains no values.",
    ),
];
