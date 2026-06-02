use vela_common::HostMethodId;
use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, descs};

pub(crate) fn range_method_descs() -> Vec<MethodDesc> {
    descs(RANGE_METHODS, "range")
}

const RANGE_METHODS: &[MethodSpec] = &[
    MethodSpec::new(range_id(0), "len", &[], "int", "Returns the range length."),
    MethodSpec::new(
        range_id(1),
        "is_empty",
        &[],
        "bool",
        "Returns true when the range contains no values.",
    ),
];

const fn range_id(offset: u64) -> HostMethodId {
    HostMethodId::new(0xff00_0d00 + offset)
}
